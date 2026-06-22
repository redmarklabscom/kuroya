#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EditorVimCharFind {
    pub(crate) motion: EditorVimCharFindMotion,
    pub(crate) target: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorVimCharFindMotion {
    FindBackward,
    FindForward,
    TillBackward,
    TillForward,
}

impl EditorVimCharFindMotion {
    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::FindBackward => Self::FindForward,
            Self::FindForward => Self::FindBackward,
            Self::TillBackward => Self::TillForward,
            Self::TillForward => Self::TillBackward,
        }
    }
}
