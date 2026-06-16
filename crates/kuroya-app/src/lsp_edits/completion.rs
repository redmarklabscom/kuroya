use crate::{
    editor_input::editor_text_input_from_event,
    lsp_edits::{buffer_text_edits_from_lsp, sort_buffer_edits_by_range_and_reject_overlaps},
    workspace_state::paths_match_lexically,
};
use eframe::egui::{Event, Key};
use kuroya_core::{
    EditorSuggestInsertMode, LspCompletionItem, TextBuffer, TextEdit as BufferTextEdit,
    buffer::AutoPairSettings,
};
use std::ops::Range;

const MAX_COMPLETION_SNIPPET_TABSTOP_GROUPS: usize = 128;
const MAX_COMPLETION_SNIPPET_TABSTOPS: usize = 512;
const MAX_COMPLETION_ADDITIONAL_TEXT_EDITS: usize = 512;
const MAX_COMPLETION_TEXT_EDIT_NEW_TEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_COMPLETION_ADDITIONAL_TEXT_EDIT_TOTAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionBufferEditPlan {
    pub(crate) edits: Vec<BufferTextEdit>,
    pub(crate) primary_edit: Option<BufferTextEdit>,
    pub(crate) snippet_selection: Option<Range<usize>>,
    pub(crate) snippet_tabstops: Vec<Range<usize>>,
    pub(crate) snippet_tabstop_groups: Vec<Vec<Range<usize>>>,
}

#[cfg(test)]
pub(crate) fn completion_buffer_edits(
    buffer: &TextBuffer,
    item: &LspCompletionItem,
    insert_mode: EditorSuggestInsertMode,
) -> Option<Vec<BufferTextEdit>> {
    completion_buffer_edit_plan(buffer, item, insert_mode).map(|plan| plan.edits)
}

pub(crate) fn completion_buffer_edit_plan(
    buffer: &TextBuffer,
    item: &LspCompletionItem,
    insert_mode: EditorSuggestInsertMode,
) -> Option<CompletionBufferEditPlan> {
    if !completion_lsp_text_edits_are_bounded(&item.additional_text_edits) {
        return None;
    }
    let mut edits = completion_lsp_buffer_edits(buffer, &item.additional_text_edits)?;
    let primary_edit = if let Some(edit) = completion_text_edit_for_insert_mode(item, insert_mode) {
        if !completion_lsp_text_edit_is_bounded(edit) {
            return None;
        }
        let primary_edit = completion_lsp_buffer_edits(buffer, std::slice::from_ref(edit))?
            .into_iter()
            .next()?;
        edits.push(primary_edit.clone());
        primary_edit
    } else {
        if item.insert_text.len() > MAX_COMPLETION_TEXT_EDIT_NEW_TEXT_BYTES {
            return None;
        }
        let range = completion_insert_text_range(buffer, insert_mode)?;
        let edit = BufferTextEdit {
            range,
            inserted: item.insert_text.clone(),
        };
        edits.push(edit);
        edits.last().cloned()?
    };
    if !sort_buffer_edits_by_range_and_reject_overlaps(&mut edits) {
        return None;
    }
    let snippet_metadata = completion_snippet_metadata(item, &primary_edit);
    Some(CompletionBufferEditPlan {
        edits,
        primary_edit: Some(primary_edit),
        snippet_selection: snippet_metadata.selection,
        snippet_tabstops: snippet_metadata.tabstops,
        snippet_tabstop_groups: snippet_metadata.tabstop_groups,
    })
}

#[derive(Debug, Default)]
struct CompletionSnippetMetadata {
    selection: Option<Range<usize>>,
    tabstops: Vec<Range<usize>>,
    tabstop_groups: Vec<Vec<Range<usize>>>,
}

fn completion_snippet_metadata(
    item: &LspCompletionItem,
    primary_edit: &BufferTextEdit,
) -> CompletionSnippetMetadata {
    let inserted_len = primary_edit.inserted.chars().count();
    let selection = item
        .snippet_selection
        .as_ref()
        .filter(|range| completion_snippet_range_valid(range, inserted_len))
        .cloned();
    let tabstops = completion_snippet_ranges(&item.snippet_tabstops, inserted_len);
    let Some(tabstop_groups) =
        completion_snippet_tabstop_groups(&item.snippet_tabstop_groups, inserted_len)
    else {
        return CompletionSnippetMetadata {
            selection,
            ..Default::default()
        };
    };

    if tabstop_groups.is_empty() {
        return CompletionSnippetMetadata {
            selection,
            tabstops,
            tabstop_groups,
        };
    }

    let grouped_tabstops = completion_flatten_snippet_tabstop_groups(&tabstop_groups);
    if !completion_snippet_ranges_are_ordered(&grouped_tabstops) {
        return CompletionSnippetMetadata {
            selection,
            ..Default::default()
        };
    }
    let primary_grouped_tabstops = completion_first_snippet_tabstops(&tabstop_groups);
    if !item.snippet_tabstops.is_empty()
        && tabstops != grouped_tabstops
        && tabstops != primary_grouped_tabstops
    {
        return CompletionSnippetMetadata {
            selection,
            ..Default::default()
        };
    }

    CompletionSnippetMetadata {
        selection,
        tabstops: grouped_tabstops,
        tabstop_groups,
    }
}

fn completion_snippet_ranges(ranges: &[Range<usize>], inserted_len: usize) -> Vec<Range<usize>> {
    if !completion_snippet_ranges_are_valid(ranges, inserted_len) {
        return Vec::new();
    }

    let output = ranges.to_vec();
    if completion_snippet_ranges_are_ordered(&output) {
        output
    } else {
        Vec::new()
    }
}

fn completion_snippet_tabstop_groups(
    groups: &[Vec<Range<usize>>],
    inserted_len: usize,
) -> Option<Vec<Vec<Range<usize>>>> {
    if groups.len() > MAX_COMPLETION_SNIPPET_TABSTOP_GROUPS {
        return None;
    }

    let mut output = Vec::new();
    let mut total_ranges = 0usize;
    for group in groups {
        if total_ranges.saturating_add(group.len()) > MAX_COMPLETION_SNIPPET_TABSTOPS {
            return None;
        }
        if !completion_snippet_ranges_are_valid(group, inserted_len) {
            return None;
        }
        let ranges = completion_snippet_ranges(group, inserted_len);
        total_ranges += ranges.len();
        if !ranges.is_empty() {
            output.push(ranges);
        }
    }
    Some(output)
}

fn completion_flatten_snippet_tabstop_groups(groups: &[Vec<Range<usize>>]) -> Vec<Range<usize>> {
    let mut output = Vec::with_capacity(groups.iter().map(Vec::len).sum());
    for group in groups {
        output.extend(group.iter().cloned());
    }
    output
}

fn completion_first_snippet_tabstops(groups: &[Vec<Range<usize>>]) -> Vec<Range<usize>> {
    groups
        .iter()
        .filter_map(|group| group.first().cloned())
        .collect()
}

fn completion_snippet_ranges_are_valid(ranges: &[Range<usize>], inserted_len: usize) -> bool {
    ranges.len() <= MAX_COMPLETION_SNIPPET_TABSTOPS
        && ranges
            .iter()
            .all(|range| completion_snippet_range_valid(range, inserted_len))
}

fn completion_snippet_range_valid(range: &Range<usize>, inserted_len: usize) -> bool {
    range.start <= range.end && range.end <= inserted_len
}

fn completion_snippet_ranges_are_ordered(ranges: &[Range<usize>]) -> bool {
    let mut previous_end = None;
    for range in ranges {
        if let Some(end) = previous_end
            && range.start < end
        {
            return false;
        }
        previous_end = Some(range.end);
    }
    true
}

fn completion_lsp_buffer_edits(
    buffer: &TextBuffer,
    edits: &[kuroya_core::LspTextEdit],
) -> Option<Vec<BufferTextEdit>> {
    if !completion_lsp_text_edits_target_buffer(buffer, edits) {
        return None;
    }
    buffer_text_edits_from_lsp(buffer, edits)
}

fn completion_lsp_text_edits_target_buffer(
    buffer: &TextBuffer,
    edits: &[kuroya_core::LspTextEdit],
) -> bool {
    if edits.is_empty() {
        return true;
    }
    let Some(path) = buffer.path() else {
        return false;
    };
    edits
        .iter()
        .all(|edit| paths_match_lexically(&edit.path, path))
}

fn completion_lsp_text_edits_are_bounded(edits: &[kuroya_core::LspTextEdit]) -> bool {
    if edits.len() > MAX_COMPLETION_ADDITIONAL_TEXT_EDITS {
        return false;
    }

    let mut total_new_text_bytes = 0usize;
    edits.iter().all(|edit| {
        let Some(total) = total_new_text_bytes.checked_add(edit.new_text.len()) else {
            return false;
        };
        total_new_text_bytes = total;
        completion_lsp_text_edit_is_bounded(edit)
            && total_new_text_bytes <= MAX_COMPLETION_ADDITIONAL_TEXT_EDIT_TOTAL_BYTES
    })
}

fn completion_lsp_text_edit_is_bounded(edit: &kuroya_core::LspTextEdit) -> bool {
    edit.new_text.len() <= MAX_COMPLETION_TEXT_EDIT_NEW_TEXT_BYTES
}

fn completion_text_edit_for_insert_mode(
    item: &LspCompletionItem,
    insert_mode: EditorSuggestInsertMode,
) -> Option<&kuroya_core::LspTextEdit> {
    match insert_mode {
        EditorSuggestInsertMode::Insert => {
            item.insert_text_edit.as_ref().or(item.text_edit.as_ref())
        }
        EditorSuggestInsertMode::Replace => item.text_edit.as_ref(),
    }
}

fn completion_insert_text_range(
    buffer: &TextBuffer,
    insert_mode: EditorSuggestInsertMode,
) -> Option<std::ops::Range<usize>> {
    let prefix = buffer.completion_prefix_range()?;
    if matches!(insert_mode, EditorSuggestInsertMode::Replace)
        && let Some(word) = buffer.word_range_at_cursor()
        && word.start <= prefix.start
        && prefix.end <= word.end
    {
        return Some(word);
    }
    Some(prefix)
}

#[cfg(test)]
pub(crate) fn completion_passthrough_edit_intent(events: &[Event]) -> bool {
    completion_passthrough_edit_intent_with_acceptance(events, true, true)
}

#[cfg(test)]
pub(crate) fn completion_passthrough_edit_intent_with_acceptance(
    events: &[Event],
    accept_suggestion_on_enter: bool,
    accept_suggestion_on_tab: bool,
) -> bool {
    events.iter().any(|event| match event {
        event if editor_text_input_from_event(event).is_some() => true,
        Event::Paste(_) => true,
        Event::Key {
            key: Key::Backspace | Key::Delete,
            pressed: true,
            modifiers,
            ..
        } => !modifiers.ctrl && !modifiers.command && !modifiers.alt,
        Event::Key {
            key: Key::Enter,
            pressed: true,
            modifiers,
            ..
        } => !accept_suggestion_on_enter && !modifiers.ctrl && !modifiers.command && !modifiers.alt,
        Event::Key {
            key: Key::Tab,
            pressed: true,
            modifiers,
            ..
        } => !accept_suggestion_on_tab && !modifiers.ctrl && !modifiers.command && !modifiers.alt,
        _ => false,
    })
}

#[cfg(test)]
pub(crate) fn apply_completion_passthrough_events(
    buffer: &mut TextBuffer,
    events: &[Event],
    auto_pair_settings: AutoPairSettings,
) -> bool {
    apply_completion_passthrough_events_inner(buffer, events, auto_pair_settings, None)
}

pub(crate) fn apply_completion_passthrough_events_with_editor_keys(
    buffer: &mut TextBuffer,
    events: &[Event],
    auto_pair_settings: AutoPairSettings,
    tab: &str,
    auto_indent: bool,
) -> bool {
    apply_completion_passthrough_events_inner(
        buffer,
        events,
        auto_pair_settings,
        Some((tab, auto_indent)),
    )
}

fn apply_completion_passthrough_events_inner(
    buffer: &mut TextBuffer,
    events: &[Event],
    auto_pair_settings: AutoPairSettings,
    editor_keys: Option<(&str, bool)>,
) -> bool {
    let mut changed = false;
    for event in events {
        if let Some(text) = editor_text_input_from_event(event) {
            changed |= buffer.insert_text_with_auto_pair_settings(text, auto_pair_settings);
            continue;
        }

        match event {
            Event::Paste(text) => {
                buffer.insert_at_cursors(text);
                changed = true;
            }
            Event::Key {
                key: Key::Backspace,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.ctrl && !modifiers.command && !modifiers.alt => {
                changed |= buffer.delete_backward();
            }
            Event::Key {
                key: Key::Delete,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.ctrl && !modifiers.command && !modifiers.alt => {
                changed |= buffer.delete_forward();
            }
            Event::Key {
                key: Key::Enter,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.ctrl && !modifiers.command && !modifiers.alt => {
                if let Some((tab, auto_indent)) = editor_keys {
                    if auto_indent {
                        buffer.insert_newline_with_indent_unit(tab);
                    } else {
                        buffer.insert_at_cursors("\n");
                    }
                    changed = true;
                }
            }
            Event::Key {
                key: Key::Tab,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.ctrl && !modifiers.command && !modifiers.alt => {
                if let Some((tab, _)) = editor_keys {
                    if modifiers.shift {
                        changed |= buffer.outdent_lines(tab);
                    } else if buffer.has_selection() {
                        changed |= buffer.indent_lines(tab);
                    } else {
                        buffer.insert_at_cursors(tab);
                        changed = true;
                    }
                }
            }
            _ => {}
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_COMPLETION_ADDITIONAL_TEXT_EDIT_TOTAL_BYTES, MAX_COMPLETION_ADDITIONAL_TEXT_EDITS,
        MAX_COMPLETION_SNIPPET_TABSTOP_GROUPS, MAX_COMPLETION_SNIPPET_TABSTOPS,
        MAX_COMPLETION_TEXT_EDIT_NEW_TEXT_BYTES, completion_buffer_edit_plan,
    };
    use kuroya_core::{EditorSuggestInsertMode, LspCompletionItem, LspTextEdit, TextBuffer};
    use std::{ops::Range, path::PathBuf};

    fn completion_item(insert_text: &str) -> LspCompletionItem {
        LspCompletionItem {
            label: "snippet".to_owned(),
            detail: None,
            documentation: None,
            kind: Some(15),
            deprecated: false,
            is_snippet: true,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: insert_text.to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: None,
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: None,
        }
    }

    fn buffer_at_end(text: &str) -> TextBuffer {
        let mut buffer =
            TextBuffer::from_text(1, Some(PathBuf::from("src/lib.rs")), text.to_owned());
        buffer.set_single_cursor(buffer.len_chars());
        buffer
    }

    #[test]
    fn completion_plan_drops_stale_snippet_ranges_without_rewriting_insert_text() {
        let buffer = buffer_at_end("pri");
        let mut item = completion_item("println!(value);");
        item.snippet_selection = Some(100..105);
        item.snippet_tabstops = std::iter::once(100..105).collect();
        item.snippet_tabstop_groups = vec![std::iter::once(100..105).collect()];
        let raw_item = item.clone();

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert_eq!(
            plan.primary_edit
                .as_ref()
                .map(|edit| edit.inserted.as_str()),
            Some("println!(value);")
        );
        assert_eq!(plan.snippet_selection, None);
        assert!(plan.snippet_tabstops.is_empty());
        assert!(plan.snippet_tabstop_groups.is_empty());
        assert_eq!(item, raw_item);
    }

    #[test]
    fn completion_plan_bounds_oversized_snippet_placeholder_groups() {
        let buffer = buffer_at_end("");
        let mut item = completion_item("x");
        item.snippet_tabstop_groups = (0..MAX_COMPLETION_SNIPPET_TABSTOP_GROUPS + 1)
            .map(|_| std::iter::once(0..0).collect())
            .collect();

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert!(plan.snippet_tabstops.is_empty());
        assert!(plan.snippet_tabstop_groups.is_empty());
        assert_eq!(
            plan.primary_edit
                .as_ref()
                .map(|edit| edit.inserted.as_str()),
            Some("x")
        );
    }

    #[test]
    fn completion_plan_rejects_inconsistent_flat_and_grouped_snippet_metadata() {
        let buffer = buffer_at_end("");
        let mut item = completion_item("abcdef");
        item.snippet_tabstops = vec![0..1, 4..5];
        item.snippet_tabstop_groups = vec![vec![0..1], vec![2..3]];

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert!(plan.snippet_tabstops.is_empty());
        assert!(plan.snippet_tabstop_groups.is_empty());
    }

    #[test]
    fn completion_plan_preserves_valid_grouped_snippet_metadata() {
        let buffer = buffer_at_end("");
        let mut item = completion_item("println!(value, other);");
        item.snippet_selection = Some(9..14);
        item.snippet_tabstops = vec![9..14, 16..21, 23..23];
        item.snippet_tabstop_groups = vec![vec![9..14], vec![16..21], vec![23..23]];

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert_eq!(plan.snippet_selection, Some(9..14));
        assert_eq!(plan.snippet_tabstops, vec![9..14, 16..21, 23..23]);
        assert_eq!(
            plan.snippet_tabstop_groups,
            vec![vec![9..14], vec![16..21], vec![23..23]]
        );
    }

    #[test]
    fn completion_plan_preserves_repeated_placeholder_groups() {
        let buffer = buffer_at_end("");
        let mut item = completion_item("foo/foo");
        item.snippet_selection = Some(0..3);
        item.snippet_tabstops = std::iter::once(0..3).collect();
        item.snippet_tabstop_groups = vec![vec![0..3, 4..7]];

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert_eq!(plan.snippet_selection, Some(0..3));
        assert_eq!(plan.snippet_tabstops, vec![0..3, 4..7]);
        assert_eq!(plan.snippet_tabstop_groups, vec![vec![0..3, 4..7]]);
    }

    #[test]
    fn completion_plan_rejects_invalid_grouped_snippet_ranges() {
        let buffer = buffer_at_end("");
        let mut item = completion_item("foo");
        item.snippet_selection = Some(0..3);
        item.snippet_tabstops = std::iter::once(0..3).collect();
        item.snippet_tabstop_groups = vec![vec![0..3, 4..7]];

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert_eq!(plan.snippet_selection, Some(0..3));
        assert!(plan.snippet_tabstops.is_empty());
        assert!(plan.snippet_tabstop_groups.is_empty());
    }

    #[test]
    fn completion_plan_rejects_unordered_grouped_snippet_ranges() {
        let buffer = buffer_at_end("");
        let mut item = completion_item("abcdef");
        item.snippet_tabstop_groups = vec![vec![4..5], vec![0..1]];

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert!(plan.snippet_tabstops.is_empty());
        assert!(plan.snippet_tabstop_groups.is_empty());
    }

    #[test]
    fn completion_plan_bounds_flat_snippet_placeholder_count() {
        let buffer = buffer_at_end("");
        let mut item = completion_item("x");
        item.snippet_tabstops = (0..MAX_COMPLETION_SNIPPET_TABSTOPS + 1)
            .map(|_| Range { start: 0, end: 0 })
            .collect();

        let plan =
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).unwrap();

        assert!(plan.snippet_tabstops.is_empty());
    }

    #[test]
    fn completion_plan_rejects_too_many_additional_text_edits() {
        let buffer = buffer_at_end("Hash");
        let mut item = completion_item("HashMap");
        item.additional_text_edits = (0..=MAX_COMPLETION_ADDITIONAL_TEXT_EDITS)
            .map(|_| empty_lsp_edit())
            .collect();

        assert!(
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).is_none()
        );
    }

    #[test]
    fn completion_plan_rejects_aggregate_oversized_additional_text_edits() {
        let buffer = buffer_at_end("Hash");
        let mut item = completion_item("HashMap");
        let chunk = "x".repeat((MAX_COMPLETION_ADDITIONAL_TEXT_EDIT_TOTAL_BYTES / 2) + 1);
        item.additional_text_edits = vec![
            LspTextEdit {
                new_text: chunk.clone(),
                ..empty_lsp_edit()
            },
            LspTextEdit {
                new_text: chunk,
                ..empty_lsp_edit()
            },
        ];

        assert!(
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).is_none()
        );
    }

    #[test]
    fn completion_plan_rejects_oversized_primary_insert_text() {
        let buffer = buffer_at_end("");
        let item = completion_item(&"x".repeat(MAX_COMPLETION_TEXT_EDIT_NEW_TEXT_BYTES + 1));

        assert!(
            completion_buffer_edit_plan(&buffer, &item, EditorSuggestInsertMode::Insert).is_none()
        );
    }

    fn empty_lsp_edit() -> LspTextEdit {
        LspTextEdit {
            path: PathBuf::from("src/lib.rs"),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: String::new(),
        }
    }
}
