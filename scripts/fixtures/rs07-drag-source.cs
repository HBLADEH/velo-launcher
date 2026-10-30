// Disposable RS-07 OLE FileDrop source. Creates only its own window, never
// controls another application or synthesizes input. A real pointer gesture
// invokes DoDragDrop, so source and target integrity levels can be verified.
// Build with the Windows .NET Framework v4 csc compiler, /target:winexe and
// references to System.Windows.Forms.dll and System.Drawing.dll.
// Arguments: fixture-directory new-event-log

using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Windows.Forms;

internal static class Rs07DragSource
{
    [DllImport("user32.dll")]
    private static extern bool SetProcessDpiAwarenessContext(IntPtr context);

    [STAThread]
    private static void Main(string[] args)
    {
        if (args.Length != 2) return;
        string directory = Path.GetFullPath(args[0]);
        string[] paths = { "中文 程序.exe", "记事本 快捷方式.lnk", "无效 文件.txt" };
        paths = paths.Select(name => Path.Combine(directory, name)).ToArray();
        if (paths.Any(path => !File.Exists(path))) return;
        bool elevated = new WindowsPrincipal(WindowsIdentity.GetCurrent())
            .IsInRole(WindowsBuiltInRole.Administrator);
        using (var log = new StreamWriter(new FileStream(args[1], FileMode.CreateNew, FileAccess.Write, FileShare.Read)))
        {
            log.AutoFlush = true;
            var timer = Stopwatch.StartNew();
            Action<string> record = fields => log.WriteLine("event=drag_source pid=" + Process.GetCurrentProcess().Id
                + " utc=" + DateTime.UtcNow.ToString("O") + " elapsed_ms=" + timer.ElapsedMilliseconds + " " + fields);
            record("phase=started elevated=" + elevated.ToString().ToLowerInvariant());
            SetProcessDpiAwarenessContext(new IntPtr(-4));
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            using (var form = new Form())
            {
                form.Text = "RS-07 · " + (elevated ? "管理员" : "普通权限") + "拖放源";
                form.StartPosition = FormStartPosition.Manual;
                form.Location = new Point(35, 120);
                form.ClientSize = new Size(580, 300);
                form.AutoScaleMode = AutoScaleMode.Dpi;
                var explanation = new Label
                {
                    Dock = DockStyle.Top, Height = 76, Padding = new Padding(12),
                    Text = "本进程权限：" + (elevated ? "已提权（管理员）" : "普通权限")
                        + "\r\n在下方列表 Ctrl+A，然后用鼠标整批拖入 Velo 导入预览。\r\n只提供三个测试文件，不执行文件或修改应用库。"
                };
                var files = new ListView
                {
                    Dock = DockStyle.Fill, View = View.Details, FullRowSelect = true,
                    MultiSelect = true, HideSelection = false
                };
                files.Columns.Add("测试文件", 520);
                foreach (string path in paths) files.Items.Add(new ListViewItem(Path.GetFileName(path)) { Tag = path });
                files.ItemDrag += (sender, eventArgs) =>
                {
                    string[] selected = files.SelectedItems.Cast<ListViewItem>().Select(item => (string)item.Tag).ToArray();
                    if (selected.Length == 0) return;
                    record("phase=drag_started count=" + selected.Length + " elevated=" + elevated.ToString().ToLowerInvariant());
                    var data = new DataObject(DataFormats.FileDrop, selected);
                    DragDropEffects effect = files.DoDragDrop(data, DragDropEffects.Copy);
                    record("phase=drag_finished count=" + selected.Length + " effect=" + effect);
                };
                form.Controls.Add(files);
                form.Controls.Add(explanation);
                form.Shown += (sender, eventArgs) =>
                {
                    files.Focus();
                    foreach (ListViewItem item in files.Items) item.Selected = true;
                    record("phase=ready files=3");
                };
                Application.Run(form);
            }
            record("phase=closed");
        }
    }
}
