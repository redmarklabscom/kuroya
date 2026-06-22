mod accepts;
mod char_find;
mod operator_count;
mod operator_go;
mod register;
mod text_object;

pub(in crate::editor_vim_key_events) use accepts::vim_pending_key_accepts;
pub(in crate::editor_vim_key_events) use char_find::vim_pending_key_next_char_find;
pub(in crate::editor_vim_key_events) use operator_count::vim_pending_key_next_operator_count;
pub(in crate::editor_vim_key_events) use operator_go::vim_pending_key_next_operator_go;
pub(in crate::editor_vim_key_events) use register::vim_pending_key_next_named_register;
pub(in crate::editor_vim_key_events) use text_object::vim_pending_key_next_text_object;
