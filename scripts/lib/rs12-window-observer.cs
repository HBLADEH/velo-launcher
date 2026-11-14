// Read-only target-window state. No input, activation, titles, or query text.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;

namespace VeloRS12Window {
    public sealed class State {
        public long UnixMicroseconds;
        public double ElapsedMilliseconds;
        public bool Visible;
        public bool ForegroundTargetProcess;
        public uint DPI;
        public int X, Y, Width, Height;
    }
    public sealed class Capture {
        public int ProcessId;
        public long StartedTicks;
        public string HWND;
        public int Polls;
        public int IntervalMilliseconds;
        public double MaximumPollGapMilliseconds;
        public bool TargetEnded;
        public bool Truncated;
        public List<State> Changes = new List<State>();
    }
    public static class Observer {
        [StructLayout(LayoutKind.Sequential)] private struct Rect { public int Left,Top,Right,Bottom; }
        [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
        [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr window,out Rect rect);
        [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
        [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr window);
        [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] private static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
        public static bool WindowOwnedByProcess(long hwnd,int pid) {
            uint owner;
            // Failure leaves the out parameter untouched on Windows. A stale
            // PID must not keep a destroyed HWND alive in the observation.
            return GetWindowThreadProcessId(new IntPtr(hwnd),out owner)!=0 && owner==(uint)pid;
        }
        public static bool Changed(State previous,State current) {
            return previous==null || previous.Visible!=current.Visible || previous.ForegroundTargetProcess!=current.ForegroundTargetProcess ||
                previous.DPI!=current.DPI || previous.X!=current.X || previous.Y!=current.Y || previous.Width!=current.Width || previous.Height!=current.Height;
        }
        public static Capture Run(int pid,long startedTicks,long hwnd,int seconds,int intervalMilliseconds) {
            if(seconds<1 || seconds>900 || intervalMilliseconds<20 || intervalMilliseconds>1000) throw new ArgumentOutOfRangeException("Invalid bounded capture parameters.");
            using var process=Process.GetProcessById(pid);
            if(process.StartTime.ToUniversalTime().Ticks!=startedTicks) throw new InvalidOperationException("Target process creation identity changed.");
            var result=new Capture { ProcessId=pid,StartedTicks=startedTicks,HWND=hwnd.ToString("X"),IntervalMilliseconds=intervalMilliseconds };
            var window=new IntPtr(hwnd);
            var oldContext=SetThreadDpiAwarenessContext(new IntPtr(-4));
            if(oldContext==IntPtr.Zero) throw new InvalidOperationException("Cannot obtain physical target-window coordinates.");
            try {
                var watch=Stopwatch.StartNew();
                double previousPoll=0;
                State previous=null;
                while(watch.Elapsed.TotalSeconds<seconds) {
                    if(!WindowOwnedByProcess(hwnd,pid) || process.HasExited) { result.TargetEnded=true; break; }
                    Rect rect;
                    if(!GetWindowRect(window,out rect)) {
                        // Normal exit can destroy the window between the owner
                        // check and this call. Keep the observations already read.
                        if(!WindowOwnedByProcess(hwnd,pid) || process.HasExited) { result.TargetEnded=true; break; }
                        throw new InvalidOperationException("Cannot read target-window bounds.");
                    }
                    uint dpi=GetDpiForWindow(window);
                    if(!WindowOwnedByProcess(hwnd,pid) || process.HasExited) { result.TargetEnded=true; break; }
                    var current=new State { UnixMicroseconds=(DateTime.UtcNow.Ticks-DateTime.UnixEpoch.Ticks)/10,
                        ElapsedMilliseconds=watch.Elapsed.TotalMilliseconds,Visible=IsWindowVisible(window),ForegroundTargetProcess=WindowOwnedByProcess(GetForegroundWindow().ToInt64(),pid),
                        DPI=dpi,X=rect.Left,Y=rect.Top,Width=rect.Right-rect.Left,Height=rect.Bottom-rect.Top };
                    result.Polls++;
                    if(result.Polls>1) result.MaximumPollGapMilliseconds=Math.Max(result.MaximumPollGapMilliseconds,current.ElapsedMilliseconds-previousPoll);
                    previousPoll=current.ElapsedMilliseconds;
                    if(Changed(previous,current)) {
                        if(result.Changes.Count>=4096) { result.Truncated=true; break; }
                        result.Changes.Add(current);
                        previous=current;
                    }
                    Thread.Sleep(intervalMilliseconds);
                }
                return result;
            } finally { SetThreadDpiAwarenessContext(oldContext); }
        }
    }
}
