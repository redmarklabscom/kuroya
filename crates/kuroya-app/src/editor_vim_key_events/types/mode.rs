#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimMode {
    Normal,
    Insert,
}

impl EditorVimMode {
    pub(crate) fn accepts_text_input(self) -> bool {
        matches!(self, Self::Insert)
    }
}
