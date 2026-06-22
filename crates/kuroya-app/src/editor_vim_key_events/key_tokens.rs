mod parse;
mod render;
mod shared;

pub(in crate::editor_vim_key_events) use self::parse::vim_key_sequence_events;
pub(crate) use self::render::vim_key_token_for_event;

pub(crate) fn vim_key_sequence_is_single_supported(sequence: &str) -> bool {
    let Some(keys) = vim_key_sequence_events(sequence.trim()) else {
        return false;
    };
    keys.len() == 1
}

pub(crate) fn vim_key_sequence_is_supported(sequence: &str) -> bool {
    let Some(keys) = vim_key_sequence_events(sequence.trim()) else {
        return false;
    };
    !keys.is_empty()
}

pub(crate) fn vim_key_sequences_match(left: &str, right: &str) -> bool {
    let Some(left_keys) = vim_key_sequence_events(left.trim()) else {
        return false;
    };
    let Some(right_keys) = vim_key_sequence_events(right.trim()) else {
        return false;
    };
    left_keys == right_keys
}

pub(crate) fn vim_key_sequence_starts_with(sequence: &str, prefix: &str) -> bool {
    let Some(sequence_keys) = vim_key_sequence_events(sequence.trim()) else {
        return false;
    };
    let Some(prefix_keys) = vim_key_sequence_events(prefix.trim()) else {
        return false;
    };
    !prefix_keys.is_empty() && sequence_keys.starts_with(&prefix_keys)
}
