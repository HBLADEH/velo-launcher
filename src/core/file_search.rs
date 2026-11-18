//! Everything results are session-only and intentionally never enter the
//! application catalogue, launch history, pins or icon cache.

use std::time::Duration;

pub const PAGE_SIZE: u32 = 50;
pub const MAX_RESULTS: usize = 200;
pub const QUERY_DEBOUNCE: Duration = Duration::from_millis(80);
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(2);
pub const IDLE_SHUTDOWN: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHit {
    pub path: String,
    pub is_folder: bool,
}

impl FileHit {
    pub fn name(&self) -> &str {
        self.path
            .trim_end_matches(['\\', '/'])
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&self.path)
    }

    pub fn parent(&self) -> &str {
        self.path.rfind(['\\', '/']).map_or("", |separator| {
            if separator == 2 && self.path.as_bytes().get(1) == Some(&b':') {
                &self.path[..separator + 1]
            } else {
                &self.path[..separator]
            }
        })
    }

    pub fn extension(&self) -> &str {
        if self.is_folder {
            return "文件夹";
        }
        self.name()
            .rsplit_once('.')
            .map_or("文件", |(_, extension)| extension)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Disabled,
    Ready,
    Searching,
    Empty,
    NotRunning,
    NotIndexed,
    Incompatible,
    Timeout,
    Error,
    Protocol,
    OpenError,
}

impl Status {
    pub fn message(&self, _executable_selected: bool) -> &'static str {
        match self {
            Self::Disabled | Self::Ready => "",
            Self::Searching => "正在搜索文件…",
            Self::Empty => "没有找到匹配的文件",
            Self::NotRunning => "Everything 未运行。可启动已安装的程序，或打开官方网站安装。",
            Self::NotIndexed => "Everything 索引尚未准备好；请在 Everything 中检查索引设置后重试。",
            Self::Incompatible => {
                "未检测到支持 Unicode Query2 IPC 的 Everything 实例。请运行受支持的稳定版。"
            }
            Self::Timeout => "Everything 暂时没有响应，请检查程序状态后重试。",
            Self::Error => "无法连接 Everything，请检查程序是否正在运行。",
            Self::Protocol => "Everything 返回了不兼容或无效的数据，请更新程序后重试。",
            Self::OpenError => "Windows 无法打开该文件或文件夹；它可能已被移动或删除。",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::JsonDocument;

    #[test]
    fn file_rows_keep_the_complete_unicode_path_and_derive_display_fields() {
        let file = FileHit {
            path: r"C:\数据\项目\报告.pdf".into(),
            is_folder: false,
        };
        assert_eq!(file.name(), "报告.pdf");
        assert_eq!(file.parent(), r"C:\数据\项目");
        assert_eq!(file.extension(), "pdf");
        let folder = FileHit {
            path: r"C:\数据\项目".into(),
            is_folder: true,
        };
        assert_eq!(folder.extension(), "文件夹");
        assert!(Status::Timeout.message(false).contains("没有响应"));
    }

    #[test]
    fn default_everything_settings_remain_disabled_for_existing_configurations() {
        let config = super::super::config::Config::decode(
            br#"{"version":1,"hotkey":"Alt+Space","future_setting":{"preserved":true}}"#,
        )
        .expect("legacy config remains readable");
        assert!(!config.everything.enabled);
        assert!(config.everything.executable.is_empty());
        assert_eq!(config.unknown["future_setting"]["preserved"], true);
        assert!(config.validate().is_ok());
    }
}
