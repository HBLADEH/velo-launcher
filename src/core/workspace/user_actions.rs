use std::path::{Component, Path, Prefix};

use serde::{Deserialize, Serialize};

use crate::core::UnknownFields;
use crate::platform::windows::hotkey::Key;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserAction {
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: String,
    pub action: Action,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    InsertText { text: String },
    SendKeys { steps: Vec<KeyStep> },
    RunScript(Script),
    OpenTarget { target: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyStep {
    pub chord: String,
    #[serde(default)]
    pub wait_ms: u16,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl KeyStep {
    pub fn parts(&self) -> Option<Key> {
        Key::parse(&self.chord).ok().or_else(|| {
            // Action steps may use a single key. Global shortcuts still need a modifier.
            (!self.chord.contains('+'))
                .then(|| Key::parse(&format!("Ctrl+{}", self.chord)).ok())
                .flatten()
                .map(|key| Key {
                    modifiers: 0,
                    ..key
                })
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Script {
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub directory: String,
    pub timeout_secs: u16,
    #[serde(default)]
    pub show_console: bool,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(skip)]
    pub review: bool,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Script {
    pub fn valid(&self) -> bool {
        local_path(&self.path)
            && Path::new(&self.path).extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd")
            })
            && safe_literal(&self.path)
            && (self.directory.is_empty()
                || local_path(&self.directory) && safe_literal(&self.directory))
            && (1..=120).contains(&self.timeout_secs)
            && self.args.len() <= 16
            && self
                .args
                .iter()
                .all(|arg| arg.len() <= 256 && safe_literal(arg))
    }
}

fn local_path(text: &str) -> bool {
    let path = Path::new(text);
    path.is_absolute()
        && text
            .as_bytes()
            .get(2..)
            .is_some_and(|tail| !tail.contains(&b':'))
        && matches!(path.components().next(), Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)))
}

fn safe_literal(text: &str) -> bool {
    !text.chars().any(|ch| {
        ch.is_control()
            || matches!(
                ch,
                '"' | '%' | '!' | '^' | '&' | '|' | '<' | '>' | '(' | ')'
            )
    })
}

impl UserAction {
    pub fn valid(&self) -> bool {
        !self.label.trim().is_empty()
            && self.label.chars().count() <= 64
            && self.description.chars().count() <= 160
            && self.action.valid()
    }

    pub fn glyph(&self) -> &str {
        if self.icon.is_empty() {
            self.action.glyph()
        } else {
            &self.icon
        }
    }
}

impl Action {
    pub fn requires_target(&self) -> bool {
        matches!(self, Self::InsertText { .. } | Self::SendKeys { .. })
    }

    pub fn ready(&self) -> bool {
        !matches!(self, Self::RunScript(script) if script.digest.is_none())
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::InsertText { .. } => "InsertText",
            Self::SendKeys { .. } => "SendKeys",
            Self::RunScript(_) => "RunScript",
            Self::OpenTarget { .. } => "OpenTarget",
        }
    }

    pub fn glyph(&self) -> &'static str {
        match self {
            Self::InsertText { .. } => "document",
            Self::SendKeys { .. } => "keyboard",
            Self::RunScript(_) => "terminal",
            Self::OpenTarget { .. } => "folder",
        }
    }

    fn valid(&self) -> bool {
        match self {
            Self::InsertText { text } => {
                !text.is_empty()
                    && !text.contains('\0')
                    && text.encode_utf16().take(8193).count() <= 8192
            }
            Self::SendKeys { steps } => {
                !steps.is_empty()
                    && steps.len() <= 8
                    && steps
                        .iter()
                        .all(|step| step.wait_ms <= 2000 && step.parts().is_some())
            }
            Self::RunScript(script) => script.valid(),
            Self::OpenTarget { target } => {
                target.len() <= 4096
                    && !target.chars().any(char::is_control)
                    && ((Path::new(target).is_absolute()
                        && !Path::new(target).extension().is_some_and(|ext| {
                            ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd")
                        }))
                        || target.starts_with("https://")
                        || target.starts_with("http://"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_batch_command_expansion_and_preserves_literal_arguments() {
        let script = Script {
            path: r"C:\Users\Test User\build.cmd".into(),
            args: vec!["hello world".into(), "中文".into()],
            directory: String::new(),
            timeout_secs: 10,
            show_console: false,
            digest: None,
            review: false,
            unknown: UnknownFields::new(),
        };
        assert!(script.valid());
        for arg in ["%PATH%", "a!b", "a\r\n", "x\"", "&exit", "a|b", "a<b"] {
            assert!(
                !Script {
                    args: vec![arg.into()],
                    ..script.clone()
                }
                .valid()
            );
        }
        assert!(
            !Script {
                path: r"\\host\share\build.cmd".into(),
                ..script
            }
            .valid()
        );
    }
}
