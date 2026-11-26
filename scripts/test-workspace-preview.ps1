#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateSet('dual', 'legacy', 'disabled', 'conflict')][string]$Mode = 'dual',
    [int]$Monitor = 0,
    [switch]$CrossMonitors,
    [ValidateSet('xbutton1', 'xbutton2')][string]$SideButton,
    [switch]$MeasureResources,
    [ValidateSet('light', 'dark')][string]$Theme = 'light',
    [ValidateRange(0, 100)][int]$SideSamples = 0
)

$ErrorActionPreference = 'Stop'
if (($SideButton -or $CrossMonitors) -and $Mode -notin 'dual', 'legacy') {
    throw 'Side-button and cross-monitor checks require an active Workspace.'
}
if ($SideSamples -and (!$SideButton -or $Mode -ne 'dual')) { throw '-SideSamples requires -Mode dual and -SideButton.' }
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
    public static void Side(bool down, uint button) {
        Inject(new MouseInput { Data = button, Flags = down ? 0x80u : 0x100u, Extra = Marker });
    }
    public static void Move(int x, int y) {
        Inject(Motion(x, y));
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
    public static void Drag(int x, int y, int dx, int dy) {
        Move(x, y);
        System.Threading.Thread.Sleep(25);
        Inject(new MouseInput { Flags = 0x0002 });
        try {
            System.Threading.Thread.Sleep(90);
            Move(x + dx / 2, y + dy / 2);
            System.Threading.Thread.Sleep(60);
            Move(x + dx, y + dy);
            System.Threading.Thread.Sleep(60);
        } finally {
            Inject(new MouseInput { Flags = 0x0004 });
        }
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
    $sampleX = $rect.Left + [int](($rect.Right - $rect.Left) * 0.28)
    $sampleY = $rect.Top + [int](($rect.Bottom - $rect.Top) * 0.12)
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

function Test-DualInput {
    $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($witness.Handle) / 96
    $window = $null
    $countDispatches = { [regex]::Matches((Read-Events), '(?m)^event=workspace_action_dispatched ').Count }
    $initialDispatches = & $countDispatches
    try {
        Start-SidePress
        Start-Sleep -Milliseconds 65
        [VeloWorkspaceProbe]::Side($false, $side)
        $panel = Wait-SidePanel
        $window = $panel.Handle
        Assert-Probe (($panel.Rect.Right - $panel.Rect.Left) -eq [int](320 * $scale)) 'short press presents the three-column Panel'
        Wait-Desktop 'matched pointer application on short press' { (Read-Events) -match 'event=workspace_dual_shown .*mode=panel matched=true' }
        Save-Panel $panel "$Theme-panel"
        Close-Panel $panel $escapeKey $true
        Start-SidePress
        Wait-Desktop 'passive hold center' { (Read-Events) -match 'event=workspace_dual_shown .*mode=center matched=true' }
        Assert-Probe ([VeloWorkspaceProbe]::IsWindowVisible($window) -and [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle) 'hold retains original foreground and reuses the HWND'
        Save-Panel @{ Handle = $window; Rect = [VeloWorkspaceProbe]::Bounds($window) } "$Theme-center"
        [VeloWorkspaceProbe]::Move($centerX, ($centerY - [int](44 * $scale)))
        Wait-Desktop 'north action highlighted' { (Read-Events) -match 'phase=Selecting\(North\) effective=Some\(1\)' }
        Save-Panel @{ Handle = $window; Rect = [VeloWorkspaceProbe]::Bounds($window) } "$Theme-pad"
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'one gesture opens Launcher' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow }
        Wait-Desktop 'north action dispatch receipt' { (& $countDispatches) -ge $initialDispatches + 1 }
        Assert-Probe ((& $countDispatches) -eq $initialDispatches + 1) 'north executes exactly once'
        Close-Panel @{ Handle = $launcherWindow } $escapeKey $false

        foreach ($cancel in 'dead zone', 'empty direction', 'Escape') {
            Start-SidePress
            $priorMoves = [regex]::Matches((Read-Events), 'phase=Selecting\(North\)').Count
            [VeloWorkspaceProbe]::Move($centerX, ($centerY - [int](40 * $scale)))
            Wait-Desktop 'fresh north direction' { [regex]::Matches((Read-Events), 'phase=Selecting\(North\)').Count -gt $priorMoves }
            switch ($cancel) {
                'dead zone' { [VeloWorkspaceProbe]::Move($centerX, $centerY) }
                'empty direction' { [VeloWorkspaceProbe]::Move(($centerX + [int](40 * $scale)), $centerY) }
                'Escape' { [VeloWorkspaceProbe]::Press($escapeKey) }
            }
            Start-Sleep -Milliseconds 34
            [VeloWorkspaceProbe]::Side($false, $side)
            Wait-Desktop "dismissed upon $cancel" { -not [VeloWorkspaceProbe]::IsWindowVisible($window) }
            Assert-Probe ((& $countDispatches) -eq $initialDispatches + 1) "$cancel cancels without an action"
        }

        [void]$witnessText.Focus()
        Start-SidePress
        [VeloWorkspaceProbe]::Move(($centerX - [int](42 * $scale)), $centerY)
        Start-Sleep -Milliseconds 35
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'Unicode text in original edit control' { $witnessText.Text -match 'Velo.*中文' -and $witnessText.Text -match 'second line' }
        if ($witnessText.Text -ne "Velo 中文`r`nsecond line" -and $witnessText.Text -ne "Velo 中文`nsecond line") {
            $codes = $witnessText.Text.ToCharArray() | ForEach-Object { [int]$_ }
            throw "Unexpected character codes in synthetic text: $codes"
        }
        Assert-Probe $true 'text preserves Unicode and a line break'

        $before = $witnessText.Text
        $typed = if ([Windows.Forms.Control]::IsKeyLocked([Windows.Forms.Keys]::CapsLock)) { 'xz' } else { 'XZ' }
        Start-SidePress
        [VeloWorkspaceProbe]::Move($centerX, ($centerY + [int](43 * $scale)))
        Start-Sleep -Milliseconds 35
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'two native chord steps' { (Read-Events) -match 'kind=SendKeys result=Ok\(ShortcutSubmitted\)' }
        Wait-Desktop 'original edit control sees both steps in order' { $witnessText.Text -ceq ($before + $typed) }
        Assert-Probe $true 'two-step shortcut reaches the original edit control'

        Start-SidePress
        Start-Sleep -Milliseconds 60
        [VeloWorkspaceProbe]::Side($false, $side)
        $panel = Wait-SidePanel
        Start-Sleep -Milliseconds 80
        $rect = $panel.Rect
        Click-TestPoint ($rect.Right - [int](29 * $scale)) ($rect.Top + [int](170 * $scale)) $window
        Wait-Desktop 'settings owns the Launcher window' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow }
        $settings = @{ Handle = $launcherWindow; Rect = [VeloWorkspaceProbe]::Bounds($launcherWindow) }
        Wait-Desktop 'settings surface painted' {
            $pixel = [VeloWorkspaceProbe]::Pixel(($settings.Rect.Left + [int](27 * $scale)), ($settings.Rect.Top + [int](151 * $scale)))
            $pixel -notin 0, 0x675342, [uint32]::MaxValue
        }
        Save-Panel $settings "$Theme-gesture-settings"
        [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x11, 0x53))
        Wait-WorkspaceSave 1
        $saved = (Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json).workspace
        Assert-Probe ($saved.enabled -and $saved.hotkey -eq $workspaceChord -and
            $saved.dual.gesture_profiles[0].directions.slots[6] -eq 'launcher' -and
            $saved.dual.panel_profiles[0].matrix.slots[3] -eq 'action:text') 'save retains both layouts and the live preview binding'
        Save-Panel $settings "$Theme-gesture-settings-saved"
        [VeloWorkspaceProbe]::Press($escapeKey)
        Start-Sleep -Milliseconds 160
        Close-Panel $settings $escapeKey $false
        $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
        Close-Panel $panel $escapeKey $true
    } finally {
        [VeloWorkspaceProbe]::Side($false, $side)
    }
}

function Test-DualContexts {
    # Separate executable identities let the probe detect swapped context rules.
    $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($witness.Handle) / 96
    $position = [VeloWorkspaceProbe+Point]::new(($work.Left + 280), ($work.Top + 255))
    $source = @"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
`$form = [Windows.Forms.Form]::new()
`$form.ShowInTaskbar = `$false
`$form.TopMost = `$true
`$form.StartPosition = 'Manual'
`$form.Bounds = [Drawing.Rectangle]::new($($work.Left + 200), $($work.Top + 200), 166, 117)
`$form.Text = 'Velo context fixture'
[Windows.Forms.Application]::Run(`$form)
"@
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($source))
    $fixture = Start-Process -FilePath $contextExe -ArgumentList '-NoProfile','-STA','-EncodedCommand',$encoded -WindowStyle Hidden -PassThru
    try {
        Wait-Desktop 'secondary context fixture' {
            $hit = [VeloWorkspaceProbe]::WindowFromPoint($position)
            $owner = [uint32]0
            [void][VeloWorkspaceProbe]::GetWindowThreadProcessId($hit, [ref]$owner)
            $owner -eq $fixture.Id
        } 10000
        Focus-Witness
        [void][VeloWorkspaceProbe]::SetCursorPos($position.X, $position.Y)
        [VeloWorkspaceProbe]::Side($true, $side)
        Start-Sleep -Milliseconds 85
        [VeloWorkspaceProbe]::Side($false, $side)
        $panel = Wait-SidePanel
        Assert-Probe (($panel.Rect.Right - $panel.Rect.Left) -eq [int](488 * $scale)) 'short press uses the separate hovered five-column profile'
        Save-Panel $panel "$Theme-pointer-context"
        Close-Panel $panel $escapeKey $true

        $previous = [regex]::Matches((Read-Events), 'mode=pad matched=true').Count
        [void][VeloWorkspaceProbe]::SetCursorPos($position.X, $position.Y)
        [VeloWorkspaceProbe]::Side($true, $side)
        [VeloWorkspaceProbe]::Move($position.X, ($position.Y - [int](44 * $scale)))
        Wait-Desktop 'foreground gesture profile' { [regex]::Matches((Read-Events), 'mode=pad matched=true').Count -gt $previous }
        Assert-Probe ([VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle) 'long press keeps focus on the separate foreground application'
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'gesture from the foreground profile opens Launcher' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow }
        Close-Panel @{ Handle = $launcherWindow } $escapeKey $false
    } finally {
        [VeloWorkspaceProbe]::Side($false, $side)
        if (-not $fixture.HasExited) { $fixture.Kill(); $fixture.WaitForExit() }
        $fixture.Dispose()
    }
}

function Measure-DualCycle {
    $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($witness.Handle) / 96
    $x = $centerX - [int](20 * $scale)
    $panelY = $centerY - [int](110 * $scale)
    $padY = $centerY - [int](87 * $scale)
    $baseline = [VeloWorkspaceProbe]::Pixel($x, $panelY)
    $highlight = if ($Theme -eq 'dark') { 0x603a17 } else { 0xfff1e7 }
    try {
        Start-SidePress
        Start-Sleep -Milliseconds 65
        $watch = [Diagnostics.Stopwatch]::StartNew()
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'warm Panel first frame' { [VeloWorkspaceProbe]::Pixel($x, $panelY) -ne $baseline }
        $panelMs = $watch.Elapsed.TotalMilliseconds
        Close-Panel (Wait-SidePanel) $escapeKey $true

        Start-SidePress
        $watch.Restart()
        [VeloWorkspaceProbe]::Move($centerX, ($centerY - [int](44 * $scale)))
        Wait-Desktop 'warm Gesture Pad highlight frame' { [VeloWorkspaceProbe]::Pixel($x, $padY) -eq $highlight }
        $highlightMs = $watch.Elapsed.TotalMilliseconds
        $prior = [regex]::Matches((Read-Events), 'action=Launcher input_us=').Count
        [VeloWorkspaceProbe]::Side($false, $side)
        Wait-Desktop 'gesture dispatch receipt' { [regex]::Matches((Read-Events), 'action=Launcher input_us=').Count -gt $prior }
        Wait-Desktop 'gesture opens Launcher' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow }
        Close-Panel @{ Handle = $launcherWindow } $escapeKey $false
        $events = Read-Events
        $selection = [regex]::Matches($events, 'phase=Selecting\(North\) effective=Some\(1\) input_us=(\d+)') | Select-Object -Last 1
        $dispatch = [regex]::Matches($events, 'action=Launcher input_us=(\d+)') | Select-Object -Last 1
        [pscustomobject]@{
            ReleaseToPanelFrameMs = $panelMs
            MoveToHighlightMs = $highlightMs
            MoveToSelectionUs = [int]$selection.Groups[1].Value
            ReleaseToLauncherDispatchUs = [int]$dispatch.Groups[1].Value
        }
    } finally {
        [VeloWorkspaceProbe]::Side($false, $side)
    }
}

function Test-PanelControls($Panel) {
    $rect = $Panel.Rect
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($Panel.Handle) / 96
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    Assert-Probe ([Math]::Abs($width - $panelWidth * $scale) -le 1 -and
        [Math]::Abs($height - 348 * $scale) -le 1) 'Workspace opens the matrix at the monitor DPI'
    Assert-Probe ([Math]::Abs($rect.Left + $width / 2 - $centerX) -le 1 -and
        [Math]::Abs($rect.Top + [Math]::Round(172 * $scale) - $centerY) -le 1) 'the control strip anchors at the pointer'
    $outline = if ($Theme -eq 'dark') { 0x785e4a } else { 0xd6c5b7 }
    $edges = @(
        [VeloWorkspaceProbe]::Pixel($centerX, $rect.Top)
        [VeloWorkspaceProbe]::Pixel($centerX, ($rect.Bottom - 1))
        [VeloWorkspaceProbe]::Pixel($rect.Left, $centerY)
        [VeloWorkspaceProbe]::Pixel(($rect.Right - 1), $centerY)
    )
    Assert-Probe (@($edges | Where-Object { $_ -ne $outline }).Count -eq 0) 'all four Panel edges show the theme outline'

    $dx = [int](80 * $scale)
    $dy = [int](50 * $scale)
    [VeloWorkspaceProbe]::Drag(($rect.Left + [int](72 * $scale)), ($rect.Top + [int](17 * $scale)), $dx, $dy)
    Wait-Desktop 'dragging the Panel header moves its HWND with the pointer' {
        $moved = [VeloWorkspaceProbe]::Bounds($Panel.Handle)
        [Math]::Abs($moved.Left - $rect.Left - $dx) -le 2 -and
            [Math]::Abs($moved.Top - $rect.Top - $dy) -le 2
    }
    $moved = [VeloWorkspaceProbe]::Bounds($Panel.Handle)
    Assert-Probe ([VeloWorkspaceProbe]::GetForegroundWindow() -eq $Panel.Handle -and
        $moved.Right - $moved.Left -eq $width -and $moved.Bottom - $moved.Top -eq $height) 'dragging preserves Panel focus and size'

    $outsideX = $moved.Left - 18
    $outsideY = $centerY + $moved.Top - $rect.Top
    Assert-Probe ([VeloWorkspaceProbe]::WindowFromPoint([VeloWorkspaceProbe+Point]::new($outsideX, $outsideY)) -eq $witness.Handle) 'click outside the Panel hits the original application'
    Click-TestPoint $outsideX $outsideY $witness.Handle
    Wait-Desktop 'outside click dismisses the Panel and restores the underlying target' {
        -not [VeloWorkspaceProbe]::IsWindowVisible($Panel.Handle) -and
            [VeloWorkspaceProbe]::GetForegroundWindow() -eq $witness.Handle
    }
    $corner = Open-Panel $workspaceClass $workspaceKeys ($screen.Bounds.Right - 1) ($screen.Bounds.Bottom - 1)
    Assert-Probe ($corner.Rect.Right -eq $work.Right -and $corner.Rect.Bottom -eq $work.Bottom) 'Panel stays inside the lower-right work area'
    Close-Panel $corner $workspaceKeys $true
    $corner = Open-Panel $workspaceClass $workspaceKeys $screen.Bounds.Left $screen.Bounds.Top
    Assert-Probe ($corner.Rect.Left -eq $work.Left -and $corner.Rect.Top -eq $work.Top) 'Panel stays inside the upper-left work area'
    Close-Panel $corner $escapeKey $true
}

function Test-WorkspaceUpgrade {
    $dpi = [VeloWorkspaceProbe]::GetDpiForWindow($witness.Handle)
    Assert-Probe ((Get-FileHash -LiteralPath $configPath).Hash -eq $originalHash) 'startup uses migrated settings without rewriting the old file'
    if ($SideButton) {
        $side = if ($SideButton -eq 'xbutton1') { 1 } else { 2 }
        $before = [regex]::Matches((Read-Events), 'event=workspace_action_dispatched ').Count
        try {
            Start-SidePress
            [VeloWorkspaceProbe]::Move($centerX, ($centerY - [int](50 * $dpi / 96)))
            Wait-Desktop 'legacy upgrade selects an unassigned direction' {
                (Read-Events) -match 'phase=Selecting\(North\) effective=None'
            }
            [VeloWorkspaceProbe]::Side($false, $side)
            Wait-Desktop 'empty gesture closes' {
                (Read-Events) -match 'event=workspace_hidden .*reason=empty_direction'
            }
            Assert-Probe ([regex]::Matches((Read-Events), 'event=workspace_action_dispatched ').Count -eq $before) 'previous actions do not become gesture bindings'
        } finally {
            [VeloWorkspaceProbe]::Side($false, $side)
        }
    }

    $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($panel.Handle) / 96
    Click-TestPoint ($panel.Rect.Right - [int](29 * $scale)) ($panel.Rect.Top + [int](170 * $scale)) $panel.Handle
    Wait-Desktop 'upgraded Workspace settings' { [VeloWorkspaceProbe]::GetForegroundWindow() -eq $launcherWindow }
    $settings = @{ Handle = $launcherWindow; Rect = [VeloWorkspaceProbe]::Bounds($launcherWindow) }
    Wait-Desktop 'upgraded Workspace settings paint' {
        $pixel = [VeloWorkspaceProbe]::Pixel(($settings.Rect.Left + [int](27 * $scale)), ($settings.Rect.Top + [int](151 * $scale)))
        $pixel -notin 0, 0x675342, [uint32]::MaxValue
    }
    Save-Panel $settings 'legacy-gesture-settings'
    [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x11, 0x53))
    Wait-WorkspaceSave 1
    $saved = (Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json).workspace
    Assert-Probe ($saved.enabled -and $saved.hotkey -eq $workspaceChord -and
        $saved.dual.global.columns -eq 4 -and $saved.dual.global.slots[7] -eq 'pictures' -and
        $saved.dual.default_panel.slots[4] -eq 'this-pc' -and
        $saved.dual.default_gestures.slots.Count -eq 8 -and @($saved.dual.default_gestures.slots | Where-Object { $_ }).Count -eq 0) 'the first save persists both migrated slot groups and empty gestures'
    Assert-Probe (@($saved.PSObject.Properties.Name | Where-Object { $_ -in 'upper','default','profiles','dual_enabled' }).Count -eq 0) 'saved settings contain only the current layout fields'
    $backups = @(Get-ChildItem -LiteralPath (Join-Path $OutputDirectory 'data/backups') -File)
    Assert-Probe ($backups.Count -eq 1 -and (Get-FileHash -LiteralPath $backups[0].FullName).Hash -eq $originalHash) 'first save backs up the original configuration bytes'

    $scale = [VeloWorkspaceProbe]::GetDpiForWindow($launcherWindow) / 96
    Click-TestPoint ($settings.Rect.Left + [int](91 * $scale)) ($settings.Rect.Top + [int](205 * $scale)) $launcherWindow
    Click-TestPoint ($settings.Rect.Left + [int](33 * $scale)) ($settings.Rect.Top + [int](265 * $scale)) $launcherWindow
    [VeloWorkspaceProbe]::PressEditor([byte[]]@(0x11, 0x53))
    Wait-WorkspaceSave 2
    $saved = (Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json).workspace
    Assert-Probe (!$saved.enabled) 'the only Workspace can still be disabled in Settings'
    [VeloWorkspaceProbe]::Press($escapeKey)
    Start-Sleep -Milliseconds 160
    Close-Panel $settings $escapeKey $false
    Test-WorkspaceDormant
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
            Assert-Probe ($nativeDpi -eq $targetDpi -and [Math]::Abs($width - $panelWidth * $targetDpi / 96) -le 1 -and
                [Math]::Abs($height - 348 * $targetDpi / 96) -le 1) "crossing $step adopts $targetDpi DPI"
            Assert-Probe ([Math]::Abs($rect.Left + $width / 2 - $centerX) -le 1 -and
                [Math]::Abs($rect.Top + [Math]::Round(172 * $targetDpi / 96) - $centerY) -le 1) "crossing $step stays anchored"
            Save-Panel $panel "workspace-crossing-$step"
            Close-Panel $panel $escapeKey $true
        }
    } finally {
        $witness.Bounds = $work
    }
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
$watchSide = $SideButton -or $Mode -in 'disabled', 'legacy'
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
    if ($Mode -ne 'conflict' -and -not [VeloWorkspaceProbe]::Free(3, 0x20)) {
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
    if ($Mode -eq 'dual') {
        $witnessText = [Windows.Forms.TextBox]::new()
        $witnessText.Multiline = $true
        $witnessText.ImeMode = [Windows.Forms.ImeMode]::Disable
        $witnessText.Bounds = [Drawing.Rectangle]::new(20, 20, 320, 108)
        $witness.Controls.Add($witnessText)
    }
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

    $instance = 'workspace-test-' + [Guid]::NewGuid().ToString('N').Substring(0, 12)
    $arguments = @('--background', '--diagnostics', "--theme=$Theme", "--instance-id=$instance", "--hotkey=$launcherChord",
        ('--data-dir="' + (Join-Path $OutputDirectory 'data') + '"'), ('--measurement-log="' + $eventsPath + '"'))
    if ($Mode -eq 'dual') {
        $arguments += '--workspace-preview=dual', "--workspace-hotkey=$workspaceChord"
        if ($SideButton) { $arguments += "--workspace-xbutton=$SideButton" }
    } elseif ($Mode -eq 'conflict') {
        $arguments += '--workspace-preview=dual', "--workspace-hotkey=$launcherChord"
    }
    New-Item -ItemType Directory -Path (Join-Path $OutputDirectory 'data') | Out-Null
    $configPath = Join-Path $OutputDirectory 'data/config.json'
    $config = @{ auto_check_updates = $false; theme = $Theme }
    if ($Mode -eq 'legacy') {
        $config.workspace = @{
            enabled = $true; hotkey = $workspaceChord; dual_enabled = $false
            upper = @{ count = 8; slots = @('explorer', 'calculator', 'task-manager', $null, 'launcher', 'control-panel', $null, 'pictures') }
            default = @{ count = 4; slots = @('downloads', 'documents', 'pictures', $null, 'this-pc', $null, $null, $null) }
        }
        if ($SideButton) { $config.workspace.mouse = $SideButton }
    }
    if ($Mode -eq 'dual') {
        $config.workspace = @{ enabled = $false; hotkey = $workspaceChord }
        $id = @{ kind = 'executable'; value = [Environment]::ProcessPath }
        $contextExe = Join-Path $env:SystemRoot 'System32/WindowsPowerShell/v1.0/powershell.exe'
        $config.workspace.dual = @{
            global = @{ columns = 3; slots = @('explorer', 'calculator', 'task-manager', $null, 'launcher', 'control-panel') }
            default_panel = @{ columns = 3; slots = @('downloads', 'documents', 'pictures') }
            panel_profiles = @(
                @{ app = $id; name = '测试编辑器'; matrix = @{ columns = 3; slots = @('documents', 'downloads', 'this-pc', 'action:text', 'action:keys', $null) } }
                @{ app = @{ kind = 'executable'; value = $contextExe }; name = '指针下应用'; matrix = @{ columns = 5; slots = @('launcher', 'calculator') } }
            )
            gesture_profiles = @(@{ app = $id; name = '测试编辑器'; directions = @{ slots = @($null, $null, 'action:keys', $null, 'action:text', $null, 'launcher', $null) } })
            actions = @{
                'action:text' = @{ label = '输入示例'; action = @{ type = 'insert_text'; text = "Velo 中文`r`nsecond line" } }
                'action:keys' = @{ label = '两步输入'; action = @{ type = 'send_keys'; steps = @(@{ chord = 'Shift+X' }, @{ chord = 'Shift+Z'; wait_ms = 70 }) } }
            }
        }
    }
    $config | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $configPath -Encoding utf8NoBOM
    $originalHash = (Get-FileHash -LiteralPath $configPath).Hash
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
        $state = if ($Mode -in 'dual', 'legacy') { 'enabled-not-opened' } else { 'disabled' }
        $resourceStates += Get-WorkspaceResources $state
    }
    $paintSamples = [Collections.Generic.List[uint32]]::new()
    if ($Mode -in 'dual', 'legacy') {
        $panelWidth = if ($Mode -eq 'legacy') { 394 } else { 320 }
        $launcherWindow = [VeloWorkspaceProbe]::Find($process.Id, $launcherClass)
        $panel = Open-Panel $workspaceClass $workspaceKeys $centerX $centerY
        $workspaceTimes += $panel.Milliseconds
        if ($MeasureResources) { $resourceStates += Get-WorkspaceResources 'visible' }
        Save-Panel $panel "$Theme-keyboard-panel"
        Test-PanelControls $panel
        if ($CrossMonitors) { Test-MonitorCrossing }
        if ($Mode -eq 'legacy') { Test-WorkspaceUpgrade }
        if ($Mode -eq 'dual' -and $SideButton) {
            Test-DualInput
            Test-DualContexts
        }
        if ($SideSamples) {
            $sideMeasurements = @(foreach ($sample in 1..$SideSamples) { Measure-DualCycle })
        }
        if ($MeasureResources) { $resourceStates += Get-WorkspaceResources 'used-hidden' }
    } elseif ($Mode -eq 'disabled') {
        Assert-Probe ((Read-Events) -notmatch 'event=workspace_(hotkey_ready|mouse_ready|shown) ') 'fresh config never starts Workspace'
        Test-WorkspaceDormant
    } else {
        Assert-Probe ((Read-Events) -notmatch 'event=workspace_hotkey_ready ') 'a conflict keeps the conflicting Workspace chord inactive'
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
                if ($Mode -in 'dual', 'legacy') {
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
