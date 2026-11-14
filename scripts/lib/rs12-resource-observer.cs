// Read-only, bounded sampling of one process and all observed descendants.
// Toolhelp avoids running the CIM provider at every high-frequency sample.
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;

namespace VeloRS12Resources {
    public sealed class ProcessSample {
        public int ProcessId;
        public int ParentProcessId;
        public long StartedTicks;
        public long PrivateBytes;
        public long WorkingSet;
        public double CPUSeconds;
        public uint Handles;
        public uint Threads;
    }
    public sealed class Snapshot {
        public long StartedUtcTicks;
        public long StartedQpc;
        public long EndedQpc;
        public double ElapsedMilliseconds;
        public double CollectionMilliseconds;
        public bool Complete;
        public List<int> UnreadableProcessIds = new List<int>();
        public List<ProcessSample> Processes = new List<ProcessSample>();
        public long PrivateBytes;
        public long WorkingSet;
        public long Handles;
        public long Threads;
    }
    public sealed class Capture {
        public int ProcessId;
        public long StartedTicks;
        public int IntervalMilliseconds;
        public int RequestedSeconds;
        public long CaptureStartedUtcTicks;
        public double ElapsedMilliseconds;
        public double ObserverCPUSeconds;
        public long QpcFrequency;
        public bool RootExited;
        public List<Snapshot> Snapshots = new List<Snapshot>();
    }
    public static class Observer {
        [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
        private struct Entry {
            public uint Size, Usage, Id;
            public UIntPtr Heap;
            public uint Module, Threads, Parent;
            public int Priority;
            public uint Flags;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst=260)] public string Name;
        }
        [StructLayout(LayoutKind.Sequential)]
        private struct Memory {
            public uint Size, PageFaults;
            public UIntPtr PeakWorkingSet, WorkingSet, PeakPagedPool, PagedPool;
            public UIntPtr PeakNonPagedPool, NonPagedPool, Pagefile, PeakPagefile, PrivateUsage;
        }
        [DllImport("kernel32.dll", SetLastError=true)] private static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint pid);
        [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] private static extern bool Process32FirstW(IntPtr snapshot, ref Entry entry);
        [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] private static extern bool Process32NextW(IntPtr snapshot, ref Entry entry);
        [DllImport("kernel32.dll", SetLastError=true)] private static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
        [DllImport("kernel32.dll")] private static extern bool CloseHandle(IntPtr handle);
        [DllImport("kernel32.dll", SetLastError=true)] private static extern bool GetProcessTimes(IntPtr handle, out long created, out long exited, out long kernel, out long user);
        [DllImport("kernel32.dll", SetLastError=true)] private static extern bool GetProcessHandleCount(IntPtr handle, out uint count);
        [DllImport("kernel32.dll")] private static extern uint WaitForSingleObject(IntPtr handle, uint timeout);
        [DllImport("psapi.dll", SetLastError=true)] private static extern bool GetProcessMemoryInfo(IntPtr handle, ref Memory memory, uint size);

        private static List<Entry> Entries() {
            var handle = CreateToolhelp32Snapshot(2, 0);
            if (handle == new IntPtr(-1)) throw new Win32Exception(Marshal.GetLastWin32Error(), "Cannot enumerate the process tree.");
            try {
                var entries = new List<Entry>();
                var entry = new Entry { Size=(uint)Marshal.SizeOf<Entry>() };
                if (!Process32FirstW(handle, ref entry)) throw new Win32Exception(Marshal.GetLastWin32Error(), "Cannot read the process snapshot.");
                do { entries.Add(entry); } while (Process32NextW(handle, ref entry));
                if (Marshal.GetLastWin32Error() != 18) throw new Win32Exception(Marshal.GetLastWin32Error(), "Process enumeration was incomplete.");
                return entries;
            } finally { CloseHandle(handle); }
        }

        private static ProcessSample Read(Entry entry) {
            // Query + VM_READ + SYNCHRONIZE; every opened handle is released here.
            var handle = OpenProcess(0x101010, false, entry.Id);
            if (handle == IntPtr.Zero) return null;
            try {
                long created, exited, kernel, user;
                uint handles;
                var memory = new Memory { Size=(uint)Marshal.SizeOf<Memory>() };
                if (WaitForSingleObject(handle, 0) != 258 ||
                    !GetProcessTimes(handle, out created, out exited, out kernel, out user) ||
                    !GetProcessMemoryInfo(handle, ref memory, memory.Size) || !GetProcessHandleCount(handle, out handles)) return null;
                return new ProcessSample { ProcessId=(int)entry.Id, ParentProcessId=(int)entry.Parent,
                    StartedTicks=DateTime.FromFileTimeUtc(created).Ticks, CPUSeconds=(kernel+user)/10000000.0,
                    PrivateBytes=(long)memory.PrivateUsage.ToUInt64(), WorkingSet=(long)memory.WorkingSet.ToUInt64(),
                    Handles=handles, Threads=entry.Threads };
            } finally { CloseHandle(handle); }
        }

        public static Task<Capture> Start(int pid, long startedTicks, int seconds, int intervalMilliseconds) {
            if (pid <= 0 || seconds < 1 || seconds > 120 || intervalMilliseconds < 10 || intervalMilliseconds > 1000)
                throw new ArgumentOutOfRangeException("Capture parameters are outside their bounded ranges.");
            return Task.Run(() => Run(pid, startedTicks, seconds, intervalMilliseconds));
        }

        private static Capture Run(int pid, long startedTicks, int seconds, int intervalMilliseconds) {
            var result = new Capture { ProcessId=pid, StartedTicks=startedTicks, RequestedSeconds=seconds,
                IntervalMilliseconds=intervalMilliseconds, CaptureStartedUtcTicks=DateTime.UtcNow.Ticks, QpcFrequency=Stopwatch.Frequency };
            var previousIdentities = new Dictionary<int,long>();
            using var host = Process.GetCurrentProcess();
            double observerCPU = host.TotalProcessorTime.TotalSeconds;
            var watch = Stopwatch.StartNew();
            while (watch.Elapsed.TotalSeconds < seconds) {
                var sampleWatch = Stopwatch.StartNew();
                var sample = new Snapshot { StartedUtcTicks=DateTime.UtcNow.Ticks, StartedQpc=Stopwatch.GetTimestamp(), ElapsedMilliseconds=watch.Elapsed.TotalMilliseconds, Complete=true };
                var entries = Entries();
                var selected = new Dictionary<int,ProcessSample>();
                var examined = new HashSet<int>();
                foreach (var entry in entries) {
                    long expected;
                    if ((int)entry.Id != pid && !previousIdentities.TryGetValue((int)entry.Id, out expected)) continue;
                    var process = Read(entry);
                    examined.Add((int)entry.Id);
                    expected = (int)entry.Id == pid ? startedTicks : previousIdentities[(int)entry.Id];
                    if (process != null && process.StartedTicks == expected) selected.Add(process.ProcessId, process);
                    else if ((int)entry.Id == pid) { result.RootExited=true; break; }
                    else if (process == null) { sample.UnreadableProcessIds.Add((int)entry.Id); sample.Complete=false; }
                    // A PID reused by an unrelated process must not inherit ancestry.
                }
                if (!selected.ContainsKey(pid)) { result.RootExited=true; break; }
                bool added;
                do {
                    added=false;
                    foreach (var entry in entries) {
                        ProcessSample parent;
                        if (examined.Contains((int)entry.Id) || !selected.TryGetValue((int)entry.Parent, out parent)) continue;
                        examined.Add((int)entry.Id);
                        var process = Read(entry);
                        if (process == null) { sample.UnreadableProcessIds.Add((int)entry.Id); sample.Complete=false; }
                        else if (process.StartedTicks >= parent.StartedTicks) { selected.Add(process.ProcessId, process); added=true; }
                    }
                } while (added);
                previousIdentities.Clear();
                foreach (var process in selected.Values) {
                    previousIdentities.Add(process.ProcessId, process.StartedTicks);
                    sample.Processes.Add(process);
                    sample.PrivateBytes+=process.PrivateBytes;
                    sample.WorkingSet+=process.WorkingSet;
                    sample.Handles+=process.Handles;
                    sample.Threads+=process.Threads;
                }
                sample.CollectionMilliseconds=sampleWatch.Elapsed.TotalMilliseconds;
                sample.EndedQpc=Stopwatch.GetTimestamp();
                result.Snapshots.Add(sample);
                int delay=intervalMilliseconds-(int)Math.Ceiling(sampleWatch.Elapsed.TotalMilliseconds);
                if (delay>0) Thread.Sleep(delay);
            }
            result.ElapsedMilliseconds=watch.Elapsed.TotalMilliseconds;
            result.ObserverCPUSeconds=host.TotalProcessorTime.TotalSeconds-observerCPU;
            return result;
        }
    }
}
