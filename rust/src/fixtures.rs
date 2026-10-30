//! Synthetic, embedded RS-02 content. This is not the product index or search engine.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Pinned,
    Frequent,
    System,
}

#[derive(Clone, Copy, Debug)]
pub struct FixtureItem {
    pub id: &'static str,
    pub name: &'static str,
    pub subtitle: &'static str,
    pub icon_key: &'static str,
    pub group: Group,
}

pub const ITEMS: &[FixtureItem] = &[
    item(
        "code",
        "Visual Studio Code",
        r"C:\Program Files\Microsoft VS Code\Code.exe",
        "code",
        Group::Pinned,
    ),
    item(
        "docs",
        "研发资料与跨团队项目管理工作台（离线文档中心）",
        r"D:\Workspace\研发资料\项目管理\工作台.exe",
        "folder",
        Group::Pinned,
    ),
    item(
        "terminal",
        "Windows Terminal",
        r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal\wt.exe",
        "terminal",
        Group::Pinned,
    ),
    item(
        "studio",
        "Visual Studio",
        r"C:\Program Files\Microsoft Visual Studio\2026\Community\Common7\IDE\devenv.exe",
        "code",
        Group::Frequent,
    ),
    item(
        "notes",
        "记事本",
        r"C:\Windows\System32\notepad.exe",
        "code",
        Group::Frequent,
    ),
    item(
        "browser",
        "工作浏览器",
        r"C:\Program Files\Workspace Browser\Application\browser.exe",
        "apps",
        Group::Frequent,
    ),
    item(
        "files",
        "项目文件夹",
        r"D:\Workspace\Velo\Projects",
        "folder",
        Group::Frequent,
    ),
    item(
        "board",
        "开发工具箱",
        r"D:\Tools\Developer Toolkit\toolkit.exe",
        "board",
        Group::Frequent,
    ),
    item(
        "tasks",
        "任务看板",
        r"D:\Tools\Task Board\tasks.exe",
        "apps",
        Group::Frequent,
    ),
    item(
        "editor",
        "Code Editor Preview",
        r"C:\Program Files\Developer Tools\Code Editor Preview\stable\resources\app\extensions\workspace\bin\code-editor-preview.exe",
        "code",
        Group::Frequent,
    ),
    item(
        "reader",
        "文档阅读器",
        r"D:\Tools\Document Reader\reader.exe",
        "folder",
        Group::Frequent,
    ),
    item(
        "capture",
        "屏幕录制",
        r"D:\Tools\Screen Capture\capture.exe",
        "monitor",
        Group::Frequent,
    ),
    item(
        "music",
        "音乐",
        r"D:\Tools\Music\music.exe",
        "apps",
        Group::Frequent,
    ),
    item(
        "archive",
        "归档管理",
        r"D:\Tools\Archive Manager\archive.exe",
        "folder",
        Group::Frequent,
    ),
    item(
        "missing",
        "未命名工作台",
        r"D:\Workspace\Local Tools\workbench.exe",
        "",
        Group::Frequent,
    ),
    item(
        "settings",
        "Windows 设置",
        "系统 · 管理设备与个人偏好",
        "settings",
        Group::System,
    ),
    item(
        "explorer",
        "文件资源管理器",
        "系统 · 浏览文件与文件夹",
        "folder",
        Group::System,
    ),
    item(
        "calculator",
        "计算器",
        "系统 · 标准与科学计算",
        "calculator",
        Group::System,
    ),
    item(
        "task-manager",
        "任务管理器",
        "系统 · 查看应用与资源使用",
        "monitor",
        Group::System,
    ),
    item(
        "command",
        "命令提示符",
        "系统 · 命令行工具",
        "terminal",
        Group::System,
    ),
    item(
        "control",
        "控制面板",
        "系统 · 调整计算机设置",
        "settings",
        Group::System,
    ),
    item(
        "network",
        "网络连接",
        "系统 · 查看网络适配器",
        "board",
        Group::System,
    ),
    item(
        "display",
        "显示设置",
        "系统 · 分辨率与缩放",
        "monitor",
        Group::System,
    ),
    item(
        "apps",
        "已安装的应用",
        "系统 · 管理应用程序",
        "apps",
        Group::System,
    ),
    item(
        "keyboard",
        "键盘设置",
        "系统 · 键盘与输入偏好",
        "settings",
        Group::System,
    ),
    item(
        "sound",
        "声音设置",
        "系统 · 输入与输出设备",
        "settings",
        Group::System,
    ),
    item(
        "power",
        "电源选项",
        "系统 · 电源与睡眠设置",
        "settings",
        Group::System,
    ),
    item(
        "environment",
        "环境变量",
        "系统 · 编辑用户环境变量",
        "board",
        Group::System,
    ),
];

const fn item(
    id: &'static str,
    name: &'static str,
    subtitle: &'static str,
    icon_key: &'static str,
    group: Group,
) -> FixtureItem {
    FixtureItem {
        id,
        name,
        subtitle,
        icon_key,
        group,
    }
}

/// Deliberately simple filtering for the UI fixture, with stable fixture order.
/// No pinyin, ranking, filesystem access, history, or product search semantics.
pub fn filter(query: &str) -> Vec<FixtureItem> {
    let needle = query.trim().to_lowercase();
    ITEMS
        .iter()
        .copied()
        .filter(|item| {
            item.name.to_lowercase().contains(&needle)
                || item.subtitle.to_lowercase().contains(&needle)
        })
        .collect()
}

pub fn selection<'a>(visible: &'a [FixtureItem], id: &str) -> Option<&'a FixtureItem> {
    visible.iter().find(|item| item.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn fixture_covers_grid_boundaries_and_content_edges() {
        assert_eq!(ITEMS.iter().filter(|i| i.group == Group::Pinned).count(), 3);
        assert_eq!(
            ITEMS.iter().filter(|i| i.group == Group::Frequent).count(),
            12
        );
        assert_eq!(
            ITEMS.iter().filter(|i| i.group == Group::System).count(),
            13
        );
        assert_eq!(
            ITEMS.iter().map(|i| i.id).collect::<HashSet<_>>().len(),
            ITEMS.len()
        );
        assert!(ITEMS.iter().any(|i| i.icon_key.is_empty()));
        assert!(
            ITEMS
                .iter()
                .any(|i| i.name.chars().count() > 20 && !i.name.is_ascii())
        );
        assert!(
            ITEMS
                .iter()
                .any(|i| i.subtitle.is_ascii() && i.subtitle.len() > 85)
        );
    }

    #[test]
    fn fixture_filter_handles_cjk_case_paths_whitespace_and_no_matches() {
        assert_eq!(
            filter("  CODE  ").iter().map(|i| i.id).collect::<Vec<_>>(),
            ["code", "editor"]
        );
        assert_eq!(filter("研发资料")[0].id, "docs");
        assert_eq!(filter(r"d:\tools\").len(), 6);
        assert_eq!(filter("系统").len(), 13);
        assert_eq!(filter("  ").len(), ITEMS.len());
        assert!(filter("zz-no-matches-无结果").is_empty());
    }

    #[test]
    fn stale_or_unknown_choices_cannot_be_recorded_after_filtering() {
        let visible = filter("code");
        assert_eq!(
            selection(&visible, "editor").unwrap().name,
            "Code Editor Preview"
        );
        assert!(selection(&visible, "calculator").is_none());
        assert!(selection(&filter("no-matches"), "code").is_none());
    }
}
