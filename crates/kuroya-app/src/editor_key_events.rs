use crate::KuroyaApp;
use crate::{
    editor_focus_runtime::editor_tab_focus_mode_should_release_focus,
    editor_key_events::modified::handle_modified_editor_key_event,
};
use eframe::egui::{Key, Modifiers};
use kuroya_core::{BufferId, Selection, TextBuffer, TextEdit};
use std::ops::Range;

mod modified;

pub(crate) fn handle_editor_key_event(
    app: &mut KuroyaApp,
    buffer_id: BufferId,
    key: Key,
    modifiers: Modifiers,
    tab: &str,
    auto_indent: bool,
    tab_focus_mode: bool,
    tab_size: usize,
    insert_spaces: bool,
    use_tab_stops: bool,
    trim_whitespace_on_delete: bool,
    auto_closing_delete: bool,
    changed: &mut bool,
) -> bool {
    if handle_modified_editor_key_event(app, buffer_id, key, modifiers, changed) {
        return false;
    }

    if key == Key::Enter && auto_indent {
        *changed |= app.insert_newline_with_auto_indent_for_buffer(buffer_id, tab);
        return false;
    }
    if key == Key::Tab
        && !modifiers.ctrl
        && !modifiers.command
        && !modifiers.alt
        && app.move_snippet_session_for_buffer(buffer_id, modifiers.shift)
    {
        return false;
    }
    if editor_tab_focus_mode_should_release_focus(tab_focus_mode, key, modifiers) {
        return true;
    }
    if key == Key::Tab && tab_focus_mode {
        return false;
    }

    if let Some(buffer) = app.buffer_mut(buffer_id) {
        match key {
            Key::ArrowLeft if modifiers.shift => buffer.extend_left(),
            Key::ArrowLeft => buffer.move_left(),
            Key::ArrowRight if modifiers.shift => buffer.extend_right(),
            Key::ArrowRight => buffer.move_right(),
            Key::ArrowUp if modifiers.shift => buffer.extend_up(),
            Key::ArrowUp => buffer.move_up(),
            Key::ArrowDown if modifiers.shift => buffer.extend_down(),
            Key::ArrowDown => buffer.move_down(),
            Key::Home if modifiers.shift => buffer.extend_line_start(),
            Key::Home => buffer.move_line_start(),
            Key::End if modifiers.shift => buffer.extend_line_end(),
            Key::End => buffer.move_line_end(),
            Key::Backspace if insert_spaces && use_tab_stops => {
                *changed |= delete_tab_stop_backward(buffer, tab_size, auto_closing_delete);
            }
            Key::Backspace => {
                *changed |= buffer.delete_backward_with_auto_pair_delete(auto_closing_delete);
            }
            Key::Delete if trim_whitespace_on_delete => {
                *changed |= buffer.delete_forward_with_trim_whitespace_on_delete();
            }
            Key::Delete => *changed |= buffer.delete_forward(),
            Key::Enter => {
                buffer.insert_at_cursors("\n");
                *changed = true;
            }
            Key::Tab if modifiers.shift => {
                *changed |= buffer.outdent_lines(tab);
            }
            Key::Tab if buffer.has_selection() => {
                *changed |= buffer.indent_lines(tab);
            }
            Key::Tab => {
                *changed |=
                    insert_tab_at_cursors(buffer, tab, tab_size, insert_spaces, use_tab_stops);
            }
            _ => {}
        }
    }

    false
}

pub(crate) fn insert_tab_at_cursors(
    buffer: &mut TextBuffer,
    tab: &str,
    tab_size: usize,
    insert_spaces: bool,
    use_tab_stops: bool,
) -> bool {
    if !insert_spaces || !use_tab_stops {
        buffer.insert_at_cursors(tab);
        return true;
    }

    let tab_size = tab_size.max(1);
    let edits = buffer
        .selections()
        .iter()
        .map(|selection| {
            let pos = buffer.char_position(selection.cursor);
            let visual_column = line_prefix_visual_width(buffer, pos.line, pos.column, tab_size);
            let remainder = visual_column % tab_size;
            let spaces = if remainder == 0 {
                tab_size
            } else {
                tab_size - remainder
            };
            TextEdit {
                range: selection.range(),
                inserted: " ".repeat(spaces),
            }
        })
        .collect();
    buffer.apply_edits(edits)
}

pub(crate) fn delete_tab_stop_backward(
    buffer: &mut TextBuffer,
    tab_size: usize,
    auto_closing_delete: bool,
) -> bool {
    let tab_size = tab_size.max(1);
    let selections = buffer.selections();
    let Some((first_tab_stop_index, first_tab_stop_range)) = selections
        .iter()
        .copied()
        .enumerate()
        .find_map(|(index, selection)| {
            delete_tab_stop_backward_range(buffer, selection, tab_size).map(|range| (index, range))
        })
    else {
        return buffer.delete_backward_with_auto_pair_delete(auto_closing_delete);
    };

    let mut edits = Vec::with_capacity(selections.len());
    for &selection in &selections[..first_tab_stop_index] {
        if let Some(range) = delete_backward_range(buffer, selection, auto_closing_delete) {
            edits.push(TextEdit {
                range,
                inserted: String::new(),
            });
        }
    }
    edits.push(TextEdit {
        range: first_tab_stop_range,
        inserted: String::new(),
    });
    for &selection in &selections[first_tab_stop_index + 1..] {
        if let Some(range) = delete_tab_stop_backward_range(buffer, selection, tab_size) {
            edits.push(TextEdit {
                range,
                inserted: String::new(),
            });
        } else if let Some(range) = delete_backward_range(buffer, selection, auto_closing_delete) {
            edits.push(TextEdit {
                range,
                inserted: String::new(),
            });
        }
    }

    buffer.apply_edits(edits)
}

fn delete_tab_stop_backward_range(
    buffer: &TextBuffer,
    selection: Selection,
    tab_size: usize,
) -> Option<Range<usize>> {
    if !selection.is_caret() {
        return None;
    }

    let cursor = selection.cursor;
    if cursor == 0 {
        return None;
    }
    if !matches!(buffer.char_at(cursor - 1), Some(' ')) {
        return None;
    }

    let pos = buffer.char_position(cursor);
    let prefix = line_prefix_metrics(buffer, pos.line, pos.column, tab_size)?;
    let trailing_spaces = prefix.trailing_spaces;
    if trailing_spaces == 0 {
        return None;
    }

    let previous_stop = prefix.visual_width % tab_size;
    let delete_spaces = if previous_stop == 0 {
        tab_size
    } else {
        previous_stop
    }
    .min(trailing_spaces);
    Some(cursor.saturating_sub(delete_spaces)..cursor)
}

fn delete_backward_range(
    buffer: &TextBuffer,
    selection: Selection,
    auto_closing_delete: bool,
) -> Option<Range<usize>> {
    let range = selection.range();
    if !selection.is_caret() {
        return Some(range);
    }

    let cursor = selection.cursor;
    if cursor == 0 {
        return None;
    }

    let start = cursor - 1;
    let end = if auto_closing_delete {
        auto_pair_delete_end(buffer, cursor).unwrap_or(cursor)
    } else {
        cursor
    };
    Some(start..end)
}

fn auto_pair_delete_end(buffer: &TextBuffer, cursor: usize) -> Option<usize> {
    if cursor == 0 || cursor >= buffer.len_chars() {
        return None;
    }

    let open = buffer_char(buffer, cursor - 1)?;
    let close = buffer_char(buffer, cursor)?;
    buffer
        .language()
        .configuration()
        .auto_closing_pairs()
        .iter()
        .any(|pair| pair.open == open && pair.close == close)
        .then_some(cursor + 1)
}

fn buffer_char(buffer: &TextBuffer, char_idx: usize) -> Option<char> {
    buffer.char_at(char_idx)
}

fn line_prefix_visual_width(
    buffer: &TextBuffer,
    line: usize,
    column: usize,
    tab_size: usize,
) -> usize {
    line_prefix_metrics(buffer, line, column, tab_size)
        .map(|prefix| prefix.visual_width)
        .unwrap_or(column)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LinePrefixMetrics {
    visual_width: usize,
    trailing_spaces: usize,
}

fn line_prefix_metrics(
    buffer: &TextBuffer,
    line: usize,
    column: usize,
    tab_size: usize,
) -> Option<LinePrefixMetrics> {
    if line >= buffer.len_lines() {
        return None;
    }

    let tab_size = tab_size.max(1);
    let start = buffer.line_column_to_char(line, 0);
    let end = buffer.line_column_to_char(line, column);
    let mut visual_width = 0usize;
    let mut trailing_spaces = 0usize;
    for char_idx in start..end {
        let Some(ch) = buffer.char_at(char_idx) else {
            break;
        };
        match ch {
            ' ' => {
                visual_width = visual_width.saturating_add(1);
                trailing_spaces = trailing_spaces.saturating_add(1);
            }
            '\t' => {
                let remainder = visual_width % tab_size;
                visual_width = visual_width.saturating_add(if remainder == 0 {
                    tab_size
                } else {
                    tab_size - remainder
                });
                trailing_spaces = 0;
            }
            _ => {
                visual_width = visual_width.saturating_add(1);
                trailing_spaces = 0;
            }
        }
    }

    Some(LinePrefixMetrics {
        visual_width,
        trailing_spaces,
    })
}

#[cfg(test)]
mod tests {
    use super::{delete_tab_stop_backward, insert_tab_at_cursors};
    use kuroya_core::TextBuffer;

    #[test]
    fn insert_tab_at_cursors_uses_spaces_to_next_tab_stop() {
        let mut buffer = TextBuffer::from_text(1, None, "ab".to_owned());
        buffer.set_single_cursor(2);

        assert!(insert_tab_at_cursors(&mut buffer, "    ", 4, true, true));
        assert_eq!(buffer.text(), "ab  ");
    }

    #[test]
    fn insert_tab_at_cursors_can_keep_fixed_indent_unit() {
        let mut buffer = TextBuffer::from_text(1, None, "ab".to_owned());
        buffer.set_single_cursor(2);

        assert!(insert_tab_at_cursors(&mut buffer, "    ", 4, true, false));
        assert_eq!(buffer.text(), "ab    ");
    }

    #[test]
    fn insert_tab_at_cursors_accounts_for_tabs_without_prefix_allocation() {
        let mut buffer = TextBuffer::from_text(1, None, "\tvalue".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, 1));

        assert!(insert_tab_at_cursors(&mut buffer, "    ", 4, true, true));
        assert_eq!(buffer.text(), "\t    value");
    }

    #[test]
    fn delete_tab_stop_backward_removes_spaces_to_previous_stop() {
        let mut buffer = TextBuffer::from_text(1, None, "      value".to_owned());
        buffer.set_single_cursor(6);

        assert!(delete_tab_stop_backward(&mut buffer, 4, true));
        assert_eq!(buffer.text(), "    value");
    }

    #[test]
    fn delete_tab_stop_backward_accounts_for_tabs_without_prefix_allocation() {
        let mut buffer = TextBuffer::from_text(1, None, "\t  value".to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(0, 3));

        assert!(delete_tab_stop_backward(&mut buffer, 4, true));
        assert_eq!(buffer.text(), "\tvalue");
    }

    #[test]
    fn delete_tab_stop_backward_preserves_disabled_auto_pair_delete() {
        let mut buffer = TextBuffer::from_text(1, None, "()".to_owned());
        buffer.set_single_cursor(1);

        assert!(delete_tab_stop_backward(&mut buffer, 4, false));
        assert_eq!(buffer.text(), ")");
    }

    #[test]
    fn delete_tab_stop_backward_uses_plain_delete_when_no_tab_stop_matches() {
        let mut buffer = TextBuffer::from_text(1, None, "()".to_owned());
        buffer.set_single_cursor(1);

        assert!(delete_tab_stop_backward(&mut buffer, 4, true));
        assert_eq!(buffer.text(), "");
    }

    #[test]
    fn delete_tab_stop_backward_handles_mixed_multicursor_backspace() {
        let mut buffer = TextBuffer::from_text(1, None, "    foo\nbar".to_owned());
        buffer.set_cursors([
            buffer.line_column_to_char(0, 4),
            buffer.line_column_to_char(1, 1),
        ]);

        assert!(delete_tab_stop_backward(&mut buffer, 4, true));
        assert_eq!(buffer.text(), "foo\nar");
        assert_eq!(
            buffer
                .cursor_positions()
                .into_iter()
                .map(|position| (position.line, position.column))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 0)]
        );
    }

    #[test]
    fn delete_tab_stop_backward_handles_late_tab_stop_match() {
        let mut buffer = TextBuffer::from_text(1, None, "x\n    foo".to_owned());
        buffer.set_cursors([1, buffer.line_column_to_char(1, 4)]);

        assert!(delete_tab_stop_backward(&mut buffer, 4, true));
        assert_eq!(buffer.text(), "\nfoo");
        assert_eq!(
            buffer
                .cursor_positions()
                .into_iter()
                .map(|position| (position.line, position.column))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 0)]
        );
    }

    #[test]
    fn delete_tab_stop_backward_handles_mixed_multicursor_auto_pair_delete() {
        let mut buffer = TextBuffer::from_text(1, None, "    foo\n()bar".to_owned());
        buffer.set_cursors([
            buffer.line_column_to_char(0, 4),
            buffer.line_column_to_char(1, 1),
        ]);

        assert!(delete_tab_stop_backward(&mut buffer, 4, true));
        assert_eq!(buffer.text(), "foo\nbar");
        assert_eq!(
            buffer
                .cursor_positions()
                .into_iter()
                .map(|position| (position.line, position.column))
                .collect::<Vec<_>>(),
            vec![(0, 0), (1, 0)]
        );
    }
}
