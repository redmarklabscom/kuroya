mod brackets;
mod cursors;
mod inline;

pub(crate) use brackets::{
    paint_bracket_depth_markers, paint_bracket_match_boxes, paint_bracket_pair_guides,
};
pub(crate) use cursors::{paint_cursors, primary_insertion_cursor_rect};
pub(crate) use inline::{
    code_lens_command_at_pointer, paint_code_lenses, paint_completion_preview,
    paint_diagnostic_message, paint_folded_label, paint_ime_preedit, paint_inlay_hints,
};
