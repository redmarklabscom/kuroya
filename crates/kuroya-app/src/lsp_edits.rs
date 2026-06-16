use crate::lsp_text_positions::lsp_one_based_utf16_position_to_buffer_char;
use kuroya_core::{LspDocumentHighlight, LspTextEdit, TextBuffer, TextEdit as BufferTextEdit};
use std::{ops::Range, path::Path};

mod completion;

pub(crate) use completion::{
    CompletionBufferEditPlan, apply_completion_passthrough_events_with_editor_keys,
    completion_buffer_edit_plan,
};

#[cfg(test)]
pub(crate) use completion::{
    apply_completion_passthrough_events, completion_buffer_edits,
    completion_passthrough_edit_intent, completion_passthrough_edit_intent_with_acceptance,
};

pub(crate) fn buffer_text_edits_from_lsp(
    buffer: &TextBuffer,
    edits: &[LspTextEdit],
) -> Option<Vec<BufferTextEdit>> {
    match edits {
        [] => return Some(Vec::new()),
        [edit] => {
            let range = lsp_text_edit_to_buffer_range(buffer, edit)?;
            return Some(vec![BufferTextEdit {
                range,
                inserted: edit.new_text.clone(),
            }]);
        }
        _ => {}
    }

    let mut buffer_edits = Vec::with_capacity(edits.len());
    for edit in edits {
        buffer_edits.push(BufferTextEdit {
            range: lsp_text_edit_to_buffer_range(buffer, edit)?,
            inserted: edit.new_text.clone(),
        });
    }
    if !sort_buffer_edits_by_range_and_reject_overlaps(&mut buffer_edits) {
        return None;
    }
    Some(buffer_edits)
}

fn lsp_text_edit_to_buffer_range(buffer: &TextBuffer, edit: &LspTextEdit) -> Option<Range<usize>> {
    let start =
        lsp_one_based_utf16_position_to_buffer_char(buffer, edit.start_line, edit.start_column)?;
    let end = lsp_one_based_utf16_position_to_buffer_char(buffer, edit.end_line, edit.end_column)?;
    (start <= end).then_some(start..end)
}

fn sort_buffer_edits_by_range_and_reject_overlaps(edits: &mut [BufferTextEdit]) -> bool {
    if edits.len() <= 1 {
        return true;
    }

    edits.sort_by(|left, right| {
        left.range
            .start
            .cmp(&right.range.start)
            .then(left.range.end.cmp(&right.range.end))
    });

    edits
        .windows(2)
        .all(|pair| pair[0].range.end <= pair[1].range.start)
}

pub(crate) fn document_highlight_char_range(
    buffer: &TextBuffer,
    highlight: &LspDocumentHighlight,
) -> Option<Range<usize>> {
    let start =
        lsp_one_based_utf16_position_to_buffer_char(buffer, highlight.line, highlight.column)?;
    let end = lsp_one_based_utf16_position_to_buffer_char(
        buffer,
        highlight.end_line,
        highlight.end_column,
    )?;
    (start < end).then_some(start..end)
}

pub(crate) fn apply_lsp_edits_to_text(
    path: &Path,
    text: String,
    edits: &[LspTextEdit],
) -> Option<String> {
    let mut buffer = TextBuffer::from_text(0, Some(path.to_path_buf()), text);
    let buffer_edits = buffer_text_edits_from_lsp(&buffer, edits)?;
    buffer.apply_edits(buffer_edits);
    Some(buffer.text())
}

#[cfg(test)]
mod tests {
    use super::buffer_text_edits_from_lsp;
    use kuroya_core::{LspTextEdit, TextBuffer};

    #[test]
    fn lsp_edits_are_sorted_in_place_and_reject_overlaps() {
        let buffer = TextBuffer::from_text(1, None, "abc\ndef\n".to_owned());
        let edits = vec![
            LspTextEdit {
                path: "src/lib.rs".into(),
                start_line: 2,
                start_column: 1,
                end_line: 2,
                end_column: 1,
                new_text: "X".to_owned(),
            },
            LspTextEdit {
                path: "src/lib.rs".into(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: "Y".to_owned(),
            },
        ];

        let buffer_edits = buffer_text_edits_from_lsp(&buffer, &edits).unwrap();

        assert_eq!(buffer_edits[0].range, 0..0);
        assert_eq!(buffer_edits[1].range, 4..4);

        let overlapping = vec![
            LspTextEdit {
                path: "src/lib.rs".into(),
                start_line: 1,
                start_column: 2,
                end_line: 1,
                end_column: 4,
                new_text: "left".to_owned(),
            },
            LspTextEdit {
                path: "src/lib.rs".into(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 3,
                new_text: "right".to_owned(),
            },
        ];

        assert!(buffer_text_edits_from_lsp(&buffer, &overlapping).is_none());
    }
}
