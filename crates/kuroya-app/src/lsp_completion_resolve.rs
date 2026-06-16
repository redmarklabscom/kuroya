use crate::{
    KuroyaApp,
    lsp_edits::completion_buffer_edit_plan,
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::{display_error_label_cow, display_path_label_cow, sanitized_display_label_cow},
    workspace_state::{
        active_buffer_lsp_position_matches, buffer_id_path_version_matches,
        lsp_event_path_is_current, paths_match_lexically,
    },
};
use kuroya_core::{BufferId, LspCompletionItem};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const MAX_COMPLETION_PREVIEW_RESOLVE_KEYS: usize = 32;
const MAX_COMPLETION_RESOLVE_STATUS_LABEL_CACHE_ENTRIES: usize = 32;
const COMPLETION_RESOLVE_STATUS_LABEL_MAX_CHARS: usize = 80;
const MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDITS: usize = 512;
const MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDIT_BYTES: usize = 2 * 1024 * 1024;
const MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDIT_TOTAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompletionResolveIntent {
    Apply { commit_text: Option<String> },
    Preview { selected: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionPreviewResolveKey {
    pub(crate) id: BufferId,
    pub(crate) path: PathBuf,
    pub(crate) version: u64,
    pub(crate) line: usize,
    pub(crate) character: usize,
    pub(crate) selected: usize,
    pub(crate) item: Box<LspCompletionItem>,
}

impl KuroyaApp {
    pub(crate) fn request_selected_completion_preview_resolve(&mut self) {
        if !self.completion_open {
            return;
        }
        let selected = self.completion_selected;
        if !self
            .completion_items
            .get(selected)
            .is_some_and(completion_item_needs_preview_resolve)
        {
            return;
        }
        let Some(origin_id) = self.completion_buffer_id else {
            return;
        };
        let Some(origin_path) = self.completion_path.as_ref() else {
            return;
        };
        let Some(origin_version) = self.completion_version else {
            return;
        };
        let Some((id, path, version, line, character)) = self.active_lsp_position() else {
            return;
        };
        let Some(origin_line) = line.checked_add(1) else {
            return;
        };
        let Some(origin_column) = character.checked_add(1) else {
            return;
        };
        if id != origin_id
            || path.as_path() != origin_path
            || version != origin_version
            || self.completion_line != origin_line
            || self.completion_column != origin_column
        {
            return;
        }

        let Some(item) = self.completion_items.get(selected).cloned() else {
            return;
        };
        let key = CompletionPreviewResolveKey {
            id,
            path: origin_path.clone(),
            version: origin_version,
            line,
            character,
            selected,
            item: Box::new(item.clone()),
        };
        if self
            .completion_preview_resolve_in_flight
            .iter()
            .any(|candidate| candidate == &key)
            || self
                .completion_preview_resolve_recent_attempts
                .iter()
                .any(|candidate| candidate == &key)
        {
            return;
        }
        let Some(client) = self.ensure_lsp_for_buffer(origin_id) else {
            return;
        };
        if !client.resolve_completion_item(
            origin_id,
            key.path.clone(),
            origin_version,
            line,
            character,
            item,
            CompletionResolveIntent::Preview { selected },
        ) {
            self.remember_completion_preview_resolve_attempt(key);
            self.status = lsp_command_queue_failed_status("completionItem/resolve");
            return;
        }

        self.record_lsp_client_trace(
            "completionItem/resolve",
            completion_preview_resolve_trace_label(&key.path, origin_line, origin_column),
        );
        self.track_completion_preview_resolve_in_flight(key.clone());
        self.remember_completion_preview_resolve_attempt(key);
    }

    pub(crate) fn handle_completion_preview_resolve_result(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        selected: usize,
        item: Option<LspCompletionItem>,
        fallback_item: LspCompletionItem,
        error: Option<String>,
    ) {
        let Some(character) = column.checked_sub(1) else {
            return;
        };
        let key = CompletionPreviewResolveKey {
            id,
            path,
            version,
            line,
            character,
            selected,
            item: Box::new(fallback_item),
        };
        self.completion_preview_resolve_in_flight
            .retain(|candidate| candidate != &key);

        let Some(origin_line) = line.checked_add(1) else {
            self.completion_preview_resolve_recent_attempts
                .retain(|candidate| candidate != &key);
            return;
        };
        if !self.completion_open
            || self.completion_buffer_id != Some(id)
            || self.completion_path.as_ref() != Some(&key.path)
            || self.completion_version != Some(version)
            || self.completion_line != origin_line
            || self.completion_column != column
            || self.completion_selected != selected
        {
            self.completion_preview_resolve_recent_attempts
                .retain(|candidate| candidate != &key);
            return;
        }
        if !lsp_event_path_is_current(&self.workspace.root, &key.path)
            || !buffer_id_path_version_matches(&self.buffers, id, &key.path, version)
            || !active_buffer_lsp_position_matches(
                self.active_buffer(),
                &key.path,
                version,
                line,
                column,
            )
        {
            self.completion_preview_resolve_recent_attempts
                .retain(|candidate| candidate != &key);
            self.clear_completion_popup_state();
            return;
        }

        let Some(current_item) = self.completion_items.get(selected) else {
            self.completion_preview_resolve_recent_attempts
                .retain(|candidate| candidate != &key);
            return;
        };
        if current_item != key.item.as_ref() {
            self.completion_preview_resolve_recent_attempts
                .retain(|candidate| candidate != &key);
            return;
        }
        if let Some(error) = error {
            self.status = completion_documentation_resolve_failed_status(&error);
            return;
        }
        let Some(mut item) = item else {
            return;
        };
        if !completion_resolve_preserves_apply_payload(&item, key.item.as_ref()) {
            self.status = ignored_completion_documentation_resolve_status(&key.item.label);
            return;
        }
        if !completion_resolve_additional_text_edits_are_safe(&item, &key.path)
            || self
                .active_buffer()
                .and_then(|buffer| {
                    completion_buffer_edit_plan(buffer, &item, self.settings.suggest_insert_mode)
                })
                .is_none()
        {
            self.status = ignored_completion_documentation_resolve_status(&key.item.label);
            return;
        }
        item.resolve_payload = None;
        let status = resolved_completion_documentation_status(&item.label);
        self.remember_completion_preview_resolve_attempt(key);
        self.completion_items[selected] = item;
        self.status = status;
    }

    fn track_completion_preview_resolve_in_flight(&mut self, key: CompletionPreviewResolveKey) {
        self.completion_preview_resolve_in_flight
            .retain(|candidate| candidate != &key);
        self.completion_preview_resolve_in_flight.push(key);
        trim_completion_preview_resolve_keys(&mut self.completion_preview_resolve_in_flight);
    }

    fn remember_completion_preview_resolve_attempt(&mut self, key: CompletionPreviewResolveKey) {
        self.completion_preview_resolve_recent_attempts
            .retain(|candidate| candidate != &key);
        self.completion_preview_resolve_recent_attempts.push(key);
        trim_completion_preview_resolve_keys(&mut self.completion_preview_resolve_recent_attempts);
    }
}

fn completion_item_needs_preview_resolve(item: &LspCompletionItem) -> bool {
    item.needs_resolve()
        && (item
            .documentation
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
            || item.detail.as_deref().is_none_or(str::is_empty))
}

pub(crate) fn completion_resolve_preserves_apply_payload(
    item: &LspCompletionItem,
    fallback_item: &LspCompletionItem,
) -> bool {
    item.label == fallback_item.label
        && item.kind == fallback_item.kind
        && item.deprecated == fallback_item.deprecated
        && item.is_snippet == fallback_item.is_snippet
        && item.sort_text == fallback_item.sort_text
        && item.filter_text == fallback_item.filter_text
        && item.preselect == fallback_item.preselect
        && item.commit_characters == fallback_item.commit_characters
        && item.insert_text == fallback_item.insert_text
        && item.snippet_selection == fallback_item.snippet_selection
        && item.snippet_tabstops == fallback_item.snippet_tabstops
        && item.snippet_tabstop_groups == fallback_item.snippet_tabstop_groups
        && item.text_edit == fallback_item.text_edit
        && item.insert_text_edit == fallback_item.insert_text_edit
}

fn completion_resolve_additional_text_edits_are_safe(
    item: &LspCompletionItem,
    origin_path: &Path,
) -> bool {
    if item.additional_text_edits.len() > MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDITS {
        return false;
    }

    let mut total_new_text_bytes = 0usize;
    item.additional_text_edits.iter().all(|edit| {
        let Some(total) = total_new_text_bytes.checked_add(edit.new_text.len()) else {
            return false;
        };
        total_new_text_bytes = total;
        edit.new_text.len() <= MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDIT_BYTES
            && total_new_text_bytes <= MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDIT_TOTAL_BYTES
            && paths_match_lexically(&edit.path, origin_path)
    })
}

fn completion_documentation_resolve_failed_status(error: &str) -> String {
    format!(
        "Completion documentation resolve failed: {}",
        display_error_label_cow(error)
    )
}

fn ignored_completion_documentation_resolve_status(label: &str) -> String {
    let label = completion_resolve_status_label_cow(label);
    format!(
        "Ignored completion documentation resolve for `{}`: item changed",
        label.as_ref()
    )
}

fn resolved_completion_documentation_status(label: &str) -> String {
    let label = completion_resolve_status_label_cow(label);
    format!("Resolved completion documentation for `{}`", label.as_ref())
}

#[cfg(test)]
fn completion_resolve_status_label(label: &str) -> String {
    completion_resolve_status_label_cow(label).into_owned()
}

fn completion_resolve_status_label_cow(label: &str) -> Cow<'_, str> {
    static STATUS_LABEL_CACHE: OnceLock<Mutex<CompletionResolveStatusLabelCache>> = OnceLock::new();

    if let Ok(mut cache) = STATUS_LABEL_CACHE
        .get_or_init(|| Mutex::new(CompletionResolveStatusLabelCache::default()))
        .lock()
    {
        return cache.label_cow(label);
    }

    completion_resolve_status_label_uncached_cow(label)
}

#[cfg(test)]
fn completion_resolve_status_label_uncached(label: &str) -> String {
    completion_resolve_status_label_uncached_cow(label).into_owned()
}

fn completion_resolve_status_label_uncached_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        label,
        COMPLETION_RESOLVE_STATUS_LABEL_MAX_CHARS,
        "completion",
    )
}

#[derive(Debug, Default)]
struct CompletionResolveStatusLabelCache {
    entries: Vec<CompletionResolveStatusLabelCacheEntry>,
    next_replace_index: usize,
}

#[derive(Debug)]
struct CompletionResolveStatusLabelCacheEntry {
    raw_label: String,
    status_label: String,
}

impl CompletionResolveStatusLabelCache {
    fn label_cow<'a>(&mut self, raw_label: &'a str) -> Cow<'a, str> {
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.raw_label == raw_label)
        {
            return Cow::Owned(entry.status_label.clone());
        }

        let status_label = match completion_resolve_status_label_uncached_cow(raw_label) {
            Cow::Borrowed(label) => return Cow::Borrowed(label),
            Cow::Owned(label) => label,
        };
        let entry = CompletionResolveStatusLabelCacheEntry {
            raw_label: raw_label.to_owned(),
            status_label: status_label.clone(),
        };
        if self.entries.len() < MAX_COMPLETION_RESOLVE_STATUS_LABEL_CACHE_ENTRIES {
            self.entries.push(entry);
        } else {
            let index = self.next_replace_index;
            self.entries[index] = entry;
            self.next_replace_index =
                (self.next_replace_index + 1) % MAX_COMPLETION_RESOLVE_STATUS_LABEL_CACHE_ENTRIES;
        }
        Cow::Owned(status_label)
    }

    #[cfg(test)]
    fn label(&mut self, raw_label: &str) -> String {
        self.label_cow(raw_label).into_owned()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn is_cached(&self, raw_label: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.raw_label == raw_label)
    }
}

fn completion_preview_resolve_trace_label(path: &Path, line: usize, column: usize) -> String {
    format!("{}:{line}:{column} docs", display_path_label_cow(path))
}

fn trim_completion_preview_resolve_keys(keys: &mut Vec<CompletionPreviewResolveKey>) {
    let overflow = keys
        .len()
        .saturating_sub(MAX_COMPLETION_PREVIEW_RESOLVE_KEYS);
    if overflow > 0 {
        keys.drain(0..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspTextEdit, TextBuffer, Workspace};
    use serde_json::json;
    use std::{borrow::Cow, path::PathBuf, sync::Arc, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn preview_resolve_updates_selected_completion_without_applying() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved_completion(&path)),
            fallback_completion(&path),
            None,
        );

        assert_eq!(
            app.completion_items[0].documentation.as_deref(),
            Some("Resolved docs")
        );
        assert!(!app.completion_items[0].needs_resolve());
        assert_eq!(
            app.buffer(7).expect("buffer").text(),
            "fn main() {\n    Hash\n}\n"
        );
        assert!(app.completion_open);
    }

    #[test]
    fn preview_resolve_accepts_documentation_and_additional_edits_only() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        let mut resolved = resolved_completion(&path);
        resolved.additional_text_edits = vec![LspTextEdit {
            path: path.clone(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "use std::collections::HashMap;\n".to_owned(),
        }];

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved),
            fallback_completion(&path),
            None,
        );

        let item = &app.completion_items[0];
        assert_eq!(item.documentation.as_deref(), Some("Resolved docs"));
        assert_eq!(item.detail.as_deref(), Some("struct HashMap"));
        assert_eq!(item.insert_text, "HashMap");
        assert_eq!(item.additional_text_edits.len(), 1);
        assert!(!item.needs_resolve());
    }

    #[test]
    fn preview_resolve_ignores_additional_edits_for_other_paths() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other_path = root.join("src/other.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        let mut resolved = resolved_completion(&path);
        resolved.additional_text_edits = vec![LspTextEdit {
            path: other_path,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            new_text: "use std::collections::HashMap;\n".to_owned(),
        }];

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved),
            fallback_completion(&path),
            None,
        );

        assert_eq!(app.completion_items[0].documentation, None);
        assert!(app.completion_items[0].additional_text_edits.is_empty());
        assert_eq!(
            app.status,
            "Ignored completion documentation resolve for `HashMap`: item changed"
        );
    }

    #[test]
    fn preview_resolve_ignores_too_many_additional_edits() {
        let path = PathBuf::from("workspace/src/main.rs");
        let mut resolved = resolved_completion(&path);
        resolved.additional_text_edits = (0..=MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDITS)
            .map(|_| LspTextEdit {
                path: path.clone(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: String::new(),
            })
            .collect();

        assert!(!completion_resolve_additional_text_edits_are_safe(
            &resolved, &path
        ));
    }

    #[test]
    fn preview_resolve_ignores_aggregate_oversized_additional_edits() {
        let path = PathBuf::from("workspace/src/main.rs");
        let mut resolved = resolved_completion(&path);
        let chunk = "x".repeat((MAX_COMPLETION_RESOLVE_ADDITIONAL_TEXT_EDIT_TOTAL_BYTES / 2) + 1);
        resolved.additional_text_edits = vec![
            LspTextEdit {
                path: path.clone(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: chunk.clone(),
            },
            LspTextEdit {
                path: path.clone(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                new_text: chunk,
            },
        ];

        assert!(!completion_resolve_additional_text_edits_are_safe(
            &resolved, &path
        ));
    }

    #[test]
    fn preview_resolve_ignores_overlapping_resolved_additional_edits() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        let mut resolved = resolved_completion(&path);
        resolved.additional_text_edits = vec![LspTextEdit {
            path: path.clone(),
            start_line: 2,
            start_column: 5,
            end_line: 2,
            end_column: 7,
            new_text: "overlap".to_owned(),
        }];

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved),
            fallback_completion(&path),
            None,
        );

        assert_eq!(app.completion_items[0].documentation, None);
        assert!(app.completion_items[0].additional_text_edits.is_empty());
        assert_eq!(
            app.status,
            "Ignored completion documentation resolve for `HashMap`: item changed"
        );
    }

    #[test]
    fn preview_resolve_ignores_items_that_change_apply_payload() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        let mut resolved = resolved_completion(&path);
        resolved.insert_text = "HashSet".to_owned();
        resolved.documentation = Some("Wrong docs".to_owned());

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved),
            fallback_completion(&path),
            None,
        );

        assert_eq!(app.completion_items[0].documentation, None);
        assert_eq!(app.completion_items[0].insert_text, "HashMap");
        assert_eq!(
            app.status,
            "Ignored completion documentation resolve for `HashMap`: item changed"
        );
    }

    #[test]
    fn preview_resolve_failure_status_sanitizes_and_bounds_error() {
        let error = format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        );

        let status = completion_documentation_resolve_failed_status(&error);

        assert_safe_status_text(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Completion documentation resolve failed: ".chars().count()
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn preview_resolve_statuses_sanitize_labels_without_mutating_items() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        let raw_label = unsafe_completion_label();
        let raw_documentation = "Resolved docs\nwith raw \u{202e}direction".to_owned();
        app.completion_items[0].label = raw_label.clone();
        app.completion_items[0].insert_text = raw_label.clone();
        let mut resolved = app.completion_items[0].clone();
        resolved.documentation = Some(raw_documentation.clone());
        resolved.detail = Some("detail".to_owned());
        resolved.resolve_payload = Some(Arc::new(json!({ "data": { "id": 7 } })));

        app.handle_completion_preview_resolve_result(
            7,
            path,
            version,
            1,
            9,
            0,
            Some(resolved),
            app.completion_items[0].clone(),
            None,
        );

        assert_safe_status_text(&app.status);
        assert!(app.status.contains("..."));
        assert_eq!(app.completion_items[0].label, raw_label);
        assert_eq!(app.completion_items[0].insert_text, raw_label);
        assert_eq!(
            app.completion_items[0].documentation.as_deref(),
            Some(raw_documentation.as_str())
        );
        assert!(
            app.status.chars().count()
                <= "Resolved completion documentation for ``".chars().count()
                    + COMPLETION_RESOLVE_STATUS_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn completion_resolve_status_label_cow_borrows_clean_ascii_and_unicode() {
        assert_eq!(completion_resolve_status_label("HashMap"), "HashMap");
        assert!(matches!(
            completion_resolve_status_label_cow("HashMap"),
            Cow::Borrowed("HashMap")
        ));

        let unicode = "\u{03bb}Completion";
        assert_eq!(completion_resolve_status_label(unicode), unicode);
        match completion_resolve_status_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn completion_resolve_status_label_cow_owns_dirty_truncated_and_fallback_labels() {
        let cases = [
            "Hash\nMap".to_owned(),
            "  HashMap  ".to_owned(),
            "completion-label-".repeat(COMPLETION_RESOLVE_STATUS_LABEL_MAX_CHARS),
            "\u{200b}\u{202e}".to_owned(),
        ];

        for raw_label in cases {
            let label = completion_resolve_status_label_cow(&raw_label);

            assert_eq!(
                label.as_ref(),
                completion_resolve_status_label_uncached(&raw_label)
            );
            assert!(
                matches!(&label, Cow::Owned(_)),
                "expected owned label for {raw_label:?}"
            );
            assert!(label.chars().count() <= COMPLETION_RESOLVE_STATUS_LABEL_MAX_CHARS);
        }
    }

    #[test]
    fn completion_resolve_status_label_cache_borrows_clean_labels_without_entries() {
        let mut cache = CompletionResolveStatusLabelCache::default();

        assert!(matches!(
            cache.label_cow("HashMap"),
            Cow::Borrowed("HashMap")
        ));
        assert!(matches!(
            cache.label_cow("\u{03bb}Completion"),
            Cow::Borrowed("\u{03bb}Completion")
        ));

        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn completion_resolve_status_label_cache_reuses_unsafe_labels_without_mutating_raw_text() {
        let raw_label = unsafe_completion_label();
        let mut cache = CompletionResolveStatusLabelCache::default();

        let first = cache.label(&raw_label);
        let same_raw_label = raw_label.clone();
        let second = cache.label(&same_raw_label);

        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
        assert!(cache.is_cached(&raw_label));
        assert_eq!(same_raw_label, raw_label);
        assert_safe_status_text(&first);
        assert!(first.contains("..."));
        assert!(raw_label.contains('\n'));
        assert!(raw_label.contains('\u{202e}'));
    }

    #[test]
    fn completion_resolve_status_label_cache_bounds_cached_entries() {
        let mut cache = CompletionResolveStatusLabelCache::default();
        let labels = (0..=MAX_COMPLETION_RESOLVE_STATUS_LABEL_CACHE_ENTRIES)
            .map(|idx| format!("completion-{idx}\n{}", "x".repeat(128)))
            .collect::<Vec<_>>();

        for label in labels
            .iter()
            .take(MAX_COMPLETION_RESOLVE_STATUS_LABEL_CACHE_ENTRIES)
        {
            assert_eq!(
                cache.label(label),
                completion_resolve_status_label_uncached(label)
            );
            assert!(cache.is_cached(label));
        }
        assert_eq!(
            cache.len(),
            MAX_COMPLETION_RESOLVE_STATUS_LABEL_CACHE_ENTRIES
        );

        let overflow_label = labels.last().expect("overflow label");
        assert_eq!(
            cache.label(overflow_label),
            completion_resolve_status_label_uncached(overflow_label)
        );
        assert_eq!(
            cache.len(),
            MAX_COMPLETION_RESOLVE_STATUS_LABEL_CACHE_ENTRIES
        );
        assert!(cache.is_cached(overflow_label));
        assert!(!cache.is_cached(labels.first().expect("first cached label")));
    }

    #[test]
    fn preview_resolve_ignored_status_sanitizes_fallback_label() {
        let raw_label = unsafe_completion_label();

        let status = ignored_completion_documentation_resolve_status(&raw_label);

        assert_safe_status_text(&status);
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Ignored completion documentation resolve for ``: item changed"
                    .chars()
                    .count()
                    + COMPLETION_RESOLVE_STATUS_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn preview_resolve_trace_label_sanitizes_and_bounds_path() {
        let path = PathBuf::from("workspace/src").join(format!(
            "bad\n{}\u{202e}completion.rs",
            "very-long-path-".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));

        let trace = completion_preview_resolve_trace_label(&path, 2, 9);

        assert_safe_status_text(&trace);
        assert!(trace.contains("..."), "{trace}");
        assert!(
            trace.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS + ":2:9 docs".chars().count()
        );
    }

    #[test]
    fn preview_resolve_ignores_changed_selection() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        let stale_key = preview_key(&path, version, 0, fallback_completion(&path));
        app.completion_preview_resolve_recent_attempts
            .push(stale_key.clone());
        app.completion_items.push({
            let mut item = fallback_completion(&path);
            item.label = "Hasher".to_owned();
            item
        });
        app.completion_selected = 1;
        app.status = "before".to_owned();

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved_completion(&path)),
            fallback_completion(&path),
            None,
        );

        assert_eq!(app.completion_items[0].documentation, None);
        assert_eq!(app.status, "before");
        assert!(
            !app.completion_preview_resolve_recent_attempts
                .contains(&stale_key)
        );
    }

    #[test]
    fn preview_resolve_ignores_overflowing_origin_line() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        app.status = "before".to_owned();

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            usize::MAX,
            9,
            0,
            Some(resolved_completion(&path)),
            fallback_completion(&path),
            None,
        );

        assert_eq!(app.completion_items[0].documentation, None);
        assert_eq!(app.status, "before");
        assert!(app.completion_open);
    }

    #[test]
    fn preview_resolve_clears_popup_when_origin_cursor_is_stale() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        app.status = "before".to_owned();
        app.buffer_mut(7).expect("buffer").set_single_cursor(0);

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved_completion(&path)),
            fallback_completion(&path),
            None,
        );

        assert!(!app.completion_open);
        assert!(app.completion_items.is_empty());
        assert_eq!(app.completion_buffer_id, None);
        assert_eq!(app.completion_path, None);
        assert_eq!(app.completion_version, None);
        assert!(app.completion_preview_resolve_in_flight.is_empty());
        assert!(app.completion_preview_resolve_recent_attempts.is_empty());
        assert_eq!(app.status, "before");
    }

    #[test]
    fn preview_resolve_success_keeps_original_recent_attempt_without_resolved_duplicate() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_completion(root.clone(), path.clone());
        let version = app.buffer(7).expect("buffer").version();
        let fallback = fallback_completion(&path);
        let unresolved_key = preview_key(&path, version, 0, fallback.clone());
        app.completion_preview_resolve_in_flight
            .push(unresolved_key.clone());
        app.completion_preview_resolve_recent_attempts
            .push(unresolved_key.clone());

        app.handle_completion_preview_resolve_result(
            7,
            path.clone(),
            version,
            1,
            9,
            0,
            Some(resolved_completion(&path)),
            fallback,
            None,
        );

        let mut resolved_key_item = resolved_completion(&path);
        resolved_key_item.resolve_payload = None;
        let resolved_key = preview_key(&path, version, 0, resolved_key_item);
        assert!(app.completion_preview_resolve_in_flight.is_empty());
        assert_eq!(
            app.completion_preview_resolve_recent_attempts,
            vec![unresolved_key]
        );
        assert!(
            app.completion_preview_resolve_recent_attempts[0]
                .item
                .needs_resolve()
        );
        assert!(
            !app.completion_preview_resolve_recent_attempts
                .contains(&resolved_key)
        );
    }

    #[test]
    fn preview_resolve_tracks_multiple_in_flight_keys_without_duplicates() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        let first = preview_key(&path, 3, 0, fallback_completion(&path));
        let mut second_item = fallback_completion(&path);
        second_item.label = "Hasher".to_owned();
        let second = preview_key(&path, 3, 1, second_item);

        app.track_completion_preview_resolve_in_flight(first.clone());
        app.remember_completion_preview_resolve_attempt(first.clone());
        app.track_completion_preview_resolve_in_flight(second.clone());
        app.remember_completion_preview_resolve_attempt(second.clone());
        app.track_completion_preview_resolve_in_flight(first.clone());
        app.remember_completion_preview_resolve_attempt(first.clone());

        assert_eq!(app.completion_preview_resolve_in_flight.len(), 2);
        assert!(app.completion_preview_resolve_in_flight.contains(&first));
        assert!(app.completion_preview_resolve_in_flight.contains(&second));
        assert_eq!(app.completion_preview_resolve_recent_attempts.len(), 2);
        assert!(
            app.completion_preview_resolve_recent_attempts
                .contains(&first)
        );
        assert!(
            app.completion_preview_resolve_recent_attempts
                .contains(&second)
        );
    }

    fn app_with_completion(root: PathBuf, path: PathBuf) -> KuroyaApp {
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(
            7,
            Some(path.clone()),
            "fn main() {\n    Hash\n}\n".to_owned(),
        );
        buffer.set_single_cursor(buffer.line_column_to_char(1, 8));
        let version = buffer.version();
        app.active = Some(7);
        app.buffers.push(buffer);
        app.completion_open = true;
        app.completion_items = vec![fallback_completion(&path)];
        app.completion_buffer_id = Some(7);
        app.completion_path = Some(path);
        app.completion_version = Some(version);
        app.completion_line = 2;
        app.completion_column = 9;
        app
    }

    fn resolved_completion(path: &std::path::Path) -> LspCompletionItem {
        let mut item = fallback_completion(path);
        item.documentation = Some("Resolved docs".to_owned());
        item.detail = Some("struct HashMap".to_owned());
        item
    }

    fn fallback_completion(path: &std::path::Path) -> LspCompletionItem {
        LspCompletionItem {
            label: "HashMap".to_owned(),
            detail: None,
            documentation: None,
            kind: Some(7),
            deprecated: false,
            is_snippet: false,
            sort_text: None,
            filter_text: None,
            preselect: false,
            commit_characters: Vec::new(),
            insert_text: "HashMap".to_owned(),
            snippet_selection: None,
            snippet_tabstops: Vec::new(),
            snippet_tabstop_groups: Vec::new(),
            text_edit: Some(LspTextEdit {
                path: path.to_path_buf(),
                start_line: 2,
                start_column: 5,
                end_line: 2,
                end_column: 9,
                new_text: "HashMap".to_owned(),
            }),
            insert_text_edit: None,
            additional_text_edits: Vec::new(),
            resolve_payload: Some(Arc::new(json!({ "data": { "id": 7 } }))),
        }
    }

    fn preview_key(
        path: &std::path::Path,
        version: u64,
        selected: usize,
        item: LspCompletionItem,
    ) -> CompletionPreviewResolveKey {
        CompletionPreviewResolveKey {
            id: 7,
            path: path.to_path_buf(),
            version,
            line: 1,
            character: 8,
            selected,
            item: Box::new(item),
        }
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        KuroyaApp::from_startup_context(AppStartupContext {
            runtime: Runtime::new().expect("test runtime"),
            tx,
            rx,
            workspace: Workspace::new(root.clone()),
            settings: settings.clone(),
            settings_panel_draft: settings,
            settings_editor_font_path: String::new(),
            settings_ui_font_path: String::new(),
            theme_picker_selected: 0,
            saved_session: None,
            terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
            watcher: None,
            recent_projects: Vec::new(),
            trusted_workspaces: vec![root],
            now: Instant::now(),
            startup_timings: Vec::new(),
        })
    }

    fn unsafe_completion_label() -> String {
        format!(
            "Hash\n{}\u{202e}Map",
            "unsafe-label-".repeat(COMPLETION_RESOLVE_STATUS_LABEL_MAX_CHARS)
        )
    }

    fn assert_safe_status_text(status: &str) {
        assert!(!status.contains('\n'), "{status:?}");
        assert!(!status.contains('\r'), "{status:?}");
        assert!(!status.contains('\u{202e}'), "{status:?}");
        assert!(!status.contains('\u{2066}'), "{status:?}");
    }
}
