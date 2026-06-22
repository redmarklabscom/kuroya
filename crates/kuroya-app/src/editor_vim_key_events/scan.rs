mod mutation;
mod sequence;
mod suppression;

#[cfg(test)]
pub(in crate::editor_vim_key_events) use self::mutation::vim_events_include_mutation;
pub(crate) use self::mutation::vim_events_include_mutation_with_settings;
pub(crate) use self::sequence::vim_key_sequence_is_normal_mode_supported;
pub(crate) use self::suppression::vim_text_after_suppression;
