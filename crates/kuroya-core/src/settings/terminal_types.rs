use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalCursorStyle {
    #[default]
    Block,
    Line,
    Underline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalInactiveCursorStyle {
    #[default]
    Outline,
    Block,
    Line,
    Underline,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalRightClickBehavior {
    #[default]
    Default,
    CopyPaste,
    Paste,
    SelectWord,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalMiddleClickBehavior {
    #[default]
    Default,
    Paste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalSplitCwd {
    WorkspaceRoot,
    Initial,
    #[default]
    Inherited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalMultiLinePasteWarning {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalConfirmOnKill {
    Never,
    #[default]
    Editor,
    Panel,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalConfirmOnExit {
    #[default]
    Never,
    Always,
    HasChildProcesses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalHideOnStartup {
    #[default]
    Never,
    WhenEmpty,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalTabsShowActions {
    Always,
    SingleTerminal,
    #[default]
    SingleTerminalOrNarrow,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalTabsShowActiveTerminal {
    Always,
    SingleTerminal,
    #[default]
    SingleTerminalOrNarrow,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalTabsHideCondition {
    Never,
    #[default]
    SingleTerminal,
    SingleGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalTabsFocusMode {
    #[default]
    SingleClick,
    DoubleClick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalTabsLocation {
    #[default]
    Top,
    Left,
    Right,
}
