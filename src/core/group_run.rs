//! Bounded, complete result snapshots survive notification coalescing.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Queued,
    Checking,
    Submitted,
    AlreadyRunning,
    Pending,
    NeedsReview,
    NeedsConfiguration,
    Failed,
    Cancelled,
}

impl State {
    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::NeedsReview
                | Self::NeedsConfiguration
                | Self::Pending
                | Self::Failed
                | Self::Cancelled
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "等待启动",
            Self::Checking => "正在检查",
            Self::Submitted => "已发送启动请求",
            Self::AlreadyRunning => "已运行，已跳过",
            Self::Pending => "已请求，等待程序出现",
            Self::NeedsReview => "需要确认",
            Self::NeedsConfiguration => "需要修改配置",
            Self::Failed => "启动失败",
            Self::Cancelled => "已取消",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub app_id: String,
    pub name: String,
    pub state: State,
    pub message: &'static str,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub run_id: u64,
    pub group_id: String,
    pub name: String,
    pub entries: Vec<Entry>,
    pub done: bool,
    pub history_saved: bool,
}

impl Report {
    pub fn retry_ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.state.retryable())
            .map(|entry| entry.app_id.clone())
            .collect()
    }

    pub fn accepted(&self) -> bool {
        self.entries.iter().any(|entry| {
            matches!(
                entry.state,
                State::Submitted | State::AlreadyRunning | State::Pending
            )
        })
    }

    pub fn summary(&self) -> String {
        let count = |state| {
            self.entries
                .iter()
                .filter(|entry| entry.state == state)
                .count()
        };
        let mut text = format!(
            "{} · 已请求 {} · 已运行 {} · 待确认 {} · 失败 {}",
            self.name,
            count(State::Submitted),
            count(State::AlreadyRunning),
            count(State::NeedsReview) + count(State::NeedsConfiguration) + count(State::Pending),
            count(State::Failed)
        );
        if !self.done {
            text.push_str(" · 进行中");
        }
        if !self.history_saved {
            text.push_str(" · 历史保存失败，请检查数据目录权限");
        }
        text
    }
}
