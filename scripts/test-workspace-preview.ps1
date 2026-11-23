#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateSet('disabled', 'shaped', 'rounded', 'radial', 'conflict')][string]$Mode = 'shaped',
    [int]$Monitor = 0,
    [switch]$CrossMonitors,
    [ValidateSet('xbutton1', 'xbutton2')][string]$SideButton,
    [switch]$SettingsRoundTrip,
    [switch]$EditSlots,
    [switch]$MeasureResources,
    [ValidateRange(0, 100)][int]$SideSamples = 0
)

$ErrorActionPreference = 'Stop'
if (($SideButton -or $SettingsRoundTrip -or $EditSlots -or $CrossMonitors) -and $Mode -ne 'radial') {
    throw 'Side-button, settings and cross-monitor tests require radial mode.'
}
if ($EditSlots -and $SettingsRoundTrip) {
    throw 'Run -EditSlots and -SettingsRoundTrip separately. They seed different initial settings.'
}
if ($SideSamples -and -not $SideButton) { throw '-SideSamples requires -SideButton.' }
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
if (-not ('VeloWorkspaceProbe' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class VeloWorkspaceProbe {
    [StructLayout(LayoutKind.Sequential)]
    public struct Point { public int X, Y; public Point(int x, int y) { X = x; Y = y; } }
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)]
    struct MenuBarInfo {
        public uint Size;
        public Rect Bounds;
        public IntPtr Menu;
        public IntPtr Window;
        public uint Focus;
    }
    [StructLayout(LayoutKind.Sequential)]
    struct MouseInput { public int X, Y; public uint Data, Flags, Time; public UIntPtr Extra; }
    [StructLayout(LayoutKind.Sequential)]
    struct Input { public uint Type; public MouseInput Mouse; }
    delegate bool Enumerator(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] static extern bool EnumWindows(Enumerator callback, IntPtr data);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern int GetClassName(IntPtr window, StringBuilder name, int length);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(uint processId);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")]
    static extern bool GetMenuBarInfo(IntPtr window, int objectId, int item, ref MenuBarInfo info);
    [DllImport("user32.dll", SetLastError = true)]
    static extern bool GetMenuItemRect(IntPtr window, IntPtr menu, uint item, out Rect rect);
    [DllImport("user32.dll")] static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32.dll")] static extern uint GetMenuItemID(IntPtr menu, int position);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window, uint message, IntPtr w, IntPtr l);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern IntPtr SendMessageTimeout(IntPtr window, uint message, IntPtr w, IntPtr l,
        uint flags, uint milliseconds, out UIntPtr result);
    [DllImport("user32.dll")] static extern bool RegisterHotKey(IntPtr window, int id, uint modifiers, uint key);
    [DllImport("user32.dll")] static extern bool UnregisterHotKey(IntPtr window, int id);
    [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr window);
    [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr window, IntPtr dc);
    [DllImport("gdi32.dll")] static extern uint GetPixel(IntPtr dc, int x, int y);
    [DllImport("user32.dll")] static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] static extern void mouse_event(uint flags, int x, int y, uint data, UIntPtr extra);
    [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, Input[] inputs, int size);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int index);
    [DllImport("dwmapi.dll")] public static extern int DwmFlush();
    [StructLayout(LayoutKind.Sequential)]
    struct HookEvent { public Point Pt; public uint Data, Flags, Time; public UIntPtr Extra; }
    delegate IntPtr LowMouse(int code, IntPtr w, IntPtr l);
    [DllImport("user32.dll", SetLastError = true)] static extern IntPtr SetWindowsHookEx(int id, LowMouse proc, IntPtr module, uint thread);
    [DllImport("user32.dll")] static extern bool UnhookWindowsHookEx(IntPtr hook);
    [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetModuleHandle(string name);
    [DllImport("kernel32.dll")] static extern IntPtr OpenThread(uint access, bool inherit, uint id);
    [DllImport("kernel32.dll")] static extern int GetThreadDescription(IntPtr thread, out IntPtr description);
    [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr memory);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    static readonly LowMouse Watch = Observe;
    static readonly UIntPtr Marker = (UIntPtr)(0x564C0000u | ((uint)Environment.ProcessId & 0xffff));
    static IntPtr watcher;
    public static int SideDowns { get; private set; }
    public static int SideUps { get; private set; }
    public static void WatchSide() {
        watcher = SetWindowsHookEx(14, Watch, GetModuleHandle(null), 0);
        if (watcher == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
    public static void UnwatchSide() {
        if (watcher != IntPtr.Zero) UnhookWindowsHookEx(watcher);
        watcher = IntPtr.Zero;
    }
    static IntPtr Observe(int code, IntPtr w, IntPtr l) {
        if (code == 0 && (w.ToInt64() == 0x20B || w.ToInt64() == 0x20C) &&
            Marshal.PtrToStructure<HookEvent>(l).Extra == Marker) {
            if (w.ToInt64() == 0x20B) SideDowns++; else SideUps++;
            return (IntPtr)1;
        }
        return CallNextHookEx(watcher, code, w, l);
    }

    public static bool Free(uint modifiers, uint key) {
        return RegisterHotKey(IntPtr.Zero, 301, modifiers | 0x4000, key) && UnregisterHotKey(IntPtr.Zero, 301);
    }
    public static string ThreadName(uint id) {
        const uint queryLimitedInformation = 0x0800;
        var thread = OpenThread(queryLimitedInformation, false, id);
        if (thread == IntPtr.Zero) return null;
        try {
            if (GetThreadDescription(thread, out var description) < 0) return null;
            try { return Marshal.PtrToStringUni(description); }
            finally { LocalFree(description); }
        } finally { CloseHandle(thread); }
    }
    public static bool RepaintActiveFrame(IntPtr window) {
        const uint wmNcActivate = 0x0086, abortIfHung = 0x0002;
        return SendMessageTimeout(window, wmNcActivate, (IntPtr)1, IntPtr.Zero, abortIfHung, 2000, out _) != IntPtr.Zero;
    }
    public static IntPtr Find(uint pid, string windowClass) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((window, data) => {
            GetWindowThreadProcessId(window, out uint owner);
            if (owner != pid) return true;
            var name = new StringBuilder(128);
            GetClassName(window, name, name.Capacity);
            if (name.ToString() != windowClass) return true;
            found = window;
            return false;
        }, IntPtr.Zero);
        return found;
    }
    public static Rect Bounds(IntPtr window) {
        if (!GetWindowRect(window, out Rect rect)) throw new InvalidOperationException("GetWindowRect");
        return rect;
    }
    public static Rect TrayExitBounds(IntPtr popup) {
        const int popupMenu = -4;
        const uint exitCommand = 3;
        var info = new MenuBarInfo { Size = (uint)Marshal.SizeOf<MenuBarInfo>() };
        if (!GetMenuBarInfo(popup, popupMenu, 0, ref info) || info.Menu == IntPtr.Zero) {
            throw new InvalidOperationException("No native popup menu for the tray window.");
        }
        var count = GetMenuItemCount(info.Menu);
        for (var index = 0; index < count; index++) {
            if (GetMenuItemID(info.Menu, index) != exitCommand) continue;
            if (GetMenuItemRect(IntPtr.Zero, info.Menu, (uint)index, out var bounds)) return bounds;
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        }
        throw new InvalidOperationException("The native tray menu has no exit command.");
    }
    public static void Press(byte[] keys) {
        foreach (byte key in keys) keybd_event(key, 0, 0, UIntPtr.Zero);
        for (int i = keys.Length - 1; i >= 0; i--) keybd_event(keys[i], 0, 2, UIntPtr.Zero);
    }
    public static void PressEditor(byte[] keys) {
        foreach (byte key in keys) {
            keybd_event(key, 0, 0, UIntPtr.Zero);
            System.Threading.Thread.Sleep(45);
        }
        for (int i = keys.Length - 1; i >= 0; i--) keybd_event(keys[i], 0, 2, UIntPtr.Zero);
    }
    public static void Left(bool down) {
        Inject(new MouseInput { Flags = down ? 2u : 4u });
    }
    public static void Side(bool down, uint button) {
        Inject(new MouseInput { Data = button, Flags = down ? 0x80u : 0x100u, Extra = Marker });
    }
    public static void Move(int x, int y) {
        Inject(Motion(x, y));
    }
    public static void Swipe(int x, int y, uint button) {
        var sequence = new[] {
            new Input { Mouse = new MouseInput { Data = button, Flags = 0x80u, Extra = Marker } },
            new Input { Mouse = Motion(x, y) },
            new Input { Mouse = new MouseInput { Data = button, Flags = 0x100u, Extra = Marker } }
        };
        if (SendInput((uint)sequence.Length, sequence, Marshal.SizeOf<Input>()) != sequence.Length) {
            Inject(sequence[2].Mouse);
            throw new InvalidOperationException("Sending the complete side-button swipe failed");
        }
    }
    static MouseInput Motion(int x, int y) {
        int nx = (int)((x - GetSystemMetrics(76) + 0.5) * 65536 / GetSystemMetrics(78));
        int ny = (int)((y - GetSystemMetrics(77) + 0.5) * 65536 / GetSystemMetrics(79));
        return new MouseInput { X = nx, Y = ny, Flags = 0xc001 };
    }
    static void Inject(MouseInput mouse) {
        var packet = new Input { Mouse = mouse };
        if (SendInput(1, new[] { packet }, Marshal.SizeOf<Input>()) != 1) {
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "SendInput mouse probe");
        }
    }
    public static void Click(int x, int y) {
        if (!SetCursorPos(x, y)) throw new InvalidOperationException("SetCursorPos");
        System.Threading.Thread.Sleep(25);
        mouse_event(2, 0, 0, 0, UIntPtr.Zero);
        mouse_event(4, 0, 0, 0, UIntPtr.Zero);
    }
    public static uint Pixel(int x, int y) {
        var dc = GetDC(IntPtr.Zero);
        try { return GetPixel(dc, x, y); }
        finally { ReleaseDC(IntPtr.Zero, dc); }
    }
}
'@
}

function Wait-Desktop([string]$Description, [scriptblock]$Condition, [int]$Timeout = 5000) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        [Windows.Forms.Application]::DoEvents()
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 4
    } while ($timer.ElapsedMilliseconds -lt $Timeout)
    throw "Timed out waiting for $Description"
}

function Assert-Probe([bool]$Passed, [string]$Description) {
    if (-not $Passed) { throw "Workspace verification failed: $Description" }
    Write-Output "pass $Description"
}

function Read-Events {
    if (Test-Path -LiteralPath $eventsPath) { Get-Content -LiteralPath $eventsPath -Raw }
}

function Open-Panel([string]$Class, [byte[]]$Keys, [int]$X, [int]$Y) {
    Focus-Witness
    [void][VeloWorkspaceProbe]::SetCursorPos($X, $Y)
    $timer = [Diagnostics.Stopwatch]::StartNew()
    [VeloWorkspaceProbe]::Press($Keys)
    Wait-Desktop "$Class foreground" {
        $foreground = [VeloWorkspaceProbe]::GetForegroundWindow()
        $foreground -ne [IntPtr]::Zero -and $foreground -eq [VeloWorkspaceProbe]::Find($process.Id, $Class)
    }
    $window = [VeloWorkspaceProbe]::GetForegroundWindow()
    $rect = [VeloWorkspaceProbe]::Bounds($window)
    $isRadial = $Class -eq $workspaceClass -and $Mode -eq 'radial'
    $sampleX = $rect.Left + [int](($rect.Right - $rect.Left) * $(if ($isRadial) { 0.46 } else { 0.28 }))
    $sampleY = $rect.Top + [int](($rect.Bottom - $rect.Top) * $(if ($isRadial) { 0.09 } else { 0.12 }))
    Wait-Desktop "$Class painting" {
        $pixel = [VeloWorkspaceProbe]::Pixel($sampleX, $sampleY)
        $pixel -ne 0x675342 -and $pixel -ne 0 -and $pixel -ne [uint32]::MaxValue
    }
    $milliseconds = $timer.Elapsed.TotalMilliseconds
    if ($Class -eq $workspaceClass) {
        for ($sample = 0; $sample -lt 5; $sample++) {
            Start-Sleep -Milliseconds 12
            $pixel = [VeloWorkspaceProbe]::Pixel($sampleX, $sampleY)
            $paintSamples.Add($pixel)
        }
    }
    [pscustomobject]@{ Handle = $window; Rect = $rect; Milliseconds = $milliseconds }
}

function Focus-Witness {
    $witness.Activate()
    [void][VeloWorkspaceProbe]::SetForegroundWindow($witness.Handle)

    if ([VeloWorkspaceProbe]::GetForegroundWindow() -ne $witness.Handle) {
        Click-TestPoint $centerX $centerY $witness.Handle
    }
    Wait-Desktop 'test surface foreground' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle }
}

function Close-Panel($Panel, [byte[]]$Keys, [bool]$Restore) {
    if ([VeloWorkspaceProbe]::GetForegroundWindow() -ne $Panel.Handle) { throw 'Panel lost foreground before dismissal.' }
    [VeloWorkspaceProbe]::Press($Keys)
    Wait-Desktop 'panel dismissed' { -not [VeloWorkspaceProbe]::IsWindowVisible($Panel.Handle) }
    if ($Restore) {
        Wait-Desktop 'captured surface regains foreground' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle }
    }
}

function Click-TestPoint([int]$X, [int]$Y, [IntPtr]$Expected) {
    $hit = [VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($X, $Y))
    if ($hit -ne $Expected) { throw "Unexpected hit window at ($X,$Y)." }
    [VeloWorkspaceProbe]::Click($X, $Y)
}

function Save-Panel($Panel, [string]$Name = $Mode) {
    [void][VeloWorkspaceProbe]::DwmFlush()
    Start-Sleep -Milliseconds 120
    [void][VeloWorkspaceProbe]::DwmFlush()
    $rect = $Panel.Rect
    $bitmap = [Drawing.Bitmap]::new($rect.Right - $rect.Left + 20, $rect.Bottom - $rect.Top + 20)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($rect.Left - 10, $rect.Top - 10, 0, 0, $bitmap.Size)
        $bitmap.Save((Join-Path $OutputDirectory "$Name.png"), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $graphics.Dispose(); $bitmap.Dispose() }
}

function Read-ScreenPatch([Drawing.Rectangle]$Area) {
    $bitmap = [Drawing.Bitmap]::new($Area.Width, $Area.Height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $stream = [IO.MemoryStream]::new()
    try {
        $graphics.CopyFromScreen($Area.Location, [Drawing.Point]::Empty, $Area.Size)
        $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Bmp)
        [Convert]::ToBase64String($stream.ToArray())
    } finally { $stream.Dispose(); $graphics.Dispose(); $bitmap.Dispose() }
}

function Test-RadialFrame($Panel) {
    $rect = $Panel.Rect
    $sx = ($rect.Right - $rect.Left) / 420
    $sy = ($rect.Bottom - $rect.Top) / 424
    $capY = $rect.Top + [int](20 * $sy)
    $fill = [VeloWorkspaceProbe]::Pixel(($centerX + [int](28 * $sx)), ($rect.Top + [int](34 * $sy)))
    $corner = [Drawing.Rectangle]::FromLTRB(
        ($rect.Left + [int](135 * $sx)), ($rect.Top + [int](21 * $sy)),
        ($rect.Left + [int](147 * $sx)), ($rect.Top + [int](32 * $sy)))
    $originalCorner = Read-ScreenPatch $corner
    Assert-Probe ([VeloWorkspaceProbe]::Pixel($centerX, $capY) -eq $fill) 'radial first full paint has a consistent outer tile fill'

    $idle = [Diagnostics.Stopwatch]::StartNew()
    Wait-Desktop 'five seconds of radial idle' { $idle.ElapsedMilliseconds -ge 5000 } 6000
    Assert-Probe ([VeloWorkspaceProbe]::GetForegroundWindow() -eq $Panel.Handle -and
        [VeloWorkspaceProbe]::Pixel($centerX, $capY) -eq $fill -and
        (Read-ScreenPatch $corner) -ceq $originalCorner) 'radial tile and corner stay clean while idle'

    # An activation repaint could paint a system title bar on this shaped window.
    Assert-Probe ([VeloWorkspaceProbe]::RepaintActiveFrame($Panel.Handle)) 'radial window receives the native activation repaint request'
    [void][VeloWorkspaceProbe]::DwmFlush()
    $repaintedCap = [VeloWorkspaceProbe]::Pixel($centerX, $capY)
    $hoverX = $rect.Left + [int](100 * $sx)
    $hoverY = $rect.Top + [int](101 * $sy)
    $unhovered = [VeloWorkspaceProbe]::Pixel($hoverX, $hoverY)
    [VeloWorkspaceProbe]::Move($hoverX, $hoverY)
    Wait-Desktop 'file tile hover paint' { [VeloWorkspaceProbe]::Pixel($hoverX, $hoverY) -ne $unhovered }
    [VeloWorkspaceProbe]::Move($centerX, ($rect.Top + [int](160 * $sy)))
    Wait-Desktop 'file tile hover clears' { [VeloWorkspaceProbe]::Pixel($hoverX, $hoverY) -eq $unhovered }
    Assert-Probe ($repaintedCap -eq $fill) 'native activation keeps the outer tile fill'
    Assert-Probe ((Read-ScreenPatch $corner) -ceq $originalCorner) 'hover repaint leaves the outer tile corner clean'
}

function Start-SidePress {
    Focus-Witness
    [void][VeloWorkspaceProbe]::SetCursorPos($centerX, $centerY)
    [VeloWorkspaceProbe]::Side($true, $side)
}

function Wait-SidePanel {
    Wait-Desktop 'side-button Workspace foreground' {
        $active = [VeloWorkspaceProbe]::GetForegroundWindow()
        $active -ne [IntPtr]::Zero -and $active -eq [VeloWorkspaceProbe]::Find($process.Id, $workspaceClass)
    }
    $handle = [VeloWorkspaceProbe]::GetForegroundWindow()
    [pscustomobject]@{ Handle = $handle; Rect = [VeloWorkspaceProbe]::Bounds($handle) }
}

function Test-FastColdSwipe {
    Focus-Witness
    [void][VeloWorkspaceProbe]::SetCursorPos($centerX, $centerY)
    $launcherWindow = [VeloWorkspaceProbe]::Find($process.Id, $launcherClass)
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($witness.Handle) / 96
    $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
    [VeloWorkspaceProbe]::Swipe($centerX, ($centerY - [int](94 * $scale)), $side)
    Wait-Desktop 'quick swipe dispatched while first Workspace window opens' {
        $events = Read-Events
        $events -match 'event=workspace_fast_release .*window=opening' -and
        $events -match 'event=workspace_action_dispatched .*action=Launcher' -and
        [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow
    }
    $dispatches = [regex]::Matches((Read-Events), '(?m)^event=workspace_action_dispatched .*action=Launcher').Count
    Assert-Probe ($dispatches -eq 1) 'cold side-button swipe dispatches once'
    Close-Panel @{ Handle = $launcherWindow } $escapeKey $false
}

function Test-SideInput {
    $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
    $other = 3 - $side
    $pointX = $centerX
    $upperY = $centerY - [int](94 * $dpi / 96)
    $lowerX = $centerX - [int](48 * $dpi / 96)
    $lowerY = $centerY + [int](139 * $dpi / 96)
    Focus-Witness
    [void][VeloWorkspaceProbe]::SetCursorPos($centerX, $centerY)
    $freshClicks = [VeloWorkspaceProbe]::SideDowns
    $freshReleases = [VeloWorkspaceProbe]::SideUps
    Start-Sleep -Milliseconds 30
    [VeloWorkspaceProbe]::Side($true, $other)
    [VeloWorkspaceProbe]::Side($false, $other)
    Wait-Desktop 'other side button passes to witness hook' { [VeloWorkspaceProbe]::SideDowns -gt $freshClicks -and [VeloWorkspaceProbe]::SideUps -gt $freshReleases }
    Assert-Probe $true 'unbound side button passes through'
    try {
        Start-SidePress
        Start-Sleep -Milliseconds 25
        [VeloWorkspaceProbe]::Side($false, $side)
        $panel = Wait-SidePanel
        Wait-Desktop 'full menu after short press' {
            [VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($lowerX, $lowerY)) -eq $panel.Handle
        }
        Assert-Probe ([VeloWorkspaceProbe]::SideDowns -eq ($freshClicks + 1) -and [VeloWorkspaceProbe]::SideUps -eq ($freshReleases + 1)) 'selected side-button pair consumed'
        Close-Panel $panel $escapeKey $true

        Start-SidePress
        $panel = Wait-SidePanel
        Assert-Probe ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($pointX, $upperY)) -eq $witness.Handle) 'long press shows only hub'
        Save-Panel $panel 'radial-hold'
        [VeloWorkspaceProbe]::Move($pointX, $upperY)
        Wait-Desktop 'upper section and Launcher highlight' {
            [VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($pointX, $upperY)) -eq $panel.Handle -and
            (Read-Events) -match 'event=workspace_side_highlight .*slot=4 '
        }
        Assert-Probe ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($lowerX, $lowerY)) -eq $witness.Handle) 'upper swipe leaves lower section absent'
        Save-Panel $panel 'radial-upper'
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'upper release opens Launcher' {
            [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow -and -not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle)
        }
        Close-Panel @{ Handle = $launcherWindow } $escapeKey $false

        Start-SidePress
        [VeloWorkspaceProbe]::Move($lowerX, $lowerY)
        $panel = Wait-SidePanel
        Wait-Desktop 'fast downward motion selects lower zone' {
            [VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($lowerX, $lowerY)) -eq $panel.Handle -and
            (Read-Events) -match 'event=workspace_side_highlight .*slot=9 '
        }
        Assert-Probe ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($pointX, $upperY)) -eq $witness.Handle) 'lower swipe leaves upper section absent'
        Save-Panel $panel 'radial-lower'
        [VeloWorkspaceProbe]::Move($centerX, $centerY)
        Wait-Desktop 'return to hub removes lower region' {
            [VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($lowerX, $lowerY)) -eq $witness.Handle
        }
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'hub cancels release and returns focus' {
            [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle -and -not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle)
        }
        Assert-Probe ((Read-Events) -match 'event=workspace_side_release .*result=Cancel ') 'hub release cancels'

        Start-SidePress
        $panel = Wait-SidePanel
        Close-Panel $panel $escapeKey $true
        [VeloWorkspaceProbe]::Side($false, $side)
        Start-Sleep -Milliseconds 90
        Assert-Probe (-not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle)) 'Escape consumes the pending release'
    } finally {
        [VeloWorkspaceProbe]::Side($false, $side)
    }
}

function Test-ClosedSideTarget {
    $origin = [Windows.Forms.Form]::new()
    $origin.StartPosition = 'Manual'
    $origin.ShowInTaskbar = $false
    $origin.Text = 'Workspace closed-target probe'
    $origin.Bounds = [Drawing.Rectangle]::new($centerX - 150, $centerY - 110, 300, 220)
    $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
    try {
        $origin.Show()
        $origin.Activate()
        [void][VeloWorkspaceProbe]::SetForegroundWindow($origin.Handle)
        if ([VeloWorkspaceProbe]::GetForegroundWindow() -ne $origin.Handle) {
            Click-TestPoint $centerX $centerY $origin.Handle
        }
        Wait-Desktop 'disposable target is foreground' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $origin.Handle }
        [void][VeloWorkspaceProbe]::SetCursorPos($centerX, $centerY)
        [VeloWorkspaceProbe]::Side($true, $side)
        $panel = Wait-SidePanel
        $upperY = $centerY - [int](94 * $dpi / 96)
        [VeloWorkspaceProbe]::Move($centerX, $upperY)
        Wait-Desktop 'swipe selects a tile above the target' {
            [VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($centerX, $upperY)) -eq $panel.Handle
        }
        $dispatchesBefore = [regex]::Matches((Read-Events), '(?m)^event=workspace_action_dispatched ').Count
        $origin.Close()
        Assert-Probe ([VeloWorkspaceProbe]::GetForegroundWindow() -eq $panel.Handle) 'Workspace remains foreground after target closes'
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'closed target cancels the swipe' { (Read-Events) -match 'event=workspace_hidden .*reason=target_invalid restored=false' }
        Assert-Probe ([regex]::Matches((Read-Events), '(?m)^event=workspace_action_dispatched ').Count -eq $dispatchesBefore) 'release after target closes dispatches no action'
    } finally {
        [VeloWorkspaceProbe]::Side($false, $side)
        $origin.Dispose()
    }
}

function Test-MonitorCrossing {
    $other = $screens | Where-Object DeviceName -ne $screen.DeviceName | Select-Object -First 1
    $workspaceHandle = [VeloWorkspaceProbe]::Find($process.Id, $workspaceClass)
    try {
        $step = 0
        foreach ($display in $other, $screen) {
            $step++
            $area = $display.WorkingArea
            $witness.Bounds = $area
            [Windows.Forms.Application]::DoEvents()
            $witness.Bounds = $area
            $centerX = $area.Left + [int]($area.Width / 2)
            $centerY = $area.Top + [int]($area.Height / 2)
            $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
            $nativeDpi = [VeloWorkspaceProbe]::GetDpiForWindow($panel.Handle)
            $targetDpi = [VeloWorkspaceProbe]::GetDpiForWindow($witness.Handle)
            $rect = $panel.Rect
            $width = $rect.Right - $rect.Left
            $height = $rect.Bottom - $rect.Top
            Assert-Probe ($panel.Handle -eq $workspaceHandle) 'same Workspace HWND survives crossing'
            Write-Output ("crossing={0} rect={1},{2},{3},{4} dpi={5}" -f $step, $rect.Left, $rect.Top, $width, $height, $nativeDpi)
            Assert-Probe ($nativeDpi -eq $targetDpi -and [Math]::Abs($width - 420 * $targetDpi / 96) -le 1 -and
                [Math]::Abs($height - 424 * $targetDpi / 96) -le 1) "crossing $step adopts $targetDpi DPI"
            Assert-Probe ([Math]::Abs($rect.Left + $width / 2 - $centerX) -le 1 -and
                [Math]::Abs($rect.Top + [Math]::Round($height * 210 / 424) - $centerY) -le 1) "crossing $step stays anchored"
            Save-Panel $panel "radial-crossing-$step"
            Close-Panel $panel $escapeKey $true
        }
    } finally {
        $witness.Bounds = $work
    }
}

function Open-RadialSettings {
    $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
    $rect = $panel.Rect
    $editX = $rect.Left + [int](264 * ($rect.Right - $rect.Left) / 420)
    $editY = $rect.Top + [int](226 * ($rect.Bottom - $rect.Top) / 424)
    Click-TestPoint $editX $editY $panel.Handle
    Wait-Desktop 'Launcher opens Workspace settings' {
        [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow
    }
    Start-Sleep -Milliseconds 700
    $settingsPanel = @{ Handle = $launcherWindow; Rect = [VeloWorkspaceProbe]::Bounds($launcherWindow) }
    Save-Panel $settingsPanel 'workspace-settings'
    $settingsPanel
}

function Wait-WorkspaceSave([int]$Attempt) {
    # Reading config.json during MoveFileEx can block its atomic replacement.
    $pattern = '(?m)^event=workspace_settings_finished .*saved=(true|false)'
    Wait-Desktop "Workspace save $Attempt to finish" {
        [regex]::Matches((Read-Events), $pattern).Count -ge $Attempt
    }
    $result = [regex]::Matches((Read-Events), $pattern)[$Attempt - 1]
    Assert-Probe ($result.Groups[1].Value -eq 'true') "Workspace settings commit $Attempt"
}

function Test-WorkspaceDormant {
    Assert-Probe ([VeloWorkspaceProbe]::Find($process.Id, $workspaceClass) -eq [IntPtr]::Zero) 'disabled Workspace has no HWND'
    Assert-Probe ([VeloWorkspaceProbe]::Free(3, $workspaceKeys[-1])) 'disabled Workspace leaves its hotkey free'
    $process.Refresh()
    $threads = @($process.Threads | ForEach-Object { [VeloWorkspaceProbe]::ThreadName($_.Id) })
    Assert-Probe ((@($threads -eq 'velo-hotkey').Count -eq 1) -and
        -not ($threads -like 'velo-workspace-*')) 'disabled Workspace has no hotkey, mouse or application context thread'
    Focus-Witness
    foreach ($side in 1, 2) {
        $downs = [VeloWorkspaceProbe]::SideDowns
        $ups = [VeloWorkspaceProbe]::SideUps
        [VeloWorkspaceProbe]::Side($true, $side)
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop "disabled Workspace passes side button $side" {
            [VeloWorkspaceProbe]::SideDowns -gt $downs -and [VeloWorkspaceProbe]::SideUps -gt $ups
        }
    }
    Assert-Probe ([VeloWorkspaceProbe]::Find($process.Id, $workspaceClass) -eq [IntPtr]::Zero) 'both side buttons pass through without creating Workspace'
}

function Test-RadialSettings {
    $settingsPanel = Open-RadialSettings
    $settingsRect = $settingsPanel.Rect
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($launcherWindow) / 96
    $toggleX = $settingsRect.Left + [int](33 * $scale)
    $toggleY = $settingsRect.Top + [int](195 * $scale)
    $sideX = $settingsRect.Left + [int](103 * $scale)
    $sideY = $settingsRect.Top + [int](266 * $scale)
    $saveX = $settingsRect.Right - [int](66 * $scale)
    $saveY = $settingsRect.Bottom - [int](29 * $scale)
    Click-TestPoint $toggleX $toggleY $launcherWindow
    $configPath = Join-Path $OutputDirectory 'data/config.json'
    $orderedButtons = @($null, 'xbutton2', 'xbutton1')
    for ($index = 0; $index -lt $orderedButtons.Count; $index++) {
        Click-TestPoint $sideX $sideY $launcherWindow
        if ($index -eq 0) { Save-Panel $settingsPanel 'workspace-trigger-options' }
        [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x24))
        for ($step = 0; $step -lt $index; $step++) {
            [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x28))
        }
        [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x0D))
        Start-Sleep -Milliseconds 220
        Click-TestPoint $saveX $saveY $launcherWindow
        Wait-WorkspaceSave ($index + 1)
        $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
        Assert-Probe ($config.workspace.enabled -and $config.workspace.upper.count -eq 6 -and
            $config.workspace.hotkey -eq $workspaceChord -and $config.workspace.mouse -eq $orderedButtons[$index]) "left selector saves side trigger option $index"
    }
    Click-TestPoint $toggleX $toggleY $launcherWindow
    Click-TestPoint $saveX $saveY $launcherWindow
    Wait-WorkspaceSave 4
    $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    Assert-Probe ($config.workspace.enabled -eq $false) 'isolated config disables Workspace'
    Wait-Desktop 'disabled Workspace destroys its HWND' {
        [VeloWorkspaceProbe]::Find($process.Id, $workspaceClass) -eq [IntPtr]::Zero
    }
    Assert-Probe $true 'disabling Workspace destroys window'
    [VeloWorkspaceProbe]::Press($escapeKey)
    Start-Sleep -Milliseconds 160
    Close-Panel $settingsPanel $escapeKey $false
    Test-WorkspaceDormant
}

function Test-RadialSlotEditor {
    $settingsPanel = Open-RadialSettings
    $settingsRect = $settingsPanel.Rect
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($launcherWindow) / 96
    $configPath = Join-Path $OutputDirectory 'data/config.json'
    $px = { param($x) $settingsRect.Left + [int]($x * $scale) }
    $py = { param($y) $settingsRect.Top + [int]($y * $scale) }
    Click-TestPoint (& $px 122) (& $py 347) $launcherWindow
    Start-Sleep -Milliseconds 350
    Click-TestPoint (& $px 122) (& $py 404) $launcherWindow
    Start-Sleep -Milliseconds 450
    Click-TestPoint (& $px 635) (& $py 580) $launcherWindow
    Start-Sleep -Milliseconds 450
    Save-Panel $settingsPanel 'workspace-slots-before'

    # A real pointer drag from Calculator in the palette to the left inner arc.
    $startX = & $px 466
    $startY = & $py 246
    $dropX = & $px 143
    $dropY = & $py 290
    foreach ($point in @(@($startX, $startY), @($dropX, $dropY))) {
        if ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($point[0], $point[1])) -ne $launcherWindow) {
            throw 'The workspace editor fixture is covered.'
        }
    }
    [void][VeloWorkspaceProbe]::SetCursorPos($startX, $startY)
    Wait-Desktop 'pointer is on the fixture action' {
        $cursor = [VeloWorkspaceProbe+Point]::new(0, 0)
        [VeloWorkspaceProbe]::GetCursorPos([ref]$cursor) -and $cursor.X -eq $startX -and $cursor.Y -eq $startY
    }
    Start-Sleep -Milliseconds 180
    [VeloWorkspaceProbe]::Left($true)
    try {
        foreach ($part in 1..6) {
            [void][VeloWorkspaceProbe]::SetCursorPos([int]($startX + ($dropX - $startX) * $part / 6), [int]($startY + ($dropY - $startY) * $part / 6))
            Start-Sleep -Milliseconds 45
        }
    } finally { [VeloWorkspaceProbe]::Left($false) }
    Save-Panel $settingsPanel 'workspace-slots-dropped'
    Wait-Desktop 'drop selects the inner arc' {
        $pixel = [VeloWorkspaceProbe]::Pixel((& $px 158), (& $py 283))
        $pixel -in 0x603a17, 0xfff1e7
    }
    $destinationX = & $px 394
    $destinationY = & $py 573
    if ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($destinationX, $destinationY)) -ne $launcherWindow) {
        throw 'The Workspace destination selector is covered.'
    }
    [void][VeloWorkspaceProbe]::SetCursorPos($destinationX, $destinationY)
    Start-Sleep -Milliseconds 120
    [VeloWorkspaceProbe]::Left($true)
    Start-Sleep -Milliseconds 120
    [VeloWorkspaceProbe]::Left($false)
    Save-Panel $settingsPanel 'workspace-slots-destination-focused'
    [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x0D))
    Save-Panel $settingsPanel 'workspace-slots-edited'
    [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x11, 0x53))
    Wait-WorkspaceSave 1
    Save-Panel $settingsPanel 'workspace-slots-after-save'
    $slots = (Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json).workspace.upper.slots
    Assert-Probe ($slots -and $slots[0] -eq 'calculator' -and $slots[3] -eq 'explorer') 'native drag, keyboard exchange and Ctrl+S persisted both slots'
}

function Get-WorkspaceResources([string]$State) {
    $process.Refresh()
    $cpu0 = $process.TotalProcessorTime.TotalMilliseconds
    $watch = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Milliseconds 750
    $process.Refresh()
    $cpu = 100 * ($process.TotalProcessorTime.TotalMilliseconds - $cpu0) / $watch.Elapsed.TotalMilliseconds
    $workspace = [VeloWorkspaceProbe]::Find($process.Id, $workspaceClass)
    $launcher = [VeloWorkspaceProbe]::Find($process.Id, $launcherClass)
    $tray = [VeloWorkspaceProbe]::Find($process.Id, 'VeloSlintDesktopService')
    $hwnds = @($workspace, $launcher, $tray | Where-Object { $_ -ne [IntPtr]::Zero }).Count
    [ordered]@{ State=$State; PrivateBytes=$process.PrivateMemorySize64; Threads=$process.Threads.Count; Windows=$hwnds
        WorkspaceVisible=[VeloWorkspaceProbe]::IsWindowVisible($workspace); CpuPercentOfCore=$cpu }
}

function Measure-SideCycle {
    $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
    $x = $centerX
    $y = $centerY - [int](94 * $dpi / 96)
    Focus-Witness
    [void][VeloWorkspaceProbe]::SetCursorPos($centerX, $centerY)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    [VeloWorkspaceProbe]::Side($true, $side)
    try {
        $panel = Wait-SidePanel
        Wait-Desktop 'logo painted' {
            $p = [VeloWorkspaceProbe]::Pixel($centerX, $centerY)
            $p -ne 0x675342 -and $p -ne 0 -and $p -ne [uint32]::MaxValue
        }
        $first = $clock.Elapsed.TotalMilliseconds
        $clock.Restart()
        [VeloWorkspaceProbe]::Move($x, $y)
        $selectionX = $x - [int](25 * $dpi / 96)
        Wait-Desktop 'selection fill painted' {
            $p = [VeloWorkspaceProbe]::Pixel($selectionX, $y)
            [VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($x, $y)) -eq $panel.Handle -and
            $p -in 0x603a17, 0xfff1e7
        }
        $highlight = $clock.Elapsed.TotalMilliseconds
        $clock.Restart()
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'dispatch reaches Launcher' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow }
        $dispatch = $clock.Elapsed.TotalMilliseconds
        Close-Panel @{ Handle = $launcherWindow } $escapeKey $false
        [ordered]@{ HoldToFrameMs=$first; MoveToHighlightMs=$highlight; ReleaseToLauncherMs=$dispatch }
    } finally {
        [VeloWorkspaceProbe]::Side($false, $side)
    }
}

$exe = (Resolve-Path -LiteralPath $Executable).Path
$OutputDirectory = (New-Item -ItemType Directory -Path $OutputDirectory).FullName
$eventsPath = Join-Path $OutputDirectory 'measurement.log'
$oldContext = [VeloWorkspaceProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$oldForeground = [VeloWorkspaceProbe]::GetForegroundWindow()
$oldCursor = [VeloWorkspaceProbe+Point]::new(0, 0)
[void][VeloWorkspaceProbe]::GetCursorPos([ref]$oldCursor)
$witness = [Windows.Forms.Form]::new()
$process = $null
$workspaceClass = 'VeloSlintWorkspaceWindow'
$launcherClass = 'VeloSlintLauncherWindow'
$watchSide = $SideButton -or $SettingsRoundTrip -or $Mode -eq 'disabled'
$workspaceChord = 'Ctrl+Alt+Space'
$workspaceKeys = [byte[]]@(0x11, 0x12, 0x20)
$escapeKey = [byte[]]@(0x1B)

try {
    $screens = [Windows.Forms.Screen]::AllScreens
    if ($Monitor -lt 0 -or $Monitor -ge $screens.Count) { throw 'Monitor index is outside the desktop topology.' }
    if ($CrossMonitors -and $screens.Count -lt 2) { throw 'Cross-monitor tests require two active monitors.' }
    $screen = $screens[$Monitor]
    $work = $screen.WorkingArea
    $centerX = $work.Left + [int]($work.Width / 2)
    $centerY = $work.Top + [int]($work.Height / 2)
    $launcherChord = 'Alt+Space'
    $launcherKeys = [byte[]]@(0x12, 0x20)
    if (-not [VeloWorkspaceProbe]::Free(1, 0x20)) {
        $launcherChord = 'Ctrl+Alt+K'
        $launcherKeys = [byte[]]@(0x11, 0x12, 0x4B)
        if (-not [VeloWorkspaceProbe]::Free(3, 0x4B)) {
            $launcherChord = 'Ctrl+Alt+J'
            $launcherKeys = [byte[]]@(0x11, 0x12, 0x4A)
            if (-not [VeloWorkspaceProbe]::Free(3, 0x4A)) { throw 'All launcher test chords are occupied.' }
        }
    }
    if ($Mode -in 'shaped', 'rounded', 'radial' -and -not [VeloWorkspaceProbe]::Free(3, 0x20)) {
        $workspaceChord = 'Ctrl+Alt+W'
        $workspaceKeys = [byte[]]@(0x11, 0x12, 0x57)
        if (-not [VeloWorkspaceProbe]::Free(3, 0x57)) {
            $workspaceChord = 'Ctrl+Alt+F10'
            $workspaceKeys = [byte[]]@(0x11, 0x12, 0x79)
            if (-not [VeloWorkspaceProbe]::Free(3, 0x79)) { throw 'All Workspace test chords are occupied.' }
        }
    }
    $witness.FormBorderStyle = 'None'
    $witness.StartPosition = 'Manual'
    $witness.Bounds = $work
    $witness.BackColor = [Drawing.Color]::FromArgb(66, 83, 103)
    $witness.ShowInTaskbar = $false
    $witness.TopMost = $true
    $clicked = [int[]]@(0)
    $witness.add_MouseDown({ $clicked[0]++ }.GetNewClosure())
    $witness.Show()
    if ($watchSide) { [VeloWorkspaceProbe]::WatchSide() }
    $found = $false
    Wait-Desktop 'unobstructed test surface painted' {
        foreach ($ratio in 0.5, 0.24, 0.76) {
            $x = $work.Left + [int]($work.Width * $ratio)
            $y = $centerY
            if ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($x, $y)) -eq $witness.Handle -and
                [VeloWorkspaceProbe]::Pixel($x, $y) -eq 0x675342) {
                return $true
            }
        }
        return $false
    }
    foreach ($ratio in 0.5, 0.24, 0.76) {
        $x = $work.Left + [int]($work.Width * $ratio)
        if ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($x, $centerY)) -eq $witness.Handle -and
            [VeloWorkspaceProbe]::Pixel($x, $centerY) -eq 0x675342) {
            $centerX = $x
            $found = $true
            break
        }
    }
    if (-not $found) { throw 'The test surface is obstructed.' }
    $witness.TopMost = $false
    Focus-Witness

    $instance = 'workspace-p0-' + [Guid]::NewGuid().ToString('N').Substring(0, 12)
    $arguments = @('--background', '--diagnostics', "--instance-id=$instance", "--hotkey=$launcherChord",
        ('--data-dir="' + (Join-Path $OutputDirectory 'data') + '"'), ('--measurement-log="' + $eventsPath + '"'))
    if ($Mode -in 'shaped', 'rounded', 'radial') {
        $arguments += "--workspace-preview=$Mode", "--workspace-hotkey=$workspaceChord"
    }
    if ($Mode -eq 'disabled' -or $SettingsRoundTrip -or $EditSlots -or $MeasureResources -or $SideSamples) {
        New-Item -ItemType Directory -Path (Join-Path $OutputDirectory 'data') | Out-Null
        $configPath = Join-Path $OutputDirectory 'data/config.json'
        $config = @{ auto_check_updates = $false }
        if ($Mode -ne 'disabled') {
            $config.workspace = @{ enabled = [bool]$EditSlots; hotkey = $workspaceChord }
            if ($SideButton) { $config.workspace.mouse = $SideButton }
        }
        $config | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $configPath -Encoding utf8NoBOM
    }
    if ($SideButton) {
        $arguments += "--workspace-xbutton=$SideButton"
    }
    if ($Mode -eq 'conflict') { $arguments += '--workspace-preview=shaped', "--workspace-hotkey=$launcherChord" }
    $launch = @{
        FilePath = $exe; ArgumentList = $arguments; WindowStyle = 'Hidden'; PassThru = $true
        RedirectStandardError = (Join-Path $OutputDirectory 'stderr.log')
    }
    $process = Start-Process @launch
    Wait-Desktop 'background launcher startup' { (Read-Events) -match 'event=lifecycle_hidden .*reason=startup' } 15000
    $preflight = if ($Mode -eq 'conflict') { 'workspace_hotkey_error' } else { 'workspace_hotkey_ready' }
    if ($Mode -ne 'disabled') { Wait-Desktop $preflight { (Read-Events) -match "event=$preflight " } }
    if ($SideButton) { Wait-Desktop 'opt-in side-button hook installed' { (Read-Events) -match 'event=workspace_mouse_ready ' } }
    Assert-Probe ([VeloWorkspaceProbe]::Find($process.Id, $workspaceClass) -eq [IntPtr]::Zero) 'no Workspace window at startup'
    $launcherTimes = @(foreach ($sample in 1..6) {
        $panel = Open-Panel $launcherClass $launcherKeys $centerX $centerY
        Close-Panel $panel $escapeKey $false
        $panel.Milliseconds
    })
    Assert-Probe $true "launcher responds to $launcherChord and Escape"
    $workspaceTimes = @()
    $resourceStates = @()
    $sideMeasurements = @()
    if ($MeasureResources) {
        $state = if ($Mode -eq 'radial') { 'enabled-not-opened' } else { 'disabled' }
        $resourceStates += Get-WorkspaceResources $state
    }
    if ($Mode -eq 'radial' -and $SideButton) { Test-FastColdSwipe }
    $paintSamples = [Collections.Generic.List[uint32]]::new()
    if ($Mode -in 'shaped', 'rounded') {
        $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
        $workspaceTimes += $panel.Milliseconds
        $rect = $panel.Rect
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $dpi = [VeloWorkspaceProbe]::GetDpiForWindow($panel.Handle)
        Assert-Probe ([Math]::Abs($width - 360 * $dpi / 96) -le 1 -and [Math]::Abs($height - 252 * $dpi / 96) -le 1) 'size follows monitor DPI'
        Assert-Probe ([Math]::Abs($rect.Left + $width / 2 - $centerX) -le 1 -and [Math]::Abs($rect.Top + $height / 2 - $centerY) -le 1) 'hub aligns with the pointer'
        Save-Panel $panel
        Click-TestPoint $centerX ($rect.Top + [int]($height * 0.81)) $panel.Handle
        Wait-Desktop 'lower control responds' { (Read-Events) -match 'event=workspace_highlight .*checked=true' }
        Click-TestPoint $centerX ($rect.Top + [int]($height * 0.19)) $panel.Handle
        Wait-Desktop 'upper control hides and restores focus' {
            -not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle) -and [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle
        }
        foreach ($sample in 1..5) {
            $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
            $workspaceTimes += $panel.Milliseconds
            $closeKeys = if ($sample -eq 1) { $escapeKey } else { $workspaceKeys }
            Close-Panel $panel $closeKeys $true
        }
        Assert-Probe $true 'controls, Escape and repeated hotkey return focus'
        $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
        $hitX = $panel.Rect.Left + 2
        $hitY = if ($Mode -eq 'shaped') { $centerY } else { $panel.Rect.Top + 2 }
        Assert-Probe ([VeloWorkspaceProbe]::Pixel($hitX, $hitY) -eq 0x675342) 'cropped corner or waist shows the test surface'
        $previousClicks = $clicked[0]
        Click-TestPoint $hitX $hitY $witness.Handle
        Wait-Desktop 'click reaches surface and dismisses Workspace' {
            $clicked[0] -gt $previousClicks -and [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle -and
            -not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle)
        }
        Assert-Probe $true 'outside click passes through and keeps its foreground'
        $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
        [VeloWorkspaceProbe]::Press($launcherKeys)
        $launcherWindow = [VeloWorkspaceProbe]::Find($process.Id, $launcherClass)
        Wait-Desktop 'Launcher takes focus from Workspace' {
            [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow -and
            -not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle)
        }
        Close-Panel @{ Handle = $launcherWindow } $escapeKey $false
        Assert-Probe $true 'Launcher keeps its own hotkey while Workspace is open'
        $panel = Open-Panel $workspaceClass $workspaceKeys ($screen.Bounds.Right - 1) ($screen.Bounds.Bottom - 1)
        Assert-Probe ($panel.Rect.Right -eq $work.Right -and $panel.Rect.Bottom -eq $work.Bottom) 'clamped to lower-right working area'
        Close-Panel $panel $escapeKey $true
        $panel = Open-Panel $workspaceClass $workspaceKeys $screen.Bounds.Left $screen.Bounds.Top
        Assert-Probe ($panel.Rect.Left -eq $work.Left -and $panel.Rect.Top -eq $work.Top) 'clamped to upper-left working area'
        Close-Panel $panel $escapeKey $true
    } elseif ($Mode -eq 'radial') {
        $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
        $workspaceTimes += $panel.Milliseconds
        $rect = $panel.Rect
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $dpi = [VeloWorkspaceProbe]::GetDpiForWindow($panel.Handle)
        Assert-Probe ([Math]::Abs($width - 420 * $dpi / 96) -le 1 -and [Math]::Abs($height - 424 * $dpi / 96) -le 1) 'radial board follows monitor DPI'
        Assert-Probe ([Math]::Abs($rect.Left + $width / 2 - $centerX) -le 1 -and [Math]::Abs($rect.Top + [math]::Round(210 * $height / 424) - $centerY) -le 1) 'radial hub aligns with pointer'
        Save-Panel $panel
        Test-RadialFrame $panel
        $gapY = $rect.Top + [int](160 * $height / 424)
        if ($MeasureResources) { $resourceStates += Get-WorkspaceResources 'visible' }
        $previousClicks = $clicked[0]
        Assert-Probe ([VeloWorkspaceProbe]::Pixel($centerX, $gapY) -eq 0x675342) 'radial gap shows underlying window'
        Click-TestPoint $centerX $gapY $witness.Handle
        Wait-Desktop 'gap click delivered and radial menu dismissed' {
            $clicked[0] -gt $previousClicks -and [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle -and
            -not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle)
        }
        Assert-Probe $true 'radial gap click passes through'
        $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
        Click-TestPoint $centerX ($rect.Top + [int](116 * $height / 424)) $panel.Handle
        $launcherWindow = [VeloWorkspaceProbe]::Find($process.Id, $launcherClass)
        Wait-Desktop 'radial Launcher action' {
            [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow -and
            -not [VeloWorkspaceProbe]::IsWindowVisible($panel.Handle)
        }
        Close-Panel @{ Handle = $launcherWindow } $escapeKey $false
        Assert-Probe $true 'radial tile opens Launcher'
        foreach ($sample in 1..5) {
            $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
            $workspaceTimes += $panel.Milliseconds
            Close-Panel $panel $escapeKey $true
        }
        $panel = Open-Panel $workspaceClass $workspaceKeys ($screen.Bounds.Right - 1) ($screen.Bounds.Bottom - 1)
        Assert-Probe ($panel.Rect.Right -eq $work.Right -and $panel.Rect.Bottom -eq $work.Bottom) 'radial board clamps at lower-right working area'
        Close-Panel $panel $workspaceKeys $true
        $panel = Open-Panel $workspaceClass $workspaceKeys $screen.Bounds.Left $screen.Bounds.Top
        Assert-Probe ($panel.Rect.Left -eq $work.Left -and $panel.Rect.Top -eq $work.Top) 'radial board clamps at upper-left working area'
        Close-Panel $panel $escapeKey $true
        if ($CrossMonitors) { Test-MonitorCrossing }
        if ($SideButton) { Test-SideInput }
        if ($SideSamples) { $sideMeasurements = @(foreach ($sample in 1..$SideSamples) { Measure-SideCycle }) }
        if ($MeasureResources) { $resourceStates += Get-WorkspaceResources 'used-hidden' }
        if ($SideButton) { Test-ClosedSideTarget }
        if ($EditSlots) { Test-RadialSlotEditor }
        if ($SettingsRoundTrip) { Test-RadialSettings }
    } elseif ($Mode -eq 'disabled') {
        Assert-Probe ((Read-Events) -notmatch 'event=workspace_(hotkey_ready|mouse_ready|shown) ') 'fresh config never starts Workspace'
        Test-WorkspaceDormant
    }

    Start-Sleep -Seconds 2
    $process.Refresh()
    $cpuStart = $process.TotalProcessorTime.TotalMilliseconds
    $idle = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds 3
    $process.Refresh()
    $cpuMs = $process.TotalProcessorTime.TotalMilliseconds - $cpuStart
    $startup = [regex]::Match((Read-Events), '(?m)^event=lifecycle_hidden .*?elapsed_us=(\d+).*reason=startup')
    $result = [ordered]@{
        Mode = $Mode
        ExecutableSHA256 = (Get-FileHash -LiteralPath $exe).Hash
        WindowsBuild = [Environment]::OSVersion.Version.Build
        MonitorCount = $screens.Count
        MonitorDevice = $screen.DeviceName
        MonitorPrimary = $screen.Primary
        MonitorBounds = "$($screen.Bounds.X),$($screen.Bounds.Y),$($screen.Bounds.Width),$($screen.Bounds.Height)"
        EffectiveDpi = [VeloWorkspaceProbe]::GetDpiForWindow($witness.Handle)
        WorkArea = "$($work.X),$($work.Y),$($work.Width),$($work.Height)"
        LauncherHotkey = $launcherChord
        WorkspaceHotkey = $workspaceChord
        SideButton = $SideButton
        ResourceStates = $resourceStates
        SideMeasurements = $sideMeasurements
        BackgroundReadyMs = [double]$startup.Groups[1].Value / 1000
        LauncherScreenMs = $launcherTimes
        WorkspaceScreenMs = $workspaceTimes
        WorkspacePaintSamples = $paintSamples.Count
        WorkspaceBlankSamples = @($paintSamples | Where-Object { $_ -in 0, 0x675342, [uint32]::MaxValue }).Count
        IdlePrivateBytes = $process.PrivateMemorySize64
        IdleCpuMs = $cpuMs
        IdleWallMs = $idle.Elapsed.TotalMilliseconds
    }
    $result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.json') -Encoding utf8
    $result | ConvertTo-Json -Depth 5
} finally {
    if ($process) {
        if (-not $process.HasExited) {
            $tray = [VeloWorkspaceProbe]::Find($process.Id, 'VeloSlintDesktopService')
            try {
                # Use the disposable instance's menu and verify ownership before clicking Exit.
                [void][VeloWorkspaceProbe]::AllowSetForegroundWindow($process.Id)
                [void][VeloWorkspaceProbe]::SetCursorPos($centerX, $centerY)
                [void][VeloWorkspaceProbe]::PostMessage($tray, 0x8014, [IntPtr]::Zero, [IntPtr]0x7B)
                Wait-Desktop 'owned tray menu' {
                    $menu = [VeloWorkspaceProbe]::Find($process.Id, '#32768')
                    $owner = [uint32]0
                    [void][VeloWorkspaceProbe]::GetWindowThreadProcessId([VeloWorkspaceProbe]::GetForegroundWindow(), [ref]$owner)
                    $owner -eq $process.Id -and [VeloWorkspaceProbe]::IsWindowVisible($menu)
                } 3500
                $menu = [VeloWorkspaceProbe]::Find($process.Id, '#32768')
                $item = [VeloWorkspaceProbe]::TrayExitBounds($menu)
                $itemX = [int](($item.Left + $item.Right) / 2)
                $itemY = [int](($item.Top + $item.Bottom) / 2)
                [VeloWorkspaceProbe]::Move($itemX, $itemY)
                Start-Sleep -Milliseconds 200
                Click-TestPoint $itemX $itemY $menu
                Wait-Desktop 'native tray quit received' { (Read-Events) -match '(?m)^event=desktop_event .*action=3\b' }
                if (!$process.WaitForExit(8000)) { throw 'Tray exit timed out after native quit.' }
                Write-Output "exit via tray; code=$($process.ExitCode)"
                if ($Mode -in 'shaped', 'rounded', 'radial') {
                    Assert-Probe ([VeloWorkspaceProbe]::Free(3, $workspaceKeys[-1])) 'Workspace hotkey released at exit'
                }
            } catch {
                Write-Warning "Tray exit could not be verified. Cleaning up test PID $($process.Id). $($_.Exception.Message)"
                if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
            }
        }
        $process.Dispose()
    }
    if ($witness.IsHandleCreated -and [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle -and $oldForeground -ne [IntPtr]::Zero) {
        [void][VeloWorkspaceProbe]::SetForegroundWindow($oldForeground)
    }
    if ($watchSide -and $witness.IsHandleCreated) { [VeloWorkspaceProbe]::UnwatchSide() }
    $witness.Dispose()
    [void][VeloWorkspaceProbe]::SetCursorPos($oldCursor.X, $oldCursor.Y)
    [void][VeloWorkspaceProbe]::SetThreadDpiAwarenessContext($oldContext)
}
