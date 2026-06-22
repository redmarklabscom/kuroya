mod items;

use crate::{
    KuroyaApp,
    editor_input::editor_text_input_from_event,
    lsp_completion_resolve::CompletionPreviewResolveKey,
    path_display::display_path_label_cow,
    popup_buttons::{PopupButtonKind, popup_button},
    ui_state::{clamp_selection, handle_list_navigation_keys, selection_page_step},
};
use eframe::egui::{self, Align, Color32, Id, Key, RichText, Ui};
use kuroya_core::{BufferId, EditorSettings, EditorTabCompletion, LspCompletionItem};
use std::{fmt::Write as _, path::Path, sync::Arc};

const COMPLETION_POPUP_CONTENT_CACHE_ID: &str = "completion_popup_content_cache";
const MAX_COMPLETION_COMMIT_EVENT_SCAN: usize = 256;
const MAX_COMPLETION_COMMIT_CHARACTER_SCAN: usize = 64;

pub(super) enum CompletionPopupAction {
    Close,
    Apply {
        item: Box<LspCompletionItem>,
        commit_text: Option<String>,
    },
}

impl KuroyaApp {
    pub(super) fn render_completion_popup_content(
        &mut self,
        ui: &mut Ui,
    ) -> Option<CompletionPopupAction> {
        let mut close = self.render_completion_popup_header(ui);
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            close = true;
        }
        let item_count = self.completion_items.len();
        let selection_was_normalized =
            normalize_completion_selection(&mut self.completion_selected, item_count);
        let completion_row_height = items::completion_item_row_height(
            self.settings.suggest_line_height,
            ui.spacing().interact_size.y,
        );
        let navigation_page_step =
            selection_page_step(completion_row_height, ui.available_height());
        let navigation_changed = ui.input(|input| {
            handle_list_navigation_keys(
                input,
                &mut self.completion_selected,
                item_count,
                navigation_page_step,
            )
        });
        let selection_changed = selection_was_normalized || navigation_changed;
        let tab_accepts_for_key =
            completion_selected_item(&self.completion_items, self.completion_selected)
                .map(|(_, item)| completion_tab_accepts(item, &self.settings))
                .unwrap_or(self.settings.accept_suggestion_on_tab);
        let key_accepts = ui.input(|input| {
            (self.settings.accept_suggestion_on_enter && input.key_pressed(Key::Enter))
                || (tab_accepts_for_key && input.key_pressed(Key::Tab))
        });

        ui.separator();
        let clicked_apply = self.render_completion_items(ui, selection_changed);
        let item_count = self.completion_items.len();
        let (tab_accepts, should_request_preview_resolve) = {
            let selected_item =
                completion_selected_item(&self.completion_items, self.completion_selected);
            let tab_accepts = selected_item
                .map(|(_, item)| completion_tab_accepts(item, &self.settings))
                .unwrap_or(self.settings.accept_suggestion_on_tab);
            let should_request_preview_resolve = selected_item.is_some_and(|(selected, item)| {
                completion_item_needs_preview_resolve_display(item)
                    && !selected_completion_preview_resolve_is_tracked(self, selected, item)
            });
            (tab_accepts, should_request_preview_resolve)
        };
        if self.settings.suggest_show_status_bar {
            ui.separator();
            ui.horizontal(|ui| {
                let status_label = cached_completion_status_bar_label(
                    ui,
                    item_count,
                    self.completion_selected,
                    self.settings.accept_suggestion_on_enter,
                    tab_accepts,
                    ui.visuals().weak_text_color(),
                );
                ui.label(status_label);
            });
        }
        let apply_item = completion_apply_item_snapshot(
            key_accepts,
            clicked_apply,
            self.completion_selected,
            &self.completion_items,
        );
        if close {
            Some(CompletionPopupAction::Close)
        } else if let Some(item) = apply_item {
            Some(CompletionPopupAction::Apply {
                item: Box::new(item),
                commit_text: None,
            })
        } else {
            if should_request_preview_resolve {
                self.request_selected_completion_preview_resolve();
            }
            None
        }
    }

    fn render_completion_popup_header(&self, ui: &mut Ui) -> bool {
        let mut close = false;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(completion_target_label(self))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if popup_button(ui, "Close", PopupButtonKind::Secondary).clicked() {
                    close = true;
                }
            });
        });
        close
    }
}

pub(super) fn completion_selected_item(
    items: &[LspCompletionItem],
    selected: usize,
) -> Option<(usize, &LspCompletionItem)> {
    items.get(selected).map(|item| (selected, item))
}

fn completion_apply_index(
    key_accepts: bool,
    clicked_apply: Option<usize>,
    selected: usize,
    item_count: usize,
) -> Option<usize> {
    if let Some(clicked) = clicked_apply {
        return (clicked < item_count).then_some(clicked);
    }

    (key_accepts && selected < item_count).then_some(selected)
}

fn completion_apply_item_snapshot(
    key_accepts: bool,
    clicked_apply: Option<usize>,
    selected: usize,
    items: &[LspCompletionItem],
) -> Option<LspCompletionItem> {
    completion_apply_index(key_accepts, clicked_apply, selected, items.len())
        .and_then(|idx| completion_apply_item(items, idx))
}

fn completion_apply_item(items: &[LspCompletionItem], idx: usize) -> Option<LspCompletionItem> {
    items.get(idx).cloned()
}

pub(super) fn normalize_completion_selection(selected: &mut usize, item_count: usize) -> bool {
    let before = *selected;
    clamp_selection(selected, item_count);
    before != *selected
}

fn cached_completion_status_bar_label(
    ui: &mut Ui,
    count: usize,
    selected: usize,
    enter_accepts: bool,
    tab_accepts: bool,
    text_color: Color32,
) -> Arc<RichText> {
    ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<CompletionPopupContentCache>(Id::new(
            COMPLETION_POPUP_CONTENT_CACHE_ID,
        ))
        .status_bar
        .label(count, selected, enter_accepts, tab_accepts, text_color)
    })
}

#[derive(Clone, Default)]
struct CompletionPopupContentCache {
    status_bar: CompletionStatusBarLabelCache,
}

#[derive(Clone, Default)]
struct CompletionStatusBarLabelCache {
    key: Option<CompletionStatusBarLabelKey>,
    label: Option<Arc<RichText>>,
}

impl CompletionStatusBarLabelCache {
    fn label(
        &mut self,
        count: usize,
        selected: usize,
        enter_accepts: bool,
        tab_accepts: bool,
        text_color: Color32,
    ) -> Arc<RichText> {
        let key = CompletionStatusBarLabelKey {
            count,
            selected,
            enter_accepts,
            tab_accepts,
            text_color: color_cache_key(text_color),
        };
        if self.key == Some(key)
            && let Some(label) = &self.label
        {
            return Arc::clone(label);
        }

        let label = Arc::new(
            RichText::new(completion_status_bar_text(
                count,
                selected,
                enter_accepts,
                tab_accepts,
            ))
            .small()
            .color(text_color),
        );
        self.key = Some(key);
        self.label = Some(Arc::clone(&label));
        label
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionStatusBarLabelKey {
    count: usize,
    selected: usize,
    enter_accepts: bool,
    tab_accepts: bool,
    text_color: [u8; 4],
}

fn color_cache_key(color: Color32) -> [u8; 4] {
    [color.r(), color.g(), color.b(), color.a()]
}

fn completion_item_needs_preview_resolve_display(item: &LspCompletionItem) -> bool {
    item.needs_resolve()
        && (item
            .documentation
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
            || item.detail.as_deref().is_none_or(str::is_empty))
}

fn selected_completion_preview_resolve_is_tracked(
    app: &KuroyaApp,
    selected: usize,
    item: &LspCompletionItem,
) -> bool {
    app.completion_preview_resolve_in_flight
        .iter()
        .chain(app.completion_preview_resolve_recent_attempts.iter())
        .any(|key| {
            completion_preview_resolve_key_matches_selected_item(
                key,
                app.completion_buffer_id,
                app.completion_path.as_deref(),
                app.completion_version,
                app.completion_line,
                app.completion_column,
                selected,
                item,
            )
        })
}

fn completion_preview_resolve_key_matches_selected_item(
    key: &CompletionPreviewResolveKey,
    id: Option<BufferId>,
    path: Option<&Path>,
    version: Option<u64>,
    line: usize,
    column: usize,
    selected: usize,
    item: &LspCompletionItem,
) -> bool {
    id == Some(key.id)
        && path.is_some_and(|path| key.path.as_path() == path)
        && Some(key.version) == version
        && key.line.checked_add(1) == Some(line)
        && key.character.checked_add(1) == Some(column)
        && key.selected == selected
        && key.item.as_ref() == item
}

pub(crate) fn completion_tab_accepts(item: &LspCompletionItem, settings: &EditorSettings) -> bool {
    settings.accept_suggestion_on_tab
        || match settings.tab_completion {
            EditorTabCompletion::On => true,
            EditorTabCompletion::Off => false,
            EditorTabCompletion::OnlySnippets => item.is_snippet || item.kind == Some(15),
        }
}

pub(crate) fn completion_commit_text(
    item: &LspCompletionItem,
    settings: &EditorSettings,
    events: &[egui::Event],
) -> Option<String> {
    if !settings.accept_suggestion_on_commit_character || item.commit_characters.is_empty() {
        return None;
    }
    events
        .iter()
        .take(MAX_COMPLETION_COMMIT_EVENT_SCAN)
        .find_map(|event| {
            let text = editor_text_input_from_event(event)?;
            let mut chars = text.chars();
            if chars.next().is_some()
                && chars.next().is_none()
                && completion_item_has_commit_character(item, text)
            {
                Some(text.to_owned())
            } else {
                None
            }
        })
}

fn completion_item_has_commit_character(item: &LspCompletionItem, text: &str) -> bool {
    item.commit_characters
        .iter()
        .take(MAX_COMPLETION_COMMIT_CHARACTER_SCAN)
        .any(|ch| ch == text)
}

fn completion_target_label(app: &KuroyaApp) -> String {
    app.completion_path
        .as_ref()
        .map(|path| completion_target_path_label(path, app.completion_line, app.completion_column))
        .unwrap_or_else(|| "No target".to_owned())
}

fn completion_target_path_label(path: &Path, line: usize, column: usize) -> String {
    let path = display_path_label_cow(path);
    let mut label = String::with_capacity(path.len() + 24);
    label.push_str(&path);
    let _ = write!(label, ":{line}:{column}");
    label
}

fn completion_status_bar_text(
    count: usize,
    selected: usize,
    enter_accepts: bool,
    tab_accepts: bool,
) -> String {
    let accept = match (enter_accepts, tab_accepts) {
        (true, true) => "Enter/Tab to accept",
        (true, false) => "Enter to accept",
        (false, true) => "Tab to accept",
        (false, false) => "Click to accept",
    };

    let mut text = String::with_capacity(24 + accept.len());
    if count == 0 {
        text.push_str("0 of 0");
    } else {
        let _ = write!(text, "{} of {count}", selected.min(count - 1) + 1);
    }
    text.push_str(" | ");
    text.push_str(accept);
    text
}

#[cfg(test)]
mod tests {
    use super::{
        CompletionStatusBarLabelCache, MAX_COMPLETION_COMMIT_CHARACTER_SCAN,
        MAX_COMPLETION_COMMIT_EVENT_SCAN, completion_apply_index, completion_apply_item,
        completion_apply_item_snapshot, completion_commit_text,
        completion_item_has_commit_character, completion_item_needs_preview_resolve_display,
        completion_preview_resolve_key_matches_selected_item, completion_selected_item,
        completion_status_bar_text, completion_tab_accepts, completion_target_path_label,
        items::completion_item_label, normalize_completion_selection,
    };
    use crate::lsp_completion_resolve::CompletionPreviewResolveKey;
    use crate::path_display::DISPLAY_PATH_LABEL_MAX_CHARS;
    use eframe::egui::{Color32, Event, ImeEvent};
    use kuroya_core::{EditorSettings, EditorTabCompletion, LspCompletionItem};
    use std::{path::PathBuf, sync::Arc};

    fn item(is_snippet: bool) -> LspCompletionItem {
        LspCompletionItem {
            label: "println!".to_owned(),
            detail: None,
            documentation: None,
            kind: None,
            deprecated: false,
            is_snippet,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: vec![".".to_owned()],
            insert_text: "println!".to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: None,
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: None,
        }
    }

    #[test]
    fn completion_status_bar_text_tracks_selection_and_accept_keys() {
        assert_eq!(
            completion_status_bar_text(3, 1, true, true),
            "2 of 3 | Enter/Tab to accept"
        );
        assert_eq!(
            completion_status_bar_text(0, 4, false, true),
            "0 of 0 | Tab to accept"
        );
    }

    #[test]
    fn completion_status_bar_label_cache_reuses_unchanged_status_display() {
        let mut cache = CompletionStatusBarLabelCache::default();

        let first = cache.label(3, 1, true, true, Color32::WHITE);
        let second = cache.label(3, 1, true, true, Color32::WHITE);
        let changed_selection = cache.label(3, 2, true, true, Color32::WHITE);

        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &changed_selection));
    }

    #[test]
    fn completion_target_path_label_sanitizes_and_bounds_path_text() {
        let path = PathBuf::from("workspace").join(format!(
            "bad\n{}\u{202e}.rs",
            "target-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));

        let label = completion_target_path_label(&path, 12, 4);

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.ends_with(":12:4"));
        assert!(
            label.trim_end_matches(":12:4").chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS,
            "completion target path should be bounded: {label:?}"
        );
    }

    #[test]
    fn completion_selection_normalization_clamps_stale_indexes() {
        let mut selected = 12;
        assert!(normalize_completion_selection(&mut selected, 3));
        assert_eq!(selected, 2);

        assert!(!normalize_completion_selection(&mut selected, 3));
        assert_eq!(selected, 2);

        assert!(normalize_completion_selection(&mut selected, 0));
        assert_eq!(selected, 0);
    }

    #[test]
    fn completion_selected_item_rejects_stale_indexes_and_preserves_raw_item() {
        let mut raw_item = item(false);
        raw_item.label = "Raw\nHashMap\u{202e}".to_owned();
        raw_item.detail = Some("raw detail".to_owned());
        raw_item.resolve_payload = Some(Arc::new(serde_json::json!({
            "label": raw_item.label.clone(),
            "data": {
                "token": "raw-item"
            }
        })));
        let expected_item = raw_item.clone();
        let items = vec![item(false), raw_item];

        let (selected, selected_item) = completion_selected_item(&items, 1).expect("selected item");

        assert_eq!(selected, 1);
        assert_eq!(selected_item, &expected_item);
        assert_eq!(items[1], expected_item);
        assert!(completion_selected_item(&items, 4).is_none());
    }

    #[test]
    fn completion_apply_index_prefers_clicked_row_and_rejects_stale_indexes() {
        assert_eq!(completion_apply_index(true, Some(2), 0, 4), Some(2));
        assert_eq!(completion_apply_index(true, None, 2, 4), Some(2));
        assert_eq!(completion_apply_index(false, Some(2), 0, 4), Some(2));
        assert_eq!(completion_apply_index(true, Some(8), 1, 4), None);
        assert_eq!(completion_apply_index(true, None, 8, 4), None);
        assert_eq!(completion_apply_index(false, None, 1, 4), None);
    }

    #[test]
    fn completion_apply_item_snapshot_uses_current_items_for_click_and_key_paths() {
        let mut clicked_item = item(false);
        clicked_item.label = "Clicked".to_owned();
        let mut selected_item = item(false);
        selected_item.label = "Selected".to_owned();
        let items = vec![clicked_item.clone(), selected_item.clone()];

        assert_eq!(
            completion_apply_item_snapshot(true, Some(0), 1, &items),
            Some(clicked_item)
        );
        assert_eq!(
            completion_apply_item_snapshot(true, None, 1, &items),
            Some(selected_item)
        );
        assert_eq!(
            completion_apply_item_snapshot(true, Some(4), 1, &items),
            None
        );
        assert_eq!(completion_apply_item_snapshot(true, None, 4, &items), None);
        assert_eq!(completion_apply_item_snapshot(false, None, 1, &items), None);
    }

    #[test]
    fn completion_apply_item_snapshots_without_removing_selected_item() {
        let mut raw_item = item(false);
        raw_item.label = "Raw\nHashMap\u{202e}".to_owned();
        raw_item.detail = Some("raw detail".to_owned());
        raw_item.resolve_payload = Some(Arc::new(serde_json::json!({
            "label": raw_item.label.clone(),
            "data": {
                "token": "raw-item"
            }
        })));
        let expected_item = raw_item.clone();
        let items = vec![item(false), raw_item];

        let selected_item = completion_apply_item(&items, 1).expect("selected item");

        assert_eq!(selected_item, expected_item);
        assert_eq!(items.len(), 2);
        assert_eq!(items[1], expected_item);
        assert_eq!(completion_apply_item(&items, 3), None);
    }

    #[test]
    fn completion_preview_resolve_display_check_only_needs_missing_doc_or_detail() {
        let mut completion = item(false);
        assert!(!completion_item_needs_preview_resolve_display(&completion));

        completion.resolve_payload = Some(Arc::new(serde_json::Value::Null));
        assert!(completion_item_needs_preview_resolve_display(&completion));

        completion.documentation = Some("Docs".to_owned());
        assert!(completion_item_needs_preview_resolve_display(&completion));

        completion.detail = Some("macro".to_owned());
        assert!(!completion_item_needs_preview_resolve_display(&completion));
    }

    #[test]
    fn completion_preview_resolve_tracking_matches_exact_raw_selected_item() {
        let path = PathBuf::from("workspace/src/main.rs");
        let completion = item(false);
        let key = CompletionPreviewResolveKey {
            id: 7,
            path: path.clone(),
            version: 3,
            line: 4,
            character: 8,
            selected: 2,
            item: Box::new(completion.clone()),
        };

        assert!(completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(3),
            5,
            9,
            2,
            &completion,
        ));

        let mut changed_raw_item = completion.clone();
        changed_raw_item.insert_text = "eprintln!".to_owned();
        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(3),
            5,
            9,
            2,
            &changed_raw_item,
        ));
        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(3),
            5,
            9,
            1,
            &completion,
        ));
    }

    #[test]
    fn completion_preview_resolve_tracking_uses_current_selected_item_pair() {
        let path = PathBuf::from("workspace/src/main.rs");
        let first = item(false);
        let mut second = item(false);
        second.label = "VecDeque".to_owned();
        second.insert_text = "VecDeque".to_owned();
        let items = vec![first.clone(), second.clone()];
        let key = CompletionPreviewResolveKey {
            id: 7,
            path: path.clone(),
            version: 3,
            line: 4,
            character: 8,
            selected: 0,
            item: Box::new(first.clone()),
        };
        let (selected, selected_item) =
            completion_selected_item(&items, 1).expect("current selected item");

        assert!(completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(3),
            5,
            9,
            0,
            &first,
        ));
        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(3),
            5,
            9,
            selected,
            selected_item,
        ));
    }

    #[test]
    fn completion_preview_resolve_tracking_rejects_stale_origin_fields() {
        let path = PathBuf::from("workspace/src/main.rs");
        let other_path = PathBuf::from("workspace/src/lib.rs");
        let completion = item(false);
        let key = CompletionPreviewResolveKey {
            id: 7,
            path: path.clone(),
            version: 3,
            line: 4,
            character: 8,
            selected: 2,
            item: Box::new(completion.clone()),
        };

        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(8),
            Some(&path),
            Some(3),
            5,
            9,
            2,
            &completion,
        ));
        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&other_path),
            Some(3),
            5,
            9,
            2,
            &completion,
        ));
        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(4),
            5,
            9,
            2,
            &completion,
        ));
        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(3),
            4,
            9,
            2,
            &completion,
        ));
        assert!(!completion_preview_resolve_key_matches_selected_item(
            &key,
            Some(7),
            Some(&path),
            Some(3),
            5,
            8,
            2,
            &completion,
        ));
    }

    #[test]
    fn completion_tab_acceptance_follows_tab_completion_setting() {
        let mut settings = EditorSettings {
            accept_suggestion_on_tab: false,
            tab_completion: EditorTabCompletion::OnlySnippets,
            ..Default::default()
        };

        assert!(completion_tab_accepts(&item(true), &settings));
        assert!(!completion_tab_accepts(&item(false), &settings));

        settings.tab_completion = EditorTabCompletion::On;
        assert!(completion_tab_accepts(&item(false), &settings));
    }

    #[test]
    fn completion_commit_text_requires_setting_and_item_commit_character() {
        let mut settings = EditorSettings::default();
        let events = vec![Event::Text(".".to_owned())];

        assert_eq!(
            completion_commit_text(&item(false), &settings, &events),
            Some(".".to_owned())
        );
        assert_eq!(
            completion_commit_text(
                &item(false),
                &settings,
                &[Event::Ime(ImeEvent::Commit(".".to_owned()))]
            ),
            Some(".".to_owned())
        );
        assert_eq!(
            completion_commit_text(
                &item(false),
                &settings,
                &[Event::Ime(ImeEvent::Preedit(".".to_owned()))]
            ),
            None
        );

        settings.accept_suggestion_on_commit_character = false;
        assert_eq!(
            completion_commit_text(&item(false), &settings, &events),
            None
        );
    }

    #[test]
    fn completion_commit_text_scans_bounded_events_and_commit_characters() {
        let settings = EditorSettings::default();
        let mut completion = item(false);
        completion.commit_characters = (0..MAX_COMPLETION_COMMIT_CHARACTER_SCAN)
            .map(|idx| format!("{idx}"))
            .chain(std::iter::once(".".to_owned()))
            .collect();

        assert!(!completion_item_has_commit_character(&completion, "."));
        assert_eq!(
            completion_commit_text(&completion, &settings, &[Event::Text(".".to_owned())]),
            None
        );

        completion.commit_characters[0] = ".".to_owned();
        let mut events = (0..MAX_COMPLETION_COMMIT_EVENT_SCAN)
            .map(|_| Event::Text(",".to_owned()))
            .collect::<Vec<_>>();
        events.push(Event::Text(".".to_owned()));

        assert_eq!(
            completion_commit_text(&completion, &settings, &events),
            None
        );
        assert_eq!(
            completion_commit_text(&completion, &settings, &[Event::Text(".".to_owned())]),
            Some(".".to_owned())
        );
    }

    #[test]
    fn completion_row_display_is_bounded_without_rewriting_raw_item() {
        let mut completion = item(false);
        completion.label = format!("print\n{}\u{202e}tail", "unsafe-label-".repeat(80));
        completion.detail = Some(format!(
            "detail\t{}\u{2066}tail",
            "unsafe-detail-".repeat(80)
        ));
        completion.insert_text = "raw\ninsert\u{202e} text".to_owned();
        let raw_completion = completion.clone();

        let label = completion_item_label(&completion, true, true);

        assert!(!label.contains('\n'));
        assert!(!label.contains('\t'));
        assert!(!label.contains('\u{202e}'));
        assert!(!label.contains('\u{2066}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= 350, "{label}");
        assert_eq!(completion, raw_completion);
    }
}
