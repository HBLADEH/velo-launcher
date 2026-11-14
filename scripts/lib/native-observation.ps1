# Read-only HWND lookup for normal Slint sessions without an in-process probe.
function Get-VeloLauncherWindowHandle {
    param([int]$ProcessId, [ValidateSet('Slint','Wails')][string]$ApplicationKind = 'Slint')
    if (-not ('VeloMeasurement.SlintWindowLookup' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
namespace VeloMeasurement {
    public static class SlintWindowLookup {
        private delegate bool Callback(IntPtr window, IntPtr data);
        [DllImport("user32.dll")] private static extern bool EnumWindows(Callback callback, IntPtr data);
        [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
        [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr window, StringBuilder name, int count);
        public static string[] Read(uint pid, string className) {
            var matches = new List<string>();
            EnumWindows((window, unused) => {
                uint owner;
                GetWindowThreadProcessId(window, out owner);
                var name = new StringBuilder(256);
                if (owner == pid && GetClassName(window, name, name.Capacity) > 0 && name.ToString() == className) matches.Add(window.ToInt64().ToString("X"));
                return true;
            }, IntPtr.Zero);
            return matches.ToArray();
        }
    }
}
'@
    }
    $className = if ($ApplicationKind -eq 'Slint') { 'VeloSlintLauncherWindow' } else { 'VeloLauncherWindow' }
    $windows = @([VeloMeasurement.SlintWindowLookup]::Read($ProcessId, $className))
    if ($windows.Count -ne 1) { throw 'Expected one launcher window of the requested class owned by this PID.' }
    $windows[0]
}

function Get-VeloSlintWindowHandle {
    param([int]$ProcessId)
    Get-VeloLauncherWindowHandle -ProcessId $ProcessId -ApplicationKind Slint
}
