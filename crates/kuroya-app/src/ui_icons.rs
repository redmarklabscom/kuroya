pub(crate) use crate::ui_icon_shapes::draw_icon;
pub(crate) use crate::ui_icons::widgets::{icon_button, icon_label, icon_text_button};

mod widgets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    NewFile,
    File,
    Folder,
    FolderOpen,
    ChevronRight,
    ChevronDown,
    Plus,
    Minus,
    Refresh,
    Maximize,
    Restore,
    Command,
    Search,
    Close,
    Terminal,
    Trash,
    Copy,
    Panes,
    GitBranch,
    Diagnostics,
    Lsp,
    Cursor,
    Theme,
    Code,
    Settings,
}
