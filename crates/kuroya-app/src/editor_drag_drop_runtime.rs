use crate::{KuroyaApp, transient_state::EditorSelectionDrag, workspace_state::PaneId};
use kuroya_core::{BufferId, TextBuffer, TextEdit};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorSelectionDragPayload {
    pub(crate) ranges: Vec<Range<usize>>,
    pub(crate) text: String,
}

impl KuroyaApp {
    pub(crate) fn start_editor_selection_drag(
        &mut self,
        pane_id: PaneId,
        buffer_id: BufferId,
        char_idx: usize,
    ) {
        if !self.settings.drag_and_drop {
            self.editor_selection_drag = None;
            return;
        }
        let Some(payload) = self
            .buffer(buffer_id)
            .and_then(|buffer| editor_selection_drag_payload(buffer, char_idx))
        else {
            self.editor_selection_drag = None;
            return;
        };
        self.editor_selection_drag = Some(EditorSelectionDrag {
            pane_id,
            buffer_id,
            ranges: payload.ranges,
            text: payload.text,
        });
    }

    pub(crate) fn finish_editor_selection_drag(
        &mut self,
        pane_id: PaneId,
        buffer_id: BufferId,
        drop_char_idx: usize,
    ) {
        let Some(drag) = self.editor_selection_drag.take() else {
            return;
        };
        if !self.settings.drag_and_drop || drag.pane_id != pane_id || drag.buffer_id != buffer_id {
            return;
        }

        let changed = self.buffer_mut(buffer_id).is_some_and(|buffer| {
            move_selected_text_by_drag(buffer, &drag.ranges, &drag.text, drop_char_idx)
        });
        if changed {
            self.mark_buffer_changed(buffer_id);
            self.status = "Moved selection".to_owned();
        }
    }

    pub(crate) fn clear_editor_selection_drag_for_buffer(&mut self, id: BufferId) {
        if self
            .editor_selection_drag
            .as_ref()
            .is_some_and(|drag| drag.buffer_id == id)
        {
            self.editor_selection_drag = None;
        }
    }
}

pub(crate) fn editor_selection_drag_payload(
    buffer: &TextBuffer,
    char_idx: usize,
) -> Option<EditorSelectionDragPayload> {
    let ranges = buffer
        .selections()
        .iter()
        .filter_map(|selection| {
            let range = selection.range();
            (range.start < range.end).then_some(range)
        })
        .collect::<Vec<_>>();
    if ranges.is_empty()
        || !ranges
            .iter()
            .any(|range| char_idx >= range.start && char_idx <= range.end)
    {
        return None;
    }

    Some(EditorSelectionDragPayload {
        ranges,
        text: buffer.selected_text()?,
    })
}

pub(crate) fn move_selected_text_by_drag(
    buffer: &mut TextBuffer,
    ranges: &[Range<usize>],
    text: &str,
    drop_char_idx: usize,
) -> bool {
    let drop_char_idx = drop_char_idx.min(buffer.len_chars());
    if text.is_empty()
        || ranges.is_empty()
        || ranges
            .iter()
            .any(|range| drop_char_idx >= range.start && drop_char_idx <= range.end)
        || !drag_ranges_are_valid(buffer, ranges)
        || drag_text_for_ranges(buffer, ranges).as_deref() != Some(text)
    {
        return false;
    }

    let insertion = TextEdit {
        range: drop_char_idx..drop_char_idx,
        inserted: text.to_owned(),
    };
    let inserted_selection = 0..text.chars().count();
    let mut edits = ranges
        .iter()
        .map(|range| TextEdit {
            range: range.clone(),
            inserted: String::new(),
        })
        .collect::<Vec<_>>();
    edits.push(insertion.clone());

    buffer.apply_edits_with_inserted_selection(edits, &insertion, inserted_selection)
}

fn drag_ranges_are_valid(buffer: &TextBuffer, ranges: &[Range<usize>]) -> bool {
    let len_chars = buffer.len_chars();
    if ranges
        .iter()
        .any(|range| range.start >= range.end || range.end > len_chars)
    {
        return false;
    }

    let mut sorted = ranges.to_vec();
    sorted.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    sorted.windows(2).all(|pair| pair[0].end <= pair[1].start)
}

fn drag_text_for_ranges(buffer: &TextBuffer, ranges: &[Range<usize>]) -> Option<String> {
    let selected = ranges
        .iter()
        .map(|range| buffer.text_range(range.clone()))
        .collect::<Option<Vec<_>>>()?;
    (!selected.is_empty()).then(|| selected.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{editor_selection_drag_payload, move_selected_text_by_drag};
    use kuroya_core::TextBuffer;

    #[test]
    fn selection_drag_payload_requires_pointer_inside_selection() {
        let mut buffer = TextBuffer::from_text(1, None, "abcXYZdef".to_owned());
        buffer.set_selection(3, 6);

        let payload = editor_selection_drag_payload(&buffer, 4).expect("payload");

        assert_eq!(payload.ranges, vec![3..6]);
        assert_eq!(payload.text, "XYZ");
        assert!(editor_selection_drag_payload(&buffer, 1).is_none());
    }

    #[test]
    fn move_selected_text_by_drag_moves_selection_forward() {
        let mut buffer = TextBuffer::from_text(1, None, "abcXYZdef".to_owned());
        buffer.set_selection(3, 6);
        let range = 3..6;

        assert!(move_selected_text_by_drag(
            &mut buffer,
            std::slice::from_ref(&range),
            "XYZ",
            9
        ));

        assert_eq!(buffer.text(), "abcdefXYZ");
        assert_eq!(buffer.selected_text().as_deref(), Some("XYZ"));
    }

    #[test]
    fn move_selected_text_by_drag_moves_selection_backward() {
        let mut buffer = TextBuffer::from_text(1, None, "abcXYZdef".to_owned());
        buffer.set_selection(3, 6);
        let range = 3..6;

        assert!(move_selected_text_by_drag(
            &mut buffer,
            std::slice::from_ref(&range),
            "XYZ",
            0
        ));

        assert_eq!(buffer.text(), "XYZabcdef");
        assert_eq!(buffer.selected_text().as_deref(), Some("XYZ"));
    }

    #[test]
    fn move_selected_text_by_drag_ignores_drops_inside_selection() {
        let mut buffer = TextBuffer::from_text(1, None, "abcXYZdef".to_owned());
        buffer.set_selection(3, 6);
        let range = 3..6;

        assert!(!move_selected_text_by_drag(
            &mut buffer,
            std::slice::from_ref(&range),
            "XYZ",
            4
        ));

        assert_eq!(buffer.text(), "abcXYZdef");
        assert_eq!(buffer.selected_text().as_deref(), Some("XYZ"));
    }

    #[test]
    fn move_selected_text_by_drag_rejects_stale_payload_text() {
        let mut buffer = TextBuffer::from_text(1, None, "abc123def".to_owned());
        let range = 3..6;

        assert!(!move_selected_text_by_drag(
            &mut buffer,
            std::slice::from_ref(&range),
            "XYZ",
            0
        ));

        assert_eq!(buffer.text(), "abc123def");
    }

    #[test]
    fn move_selected_text_by_drag_rejects_out_of_bounds_ranges() {
        let mut buffer = TextBuffer::from_text(1, None, "abc".to_owned());
        let range = 1..99;

        assert!(!move_selected_text_by_drag(
            &mut buffer,
            std::slice::from_ref(&range),
            "bc",
            0
        ));

        assert_eq!(buffer.text(), "abc");
    }

    #[test]
    fn move_selected_text_by_drag_rejects_overlapping_ranges() {
        let mut buffer = TextBuffer::from_text(1, None, "abcdef".to_owned());
        let ranges = vec![1..4, 3..5];

        assert!(!move_selected_text_by_drag(
            &mut buffer,
            &ranges,
            "bcd\nde",
            6
        ));

        assert_eq!(buffer.text(), "abcdef");
    }

    #[test]
    fn move_selected_text_by_drag_clamps_drop_before_selection_check() {
        let mut buffer = TextBuffer::from_text(1, None, "abcXYZ".to_owned());
        buffer.set_selection(3, 6);
        let range = 3..6;

        assert!(!move_selected_text_by_drag(
            &mut buffer,
            std::slice::from_ref(&range),
            "XYZ",
            usize::MAX
        ));

        assert_eq!(buffer.text(), "abcXYZ");
        assert_eq!(buffer.selected_text().as_deref(), Some("XYZ"));
    }
}
