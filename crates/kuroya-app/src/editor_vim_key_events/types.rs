mod case;
mod char_find;
mod mode;
mod operator;
mod pending;
mod register;
mod repeat;
mod result;
mod text_object;

pub(crate) use self::case::EditorVimCaseConversion;
pub(crate) use self::char_find::{EditorVimCharFind, EditorVimCharFindMotion};
pub(crate) use self::mode::EditorVimMode;
pub(crate) use self::operator::{EditorVimOperatorGoKind, EditorVimOperatorMotion};
pub(crate) use self::pending::EditorVimPendingKey;
pub(crate) use self::register::{EditorVimNamedRegister, EditorVimRegister, EditorVimRegisterKind};
pub(crate) use self::repeat::{
    EditorVimInsertReplayStep, EditorVimLastChange, EditorVimRepeatAction,
};
pub(crate) use self::result::VimKeyResult;
pub(crate) use self::text_object::{EditorVimTextObjectKind, EditorVimTextObjectScope};

pub(crate) const VIM_MAX_COUNT: usize = 999;
pub(crate) const VIM_DEFAULT_CTRL_SCROLL_LINES: usize = 10;
pub(crate) const VIM_DEFAULT_PAGE_SCROLL_LINES: usize = VIM_DEFAULT_CTRL_SCROLL_LINES * 2;
