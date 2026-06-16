use super::content::{
    completion_commit_text, completion_selected_item, completion_tab_accepts,
    normalize_completion_selection,
};
use crate::{
    KuroyaApp, editor_input::editor_text_input_from_event,
    lsp_edits::apply_completion_passthrough_events_with_editor_keys,
};
use eframe::egui::{Context, Event, Key};
use kuroya_core::{LspCompletionItem, buffer::AutoPairSettings};

impl KuroyaApp {
    pub(super) fn apply_completion_passthrough_input(&mut self, ctx: &Context) -> bool {
        let selected_item = completion_passthrough_selected_item(
            &mut self.completion_selected,
            &self.completion_items,
        );
        let tab_accepts = selected_item
            .map(|(_, item)| completion_tab_accepts(item, &self.settings))
            .unwrap_or(self.settings.accept_suggestion_on_tab);
        let input = ctx.input(|input| {
            let commit_text = selected_item
                .and_then(|(_, item)| completion_commit_text(item, &self.settings, &input.events));
            let close_for_tab_focus =
                self.settings.tab_focus_mode && !tab_accepts && tab_focus_event(&input.events);
            let edit_events = if commit_text.is_none() && !close_for_tab_focus {
                completion_passthrough_edit_events(
                    &input.events,
                    self.settings.accept_suggestion_on_enter,
                    tab_accepts,
                )
            } else {
                None
            };

            CompletionPassthroughInput {
                commit_text,
                close_for_tab_focus,
                edit_events,
            }
        });
        let commit_apply = input
            .commit_text
            .and_then(|commit_text| selected_item.map(|(_, item)| (item.clone(), commit_text)));
        if let Some((item, commit_text)) = commit_apply {
            self.apply_completion_item_with_commit(item, Some(commit_text));
            return true;
        }
        if input.close_for_tab_focus {
            self.clear_completion_popup_state();
            self.status = "Closed completions for Tab focus navigation".to_owned();
            return true;
        }
        let Some(events) = input.edit_events else {
            return false;
        };

        let Some(id) = self.active else {
            self.clear_completion_popup_state();
            self.status = "Closed completions; no active file".to_owned();
            return true;
        };
        if self.block_protected_preview_edit(id) {
            self.clear_completion_popup_state();
            return true;
        }

        let auto_pair_settings = AutoPairSettings {
            brackets: self.settings.auto_closing_brackets,
            quotes: self.settings.auto_closing_quotes,
            surround: self.settings.auto_surround,
            overtype: !matches!(
                self.settings.auto_closing_overtype,
                kuroya_core::EditorAutoClosingEditStrategy::Never
            ),
        };
        let tab = self.indent_options_for_buffer(id).unit;
        let auto_indent = self.settings.auto_indent;
        let changed = self.buffer_mut(id).is_some_and(|buffer| {
            apply_completion_passthrough_events_with_editor_keys(
                buffer,
                &events,
                auto_pair_settings,
                &tab,
                auto_indent,
            )
        });
        self.clear_completion_popup_state();
        if changed {
            self.mark_buffer_changed(id);
            if events
                .iter()
                .any(|event| matches!(event, eframe::egui::Event::Paste(_)))
                && self.settings.paste_as_enabled
                && self.settings.format_on_paste
            {
                let _ =
                    self.request_lsp_formatting_for_buffer(id, Some("Formatting paste in"), false);
            }
            self.status = "Closed completions while editing".to_owned();
        } else {
            self.status = "Closed completions".to_owned();
        }
        true
    }
}

struct CompletionPassthroughInput {
    commit_text: Option<String>,
    close_for_tab_focus: bool,
    edit_events: Option<Vec<Event>>,
}

fn completion_passthrough_selected_item<'a>(
    selected: &mut usize,
    items: &'a [LspCompletionItem],
) -> Option<(usize, &'a LspCompletionItem)> {
    normalize_completion_selection(selected, items.len());
    completion_selected_item(items, *selected)
}

fn tab_focus_event(events: &[Event]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            Event::Key {
                key: Key::Tab,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.ctrl && !modifiers.command && !modifiers.alt
        )
    })
}

fn completion_passthrough_edit_events(
    events: &[Event],
    accept_suggestion_on_enter: bool,
    accept_suggestion_on_tab: bool,
) -> Option<Vec<Event>> {
    let mut edit_events = None;
    for event in events {
        if completion_passthrough_edit_event(
            event,
            accept_suggestion_on_enter,
            accept_suggestion_on_tab,
        ) {
            edit_events.get_or_insert_with(Vec::new).push(event.clone());
        }
    }
    edit_events
}

fn completion_passthrough_edit_event(
    event: &Event,
    accept_suggestion_on_enter: bool,
    accept_suggestion_on_tab: bool,
) -> bool {
    match event {
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
    }
}

#[cfg(test)]
mod tests {
    use super::{
        completion_passthrough_edit_events, completion_passthrough_selected_item, tab_focus_event,
    };
    use eframe::egui::{Event, Key, Modifiers};
    use kuroya_core::LspCompletionItem;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn completion_passthrough_selection_clamps_and_snapshots_raw_item() {
        let mut selected = 8;
        let mut raw_item = completion_item("Raw\nHashMap\u{202e}");
        raw_item.detail = Some("raw detail".to_owned());
        raw_item.resolve_payload = Some(Arc::new(json!({
            "label": raw_item.label.clone(),
            "data": {
                "token": "raw-item"
            }
        })));
        let expected_item = raw_item.clone();
        let items = [completion_item("Vec"), raw_item];

        let (selected_index, selected_item) =
            completion_passthrough_selected_item(&mut selected, &items).expect("selected");
        let selected_item = selected_item.clone();

        assert_eq!(selected, 1);
        assert_eq!(selected_index, 1);
        assert_eq!(selected_item, expected_item);
        assert_eq!(items[1], expected_item);
    }

    #[test]
    fn completion_passthrough_selection_rejects_empty_items() {
        let mut selected = 8;
        let items = [];

        assert!(completion_passthrough_selected_item(&mut selected, &items).is_none());
        assert_eq!(selected, 0);
    }

    #[test]
    fn completion_passthrough_edit_events_respect_acceptance_keys() {
        let enter = key_event(Key::Enter, Modifiers::NONE);
        let tab = key_event(Key::Tab, Modifiers::NONE);

        assert_eq!(
            completion_passthrough_edit_events(std::slice::from_ref(&enter), true, false),
            None
        );
        assert_eq!(
            completion_passthrough_edit_events(std::slice::from_ref(&enter), false, false),
            Some(vec![enter])
        );
        assert_eq!(
            completion_passthrough_edit_events(std::slice::from_ref(&tab), false, true),
            None
        );
        assert_eq!(
            completion_passthrough_edit_events(std::slice::from_ref(&tab), false, false),
            Some(vec![tab])
        );
    }

    #[test]
    fn completion_passthrough_tab_focus_ignores_modified_tab() {
        assert!(tab_focus_event(&[key_event(Key::Tab, Modifiers::NONE)]));
        assert!(!tab_focus_event(&[key_event(Key::Tab, Modifiers::CTRL)]));
    }

    fn key_event(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn completion_item(label: &str) -> LspCompletionItem {
        LspCompletionItem {
            label: label.to_owned(),
            detail: None,
            documentation: None,
            kind: None,
            deprecated: false,
            is_snippet: false,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: label.to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: None,
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: None,
        }
    }
}
