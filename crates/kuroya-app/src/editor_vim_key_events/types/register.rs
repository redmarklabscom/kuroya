#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorVimRegister {
    pub(crate) text: String,
    pub(crate) kind: EditorVimRegisterKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimRegisterKind {
    Characterwise,
    Linewise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EditorVimNamedRegister {
    pub(crate) index: usize,
    pub(crate) append: bool,
}
