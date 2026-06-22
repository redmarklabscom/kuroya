mod mutation;
mod pending;

pub(in crate::editor_vim_key_events) use self::mutation::vim_normal_key_can_mutate;
pub(in crate::editor_vim_key_events) use self::pending::{
    vim_normal_key_next_pending, vim_normal_key_next_pending_after_count,
};
