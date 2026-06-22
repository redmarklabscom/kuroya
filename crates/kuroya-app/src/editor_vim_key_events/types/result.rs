use kuroya_core::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VimKeyResult {
    pub(crate) handled: bool,
    pub(crate) changed: bool,
    pub(crate) suppress_text: Option<char>,
    pub(crate) command: Option<Command>,
}

impl VimKeyResult {
    pub(crate) fn ignored() -> Self {
        Self {
            handled: false,
            changed: false,
            suppress_text: None,
            command: None,
        }
    }

    pub(crate) fn handled(suppress_text: Option<char>) -> Self {
        Self {
            handled: true,
            changed: false,
            suppress_text,
            command: None,
        }
    }

    pub(crate) fn changed(suppress_text: Option<char>) -> Self {
        Self {
            handled: true,
            changed: true,
            suppress_text,
            command: None,
        }
    }

    pub(crate) fn command(command: Command, suppress_text: Option<char>) -> Self {
        Self {
            handled: true,
            changed: false,
            suppress_text,
            command: Some(command),
        }
    }
}
