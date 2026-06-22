use kuroya_core::TextBuffer;

use super::super::super::state::vim_named_register;
use super::super::super::{EditorVimRegister, EditorVimRepeatAction};
use super::super::put::{vim_put_register_after, vim_put_register_before};
use super::action::ApplyLastActionOutcome;

pub(super) fn vim_apply_register_put_repeat_action(
    buffer: &mut TextBuffer,
    action: EditorVimRepeatAction,
    count: usize,
    unnamed_register: &mut Option<EditorVimRegister>,
) -> ApplyLastActionOutcome {
    let changed = match action {
        EditorVimRepeatAction::PutAfter => {
            vim_put_register_after(buffer, unnamed_register.as_ref(), count)
        }
        EditorVimRepeatAction::PutAfterNamed(register) => {
            let named_register = vim_named_register(register);
            vim_put_register_after(buffer, named_register.as_ref(), count)
        }
        EditorVimRepeatAction::PutBefore => {
            vim_put_register_before(buffer, unnamed_register.as_ref(), count)
        }
        EditorVimRepeatAction::PutBeforeNamed(register) => {
            let named_register = vim_named_register(register);
            vim_put_register_before(buffer, named_register.as_ref(), count)
        }
        _ => unreachable!("non-put repeat action routed to put handler"),
    };
    ApplyLastActionOutcome::normal(changed)
}
