use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceAction {
    Launcher,
    Explorer,
    Calculator,
    TaskManager,
    ControlPanel,
    Downloads,
    Documents,
    Pictures,
    ThisPc,
    CodeTerminal,
    CodeCommands,
    CodeFind,
    EdgeNewTab,
    EdgeAddress,
    EdgeBookmarks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Template {
    Explorer,
    Code,
    Edge,
}

impl Template {
    pub const ALL: [Self; 3] = [Self::Explorer, Self::Code, Self::Edge];

    pub fn name(self) -> &'static str {
        match self {
            Self::Explorer => "文件资源管理器",
            Self::Code => "VS Code",
            Self::Edge => "Microsoft Edge",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Explorer => "folder",
            Self::Code => "code",
            Self::Edge => "apps",
        }
    }
}

impl WorkspaceAction {
    pub const ALL: [Self; 15] = [
        Self::Launcher,
        Self::Explorer,
        Self::Calculator,
        Self::TaskManager,
        Self::ControlPanel,
        Self::Downloads,
        Self::Documents,
        Self::Pictures,
        Self::ThisPc,
        Self::CodeTerminal,
        Self::CodeCommands,
        Self::CodeFind,
        Self::EdgeNewTab,
        Self::EdgeAddress,
        Self::EdgeBookmarks,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Launcher => "启动器",
            Self::Explorer => "文件管理",
            Self::Calculator => "计算器",
            Self::TaskManager => "任务管理",
            Self::ControlPanel => "控制面板",
            Self::Downloads => "下载目录",
            Self::Documents => "文档目录",
            Self::Pictures => "图片目录",
            Self::ThisPc => "此电脑",
            Self::CodeTerminal => "终端",
            Self::CodeCommands => "命令面板",
            Self::CodeFind => "查找",
            Self::EdgeNewTab => "新标签",
            Self::EdgeAddress => "地址栏",
            Self::EdgeBookmarks => "书签",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Launcher => "launcher",
            Self::Explorer => "explorer",
            Self::Calculator => "calculator",
            Self::TaskManager => "task-manager",
            Self::ControlPanel => "control-panel",
            Self::Downloads => "downloads",
            Self::Documents => "documents",
            Self::Pictures => "pictures",
            Self::ThisPc => "this-pc",
            Self::CodeTerminal => "code-terminal",
            Self::CodeCommands => "code-commands",
            Self::CodeFind => "code-find",
            Self::EdgeNewTab => "edge-new-tab",
            Self::EdgeAddress => "edge-address",
            Self::EdgeBookmarks => "edge-bookmarks",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Launcher | Self::CodeFind | Self::EdgeAddress => "search",
            Self::Explorer => "folder",
            Self::Downloads => "download",
            Self::Documents => "document",
            Self::Pictures => "image",
            Self::Calculator => "calculator",
            Self::TaskManager | Self::EdgeNewTab => "apps",
            Self::ControlPanel => "settings",
            Self::ThisPc => "monitor",
            Self::CodeTerminal => "terminal",
            Self::CodeCommands => "code",
            Self::EdgeBookmarks => "star",
        }
    }

    pub fn system_id(self) -> Option<&'static str> {
        match self {
            Self::Explorer => Some("system:explorer"),
            Self::Calculator => Some("system:calculator"),
            Self::TaskManager => Some("system:taskmanager"),
            Self::ControlPanel => Some("system:control"),
            _ => None,
        }
    }

    pub fn template(self) -> Option<Template> {
        match self {
            Self::CodeTerminal | Self::CodeCommands | Self::CodeFind => Some(Template::Code),
            Self::EdgeNewTab | Self::EdgeAddress | Self::EdgeBookmarks => Some(Template::Edge),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Success {
    Opened,
    ShortcutSubmitted,
    TextSubmitted,
    ScriptCompleted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unavailable,
    Shell,
    TargetChanged,
    Permission,
    KeysHeld,
    Keyboard,
    ScriptUntrusted,
    ScriptUnavailable,
    ScriptTimeout,
    ScriptExit(u32),
    Cancelled,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "工作台动作不可用，请检查设置",
            Self::Shell => "Windows 无法打开该目标",
            Self::TargetChanged => "原窗口或输入焦点已变化，已取消输入",
            Self::Permission => "无法验证目标权限，已取消输入",
            Self::KeysHeld => "请松开修饰键后重试",
            Self::Keyboard => "Windows 未完整接收输入，请检查目标应用",
            Self::ScriptUntrusted => "脚本内容或参数已变化，请在动作库重新确认信任",
            Self::ScriptUnavailable => "脚本无法启动，请检查路径和工作目录",
            Self::ScriptTimeout => "脚本运行超时，已结束进程",
            Self::ScriptExit(_) => "脚本异常退出，请检查脚本",
            Self::Cancelled => "工作台动作已取消",
        }
    }
}
