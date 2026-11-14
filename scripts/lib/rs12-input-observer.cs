// Passive RS-12 timing observer. Never sends input or changes a window.
// Only Alt+Space and visibility/focus metadata are retained; never query text.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace VeloRS12Input {
    public sealed class Observation {
        public int Number;
        public string Kind;
        public bool Valid;
        public bool InjectedStart;
        public string Reason;
        public long StartLowerUnixUs;
        public long StartObservedUnixUs;
        public long AcceptedUnixUs;
        public double DurationUpperMilliseconds;
        public double ClockUncertaintyMilliseconds;
        public double WallClockDriftMilliseconds;
    }
    public sealed class Result {
        public string Kind;
        public int ProcessId;
        public string HWND;
        public double EventClockQuantumMilliseconds;
        public List<Observation> Observations = new List<Observation>();
        public bool Completed;
        public string CountMode;
        public string EndReason;
        public bool ValidSampleCountSatisfied;
        public bool NativeEventsDropped;
        public List<NativeEvent> NativeEvents = new List<NativeEvent>();
    }
    public sealed class NativeEvent {
        public string Kind;
        public long UnixMicroseconds;
        public double ElapsedMilliseconds;
        public bool Visible;
        public bool ForegroundTargetProcess;
        public bool Injected;
    }
    public static class Observer {
        [StructLayout(LayoutKind.Sequential)] private struct KeyData { public uint VK, Scan, Flags, Time; public UIntPtr Extra; }
        [StructLayout(LayoutKind.Sequential)] private struct Message { public IntPtr Window; public uint Msg; public UIntPtr WParam; public IntPtr LParam; public uint Time; public int X, Y; public uint Private; }
        private delegate IntPtr Hook(int code, UIntPtr message, IntPtr data);
        private delegate void WinEvent(IntPtr hook, uint kind, IntPtr window, int objectId, int childId, uint thread, uint time);
        [DllImport("user32.dll")] private static extern IntPtr SetWinEventHook(uint first, uint last, IntPtr module, WinEvent callback, uint pid, uint thread, uint flags);
        [DllImport("user32.dll")] private static extern bool UnhookWinEvent(IntPtr hook);
        [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] private static extern IntPtr SetWindowsHookEx(int type, Hook callback, IntPtr module, uint thread);
        [DllImport("user32.dll")] private static extern bool UnhookWindowsHookEx(IntPtr hook);
        [DllImport("user32.dll")] private static extern IntPtr CallNextHookEx(IntPtr hook, int code, UIntPtr message, IntPtr data);
        [DllImport("user32.dll")] private static extern int GetMessage(out Message message, IntPtr window, uint min, uint max);
        [DllImport("user32.dll")] private static extern UIntPtr SetTimer(IntPtr window, UIntPtr id, uint milliseconds, IntPtr callback);
        [DllImport("user32.dll")] private static extern bool KillTimer(IntPtr window, UIntPtr id);
        [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
        [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
        [DllImport("user32.dll")] private static extern short GetAsyncKeyState(int key);
        [DllImport("kernel32.dll")] private static extern IntPtr GetModuleHandle(string name);
        [DllImport("kernel32.dll")] private static extern ulong GetTickCount64();
        [DllImport("kernel32.dll")] private static extern bool GetSystemTimeAdjustment(out uint adjustment, out uint increment, out bool disabled);
        private const long EpochTicks = 621355968000000000L;
        public static long EventLowerBound(long unixUs, uint nowTick, uint eventTick, long quantumUs) {
            return unixUs - (long)unchecked(nowTick - eventTick) * 1000L - quantumUs;
        }
        public static Dictionary<string,string> Parse(string line) {
            var fields = new Dictionary<string,string>();
            foreach (var part in line.Split(' ')) {
                int equal = part.IndexOf('=');
                if (equal > 0) fields[part.Substring(0,equal)] = part.Substring(equal+1);
            }
            return fields;
        }
        private static bool Is(Dictionary<string,string> fields, string name, string value) {
            string actual;
            return fields.TryGetValue(name, out actual) && actual == value;
        }
        public static bool WindowOwnedByProcess(long hwnd,int pid) {
            uint owner;
            // Windows may leave owner unchanged when the HWND is destroyed.
            return GetWindowThreadProcessId(new IntPtr(hwnd),out owner)!=0 && owner==(uint)pid;
        }
        public static bool Accepted(Dictionary<string,string> fields, int pid) {
            string bytes;
            int count;
            return Is(fields,"event","input_accepted") && Is(fields,"pid",pid.ToString()) &&
                Is(fields,"focused","true") && Is(fields,"native_focused","true") &&
                Is(fields,"visible","true") && Is(fields,"composing","false") &&
                !Is(fields,"synthetic","true") && !Is(fields,"trusted","false") && Is(fields,"dropped","0") &&
                fields.TryGetValue("utf8_bytes",out bytes) && int.TryParse(bytes,out count) && count>0;
        }
        private static string[] Lines(string path) {
            if (!File.Exists(path)) return new string[0];
            using (var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
            using (var reader = new StreamReader(stream, Encoding.UTF8)) {
                var text = reader.ReadToEnd();
                int last = text.LastIndexOf('\n');
                return last < 0 ? new string[0] : text.Substring(0,last).Split('\n');
            }
        }
        public static bool CountReached(List<Observation> observations, int count, string countMode) {
            if(countMode=="UntilExit") return false;
            int valid=0;
            foreach(var observation in observations) if(observation.Valid && !observation.InjectedStart) valid++;
            return countMode=="PhysicalStarts" ? observations.Count>=count : valid>=count;
        }
        public static void EndPending(Result result, ref Observation pending, string reason) {
            if(pending==null) return;
            pending.Number=result.Observations.Count+1;
            pending.Valid=false;
            pending.Reason=reason;
            result.Observations.Add(pending);
            pending=null;
        }
        public static Result Run(int pid, long hwnd, string path, string kind, int count, int timeoutSeconds, long processStartUs, string countMode="ValidSamples") {
            if(countMode!="ValidSamples" && countMode!="PhysicalStarts" && countMode!="UntilExit") throw new ArgumentException("Unsupported input count mode.");
            if(kind!="HotkeyActivation" && countMode!="ValidSamples") throw new ArgumentException("Only hotkey observation supports diagnostic count modes.");
            var window = new IntPtr(hwnd);
            using var process=Process.GetProcessById(pid);
            if (!WindowOwnedByProcess(hwnd,pid) || process.HasExited) throw new InvalidOperationException("The launcher HWND is not owned by the target PID.");
            uint adjustment, increment;
            bool disabled;
            if (!GetSystemTimeAdjustment(out adjustment,out increment,out disabled)) throw new InvalidOperationException("Cannot bound the Windows event clock.");
            long quantumUs = (increment + 9L) / 10L;
            var result = new Result { Kind=kind, ProcessId=pid, HWND=hwnd.ToString("X"), EventClockQuantumMilliseconds=quantumUs/1000.0, CountMode=countMode, EndReason="Timeout" };
            Observation pending = null;
            long pendingWatch = 0;
            int parsed = 0;
            bool lost = false;
            var watch = Stopwatch.StartNew();
            Action<string,bool> recordNative = (eventKind,injected) => {
                if(result.NativeEvents.Count>=512) { result.NativeEventsDropped=true; return; }
                result.NativeEvents.Add(new NativeEvent { Kind=eventKind, UnixMicroseconds=(DateTime.UtcNow.Ticks-EpochTicks)/10,
                    ElapsedMilliseconds=watch.Elapsed.TotalMilliseconds, Visible=IsWindowVisible(window),
                    ForegroundTargetProcess=WindowOwnedByProcess(GetForegroundWindow().ToInt64(),pid), Injected=injected });
            };
            WinEvent nativeCallback = (eventHook,eventKind,eventWindow,objectId,childId,thread,time) => {
                if(eventKind==3) recordNative("foreground",false);
                else if(eventWindow==window && objectId==0 && childId==0) recordNative(eventKind==0x8002 ? "show" : "hide",false);
            };
            if (kind == "WarmStartup" || kind == "ColdStartup") {
                pending = new Observation { Kind=kind, StartLowerUnixUs=processStartUs-1000, StartObservedUnixUs=processStartUs, ClockUncertaintyMilliseconds=1.0 };
                pendingWatch = Stopwatch.GetTimestamp();
            }
            bool spaceDown=false;
            Hook callback = (code,message,data) => {
                if (code == 0 && kind == "HotkeyActivation") {
                    var key = Marshal.PtrToStructure<KeyData>(data);
                    bool keyDown=message.ToUInt64()==0x100 || message.ToUInt64()==0x104;
                    bool repeated=spaceDown;
                    if(key.VK==0x20) spaceDown=keyDown;
                    if (keyDown && !repeated && key.VK == 0x20 && (key.Flags & 0x20) != 0 &&
                        GetAsyncKeyState(0x11) >= 0 && GetAsyncKeyState(0x10) >= 0 && GetAsyncKeyState(0x5b) >= 0 && GetAsyncKeyState(0x5c) >= 0) {
                        bool injected=(key.Flags&0x10)!=0;
                        recordNative("alt_space",injected);
                        // Automation must neither start a physical attempt nor
                        // consume/replace one already in progress.
                        if(injected) return CallNextHookEx(IntPtr.Zero,code,message,data);
                        // A retry cannot silently turn an earlier flash/focus failure
                        // into one successful, slow activation observation.
                        EndPending(result,ref pending,"Another physical Alt+Space arrived before accepted input; earlier activation retained as incomplete.");
                        if(IsWindowVisible(window) || CountReached(result.Observations,count,countMode)) return CallNextHookEx(IntPtr.Zero,code,message,data);
                        long before = (DateTime.UtcNow.Ticks-EpochTicks)/10;
                        uint uptime = (uint)GetTickCount64();
                        long after = (DateTime.UtcNow.Ticks-EpochTicks)/10;
                        pending = new Observation { Kind=kind, InjectedStart=injected,
                            StartLowerUnixUs=EventLowerBound(before,uptime,key.Time,quantumUs), StartObservedUnixUs=after,
                            ClockUncertaintyMilliseconds=(quantumUs + after-before)/1000.0 };
                        pendingWatch = Stopwatch.GetTimestamp();
                    }
                }
                return CallNextHookEx(IntPtr.Zero,code,message,data);
            };
            IntPtr hook = IntPtr.Zero;
            IntPtr visibilityHook = IntPtr.Zero;
            IntPtr foregroundHook = IntPtr.Zero;
            UIntPtr timer = UIntPtr.Zero;
            try {
                if (kind == "HotkeyActivation") {
                    hook = SetWindowsHookEx(13,callback,GetModuleHandle(null),0);
                    if (hook == IntPtr.Zero) throw new InvalidOperationException("Cannot install passive Alt+Space observer.");
                    visibilityHook=SetWinEventHook(0x8002,0x8003,IntPtr.Zero,nativeCallback,(uint)pid,0,0);
                    foregroundHook=SetWinEventHook(3,3,IntPtr.Zero,nativeCallback,0,0,0);
                    if(visibilityHook==IntPtr.Zero || foregroundHook==IntPtr.Zero) throw new InvalidOperationException("Cannot install passive visibility/focus observer.");
                }
                timer = SetTimer(IntPtr.Zero,UIntPtr.Zero,50,IntPtr.Zero);
                if (timer == UIntPtr.Zero) throw new InvalidOperationException("Cannot start bounded observer timer.");
                Console.WriteLine("RS12_INPUT_OBSERVER_READY pid="+pid+" kind="+kind+" samples="+count);
                while (watch.Elapsed.TotalSeconds < timeoutSeconds) {
                    Message message;
                    if (GetMessage(out message,IntPtr.Zero,0,0) <= 0) { result.EndReason="MessageLoopEnded"; break; }
                    if (message.Msg != 0x113) continue;
                    bool targetEnded=process.HasExited || !WindowOwnedByProcess(hwnd,pid);
                    var lines = Lines(path);
                    for (;parsed<lines.Length;parsed++) {
                        var fields = Parse(lines[parsed]);
                        if (Is(fields,"pid",pid.ToString()) && !Is(fields,"dropped","0")) lost = true;
                        long accepted;
                        if (pending == null || !Accepted(fields,pid) || !fields.ContainsKey("unix_us") || !long.TryParse(fields["unix_us"],out accepted) || accepted < pending.StartObservedUnixUs) continue;
                        pending.AcceptedUnixUs = accepted;
                        pending.DurationUpperMilliseconds=(accepted-pending.StartLowerUnixUs)/1000.0;
                        if (kind == "HotkeyActivation") {
                            double mono=(Stopwatch.GetTimestamp()-pendingWatch)*1000.0/Stopwatch.Frequency;
                            double wall=((DateTime.UtcNow.Ticks-EpochTicks)/10-pending.StartObservedUnixUs)/1000.0;
                            pending.WallClockDriftMilliseconds=Math.Abs(mono-wall);
                        }
                        pending.Valid=!lost && !pending.InjectedStart && pending.DurationUpperMilliseconds>0 && pending.WallClockDriftMilliseconds<=5;
                        pending.Reason=lost ? "Observer log reported event loss." : pending.InjectedStart ? "Injected hotkey is excluded from physical-key samples." : pending.WallClockDriftMilliseconds>5 ? "UTC/monotonic mapping drift exceeded 5ms." : "Actual character accepted; upper bound includes manual delay before typing. No subtraction of human waiting or claim of first-possible readiness.";
                        pending.Number=result.Observations.Count+1;
                        result.Observations.Add(pending);
                        Console.WriteLine("RS12_INPUT_SAMPLE number="+pending.Number+" valid="+pending.Valid.ToString().ToLowerInvariant()+" upper_ms="+pending.DurationUpperMilliseconds.ToString("F3",System.Globalization.CultureInfo.InvariantCulture));
                        pending=null;
                        if (kind != "HotkeyActivation") break;
                    }
                    if (pending != null && kind == "HotkeyActivation" && (Stopwatch.GetTimestamp()-pendingWatch)/(double)Stopwatch.Frequency>10) {
                        EndPending(result,ref pending,"No focused non-composing character accepted within ten seconds of the hotkey.");
                    }
                    // Read final complete log lines before ending, even if the
                    // process/window disappeared between timer callbacks.
                    if(targetEnded) { result.EndReason="TargetWindowEnded"; result.Completed=countMode=="UntilExit" || CountReached(result.Observations,count,countMode); break; }
                    if(CountReached(result.Observations,count,countMode)) { result.Completed=true; result.EndReason="RequestedCountReached"; break; }
                }
                EndPending(result,ref pending,"Observer ended before actual focused input was accepted.");
                result.ValidSampleCountSatisfied=CountReached(result.Observations,count,"ValidSamples");
                return result;
            } finally {
                if(timer!=UIntPtr.Zero) KillTimer(IntPtr.Zero,timer);
                if(hook!=IntPtr.Zero) UnhookWindowsHookEx(hook);
                if(visibilityHook!=IntPtr.Zero) UnhookWinEvent(visibilityHook);
                if(foregroundHook!=IntPtr.Zero) UnhookWinEvent(foregroundHook);
                GC.KeepAlive(callback);
                GC.KeepAlive(nativeCallback);
            }
        }
    }
}
