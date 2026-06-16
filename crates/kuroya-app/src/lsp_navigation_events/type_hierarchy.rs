use crate::{
    KuroyaApp,
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::{display_error_label_cow, sanitized_display_label_cow},
    ui_state::clamp_selection,
    workspace_state::{
        active_buffer_path_version_matches, lsp_event_path_is_current, paths_match_lexically,
    },
};
use kuroya_core::{BufferId, LspTypeHierarchyItem};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

const TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS: usize = 120;
const TYPE_HIERARCHY_DETAIL_PATH_MAX_CHARS: usize = 160;
const TYPE_HIERARCHY_ITEM_DETAIL_MAX_CHARS: usize = 320;
const TYPE_HIERARCHY_KIND_MAX_CHARS: usize = 32;

impl KuroyaApp {
    pub(super) fn handle_lsp_type_hierarchy_prepared(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        items: Option<Vec<LspTypeHierarchyItem>>,
        error: Option<String>,
    ) {
        if !self.type_hierarchy_prepare_response_matches(id, &path, version, line, column) {
            return;
        }

        if let Some(error) = error {
            self.clear_type_hierarchy();
            self.status = type_hierarchy_failed_status(&error);
            return;
        }

        let Some(root) = items.and_then(|items| {
            items.into_iter().find(|item| {
                type_hierarchy_item_path_matches_request(&self.workspace.root, &path, &item.path)
                    && type_hierarchy_item_location_is_valid(item)
            })
        }) else {
            self.clear_type_hierarchy();
            self.status = format!("No type hierarchy at {}:{}", line + 1, column + 1);
            return;
        };

        self.type_hierarchy_open = true;
        self.type_hierarchy_root = Some(root.clone());
        self.type_hierarchy_supertypes.clear();
        self.type_hierarchy_subtypes.clear();
        self.type_hierarchy_selected = 0;
        self.type_hierarchy_path = Some(path.clone());
        self.type_hierarchy_line = line + 1;
        self.type_hierarchy_column = column + 1;
        self.status = loaded_type_hierarchy_status(&root);

        if let Some(client) = self.ensure_lsp_for_buffer(id) {
            let root_detail = type_hierarchy_item_detail(&root);
            if client.type_hierarchy_supertypes(id, path.clone(), version, root.clone()) {
                self.record_lsp_client_trace("typeHierarchy/supertypes", root_detail.clone());
            } else {
                self.status = lsp_command_queue_failed_status("typeHierarchy/supertypes");
            }
            if client.type_hierarchy_subtypes(id, path, version, root) {
                self.record_lsp_client_trace("typeHierarchy/subtypes", root_detail);
            } else {
                self.status = lsp_command_queue_failed_status("typeHierarchy/subtypes");
            }
        }
    }

    pub(super) fn handle_lsp_type_hierarchy_supertypes(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspTypeHierarchyItem,
        supertypes: Option<Vec<LspTypeHierarchyItem>>,
        error: Option<String>,
    ) {
        if !self.type_hierarchy_event_matches(id, &path, version, &item) {
            return;
        }
        if let Some(error) = error {
            self.status = supertypes_failed_status(&error);
            return;
        }
        if let Some(mut items) = supertypes {
            items.retain(|item| type_hierarchy_item_is_current(&self.workspace.root, item));
            let count = items.len();
            self.type_hierarchy_open = true;
            self.type_hierarchy_supertypes = items;
            self.clamp_type_hierarchy_selection();
            self.status = type_hierarchy_count_status("supertype", count, &item.name);
        }
    }

    pub(super) fn handle_lsp_type_hierarchy_subtypes(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspTypeHierarchyItem,
        subtypes: Option<Vec<LspTypeHierarchyItem>>,
        error: Option<String>,
    ) {
        if !self.type_hierarchy_event_matches(id, &path, version, &item) {
            return;
        }
        if let Some(error) = error {
            self.status = subtypes_failed_status(&error);
            return;
        }
        if let Some(mut items) = subtypes {
            items.retain(|item| type_hierarchy_item_is_current(&self.workspace.root, item));
            let count = items.len();
            self.type_hierarchy_open = true;
            self.type_hierarchy_subtypes = items;
            self.clamp_type_hierarchy_selection();
            self.status = type_hierarchy_count_status("subtype", count, &item.name);
        }
    }

    fn type_hierarchy_prepare_response_matches(
        &self,
        id: BufferId,
        path: &std::path::Path,
        version: u64,
        line: usize,
        column: usize,
    ) -> bool {
        self.active == Some(id)
            && self.type_hierarchy_request_matches(path, line + 1, column + 1)
            && lsp_event_path_is_current(&self.workspace.root, path)
            && active_buffer_path_version_matches(self.active_buffer(), path, version)
    }

    fn type_hierarchy_event_matches(
        &self,
        id: BufferId,
        path: &std::path::Path,
        version: u64,
        item: &LspTypeHierarchyItem,
    ) -> bool {
        self.active == Some(id)
            && self.type_hierarchy_request_path_matches(path)
            && type_hierarchy_item_path_matches_request(&self.workspace.root, path, &item.path)
            && type_hierarchy_item_location_is_valid(item)
            && lsp_event_path_is_current(&self.workspace.root, path)
            && active_buffer_path_version_matches(self.active_buffer(), path, version)
            && self
                .type_hierarchy_root
                .as_ref()
                .is_some_and(|root| root.raw == item.raw)
    }

    fn type_hierarchy_request_matches(
        &self,
        path: &std::path::Path,
        line: usize,
        column: usize,
    ) -> bool {
        self.type_hierarchy_open
            && self.type_hierarchy_request_path_matches(path)
            && self.type_hierarchy_line == line
            && self.type_hierarchy_column == column
    }

    fn type_hierarchy_request_path_matches(&self, path: &std::path::Path) -> bool {
        self.type_hierarchy_path
            .as_deref()
            .is_some_and(|request_path| paths_match_lexically(request_path, path))
    }

    fn clamp_type_hierarchy_selection(&mut self) {
        let row_count = self.type_hierarchy_supertypes.len() + self.type_hierarchy_subtypes.len();
        clamp_selection(&mut self.type_hierarchy_selected, row_count);
    }
}

fn type_hierarchy_item_path_matches_request(
    workspace_root: &Path,
    request_path: &Path,
    item_path: &Path,
) -> bool {
    lsp_event_path_is_current(workspace_root, item_path)
        && paths_match_lexically(request_path, item_path)
}

fn type_hierarchy_item_is_current(workspace_root: &Path, item: &LspTypeHierarchyItem) -> bool {
    lsp_event_path_is_current(workspace_root, &item.path)
        && type_hierarchy_item_location_is_valid(item)
}

fn type_hierarchy_item_location_is_valid(item: &LspTypeHierarchyItem) -> bool {
    one_based_range_is_valid(item.line, item.column, item.end_line, item.end_column)
}

fn one_based_range_is_valid(
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
) -> bool {
    line > 0
        && column > 0
        && end_line > 0
        && end_column > 0
        && (end_line > line || (end_line == line && end_column >= column))
}

fn type_hierarchy_item_detail(item: &LspTypeHierarchyItem) -> String {
    let detail_path = type_hierarchy_detail_path(&item.path);
    let name = type_hierarchy_status_name(&item.name);
    let detail = format!("{}:{}:{} {}", detail_path, item.line, item.column, name);
    type_hierarchy_owned_display_label(
        detail,
        TYPE_HIERARCHY_ITEM_DETAIL_MAX_CHARS,
        "type hierarchy item",
    )
}

fn type_hierarchy_detail_path(path: &Path) -> Cow<'_, str> {
    let path = path.as_os_str().to_string_lossy();
    type_hierarchy_cow_display_label(path, TYPE_HIERARCHY_DETAIL_PATH_MAX_CHARS, ".")
}

fn type_hierarchy_failed_status(error: &str) -> String {
    format!("Type hierarchy failed: {}", display_error_label_cow(error))
}

fn loaded_type_hierarchy_status(item: &LspTypeHierarchyItem) -> String {
    format!(
        "Loaded type hierarchy for {}",
        type_hierarchy_status_name(&item.name)
    )
}

fn supertypes_failed_status(error: &str) -> String {
    format!("Supertypes failed: {}", display_error_label_cow(error))
}

fn subtypes_failed_status(error: &str) -> String {
    format!("Subtypes failed: {}", display_error_label_cow(error))
}

fn type_hierarchy_count_status(kind: &str, count: usize, name: &str) -> String {
    let kind = type_hierarchy_kind_label(kind);
    let name = type_hierarchy_status_name(name);
    match count {
        0 => format!("No {kind}s for {name}"),
        1 => format!("Loaded 1 {kind} for {name}"),
        _ => format!("Loaded {count} {kind}s for {name}"),
    }
}

fn type_hierarchy_status_name(name: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(name, TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS, "Unnamed")
}

fn type_hierarchy_kind_label(kind: &str) -> Cow<'_, str> {
    match kind {
        "supertype" | "subtype" => Cow::Borrowed(kind),
        _ => sanitized_display_label_cow(kind, TYPE_HIERARCHY_KIND_MAX_CHARS, "type"),
    }
}

fn type_hierarchy_cow_display_label<'a>(
    value: Cow<'a, str>,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    match value {
        Cow::Borrowed(value) => sanitized_display_label_cow(value, max_chars, fallback),
        Cow::Owned(value) => Cow::Owned(type_hierarchy_owned_display_label(
            value, max_chars, fallback,
        )),
    }
}

fn type_hierarchy_owned_display_label(value: String, max_chars: usize, fallback: &str) -> String {
    let label = sanitized_display_label_cow(&value, max_chars, fallback);
    let reuses_value = match &label {
        Cow::Borrowed(label) => label.as_ptr() == value.as_ptr() && label.len() == value.len(),
        Cow::Owned(_) => false,
    };
    if reuses_value {
        drop(label);
        return value;
    }
    label.into_owned()
}

#[cfg(test)]
mod tests {
    use super::{
        TYPE_HIERARCHY_DETAIL_PATH_MAX_CHARS, TYPE_HIERARCHY_ITEM_DETAIL_MAX_CHARS,
        TYPE_HIERARCHY_KIND_MAX_CHARS, TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS,
        loaded_type_hierarchy_status, subtypes_failed_status, supertypes_failed_status,
        type_hierarchy_count_status, type_hierarchy_failed_status, type_hierarchy_item_detail,
        type_hierarchy_item_is_current, type_hierarchy_kind_label,
    };
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, sanitized_display_label},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspTypeHierarchyItem, TextBuffer, Workspace};
    use serde_json::{Value, json};
    use std::{borrow::Cow, path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn type_hierarchy_failure_status_sanitizes_and_bounds_error_text() {
        let status = type_hierarchy_failed_status(&unsafe_error_text());

        assert_safe_status_detail(
            &status,
            "Type hierarchy failed: ",
            DISPLAY_ERROR_LABEL_MAX_CHARS,
        );
    }

    #[test]
    fn relation_failure_statuses_sanitize_and_bound_error_text() {
        let error = unsafe_error_text();

        assert_safe_status_detail(
            &supertypes_failed_status(&error),
            "Supertypes failed: ",
            DISPLAY_ERROR_LABEL_MAX_CHARS,
        );
        assert_safe_status_detail(
            &subtypes_failed_status(&error),
            "Subtypes failed: ",
            DISPLAY_ERROR_LABEL_MAX_CHARS,
        );
    }

    #[test]
    fn loaded_type_hierarchy_status_sanitizes_name_without_mutating_item() {
        let path = PathBuf::from("workspace/src/main.txt");
        let raw_name = unsafe_type_name();
        let raw = json!({ "name": raw_name, "data": { "id": 7 } });
        let item = item(&raw_name, path, raw.clone());

        let status = loaded_type_hierarchy_status(&item);

        assert_safe_status_detail(
            &status,
            "Loaded type hierarchy for ",
            TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS,
        );
        assert_eq!(item.name, raw_name);
        assert_eq!(item.raw, raw);
    }

    #[test]
    fn type_hierarchy_count_status_sanitizes_name() {
        let status = type_hierarchy_count_status("supertype", 2, &unsafe_type_name());

        assert_safe_status_detail(
            &status,
            "Loaded 2 supertypes for ",
            TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS,
        );
    }

    #[test]
    fn type_hierarchy_count_status_sanitizes_kind() {
        let status = type_hierarchy_count_status("super\ntype\u{202e}", 1, "Root");

        assert_eq!(status, "Loaded 1 super type for Root");
    }

    #[test]
    fn type_hierarchy_kind_label_borrows_builtin_kinds() {
        assert!(matches!(
            type_hierarchy_kind_label("supertype"),
            Cow::Borrowed("supertype")
        ));
        assert!(matches!(
            type_hierarchy_kind_label("subtype"),
            Cow::Borrowed("subtype")
        ));
    }

    #[test]
    fn type_hierarchy_kind_label_borrows_clean_custom_ascii_and_unicode() {
        assert!(matches!(
            type_hierarchy_kind_label("interface"),
            Cow::Borrowed("interface")
        ));

        let unicode = "custom-\u{03bb}";
        match type_hierarchy_kind_label(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn type_hierarchy_kind_label_owns_dirty_truncated_and_fallback_kinds() {
        let cases = [
            "super\ntype\u{202e}",
            "very-long-custom-type-hierarchy-kind-label",
            "\n\u{202e}",
        ];

        for kind in cases {
            let label = type_hierarchy_kind_label(kind);

            assert_eq!(
                label.as_ref(),
                sanitized_display_label(kind, TYPE_HIERARCHY_KIND_MAX_CHARS, "type")
            );
            assert!(
                matches!(label, Cow::Owned(_)),
                "expected owned label for {kind:?}"
            );
        }
    }

    #[test]
    fn type_hierarchy_kind_label_matches_sanitized_display_label_for_custom_kinds() {
        let cases = [
            "interface",
            "custom-\u{03bb}",
            "super\ntype\u{202e}",
            "very-long-custom-type-hierarchy-kind-label",
            "\n\u{202e}",
        ];

        for kind in cases {
            assert_eq!(
                type_hierarchy_kind_label(kind).as_ref(),
                sanitized_display_label(kind, TYPE_HIERARCHY_KIND_MAX_CHARS, "type")
            );
        }
    }

    #[test]
    fn type_hierarchy_item_detail_sanitizes_path_and_name_display_only() {
        let path = PathBuf::from("workspace").join(format!(
            "bad\npath\u{202e}-{}.txt",
            "very-long-path-".repeat(TYPE_HIERARCHY_DETAIL_PATH_MAX_CHARS)
        ));
        let raw_name = unsafe_type_name();
        let raw = json!({ "name": raw_name, "data": { "path": "raw\npath\u{202e}" } });
        let item = item(&raw_name, path.clone(), raw.clone());

        let detail = type_hierarchy_item_detail(&item);

        assert!(!detail.contains('\n'), "{detail}");
        assert!(!detail.contains('\u{202e}'), "{detail}");
        assert!(detail.contains("..."), "{detail}");
        assert!(
            detail.chars().count() <= TYPE_HIERARCHY_ITEM_DETAIL_MAX_CHARS,
            "{detail}"
        );
        assert_eq!(item.path, path);
        assert_eq!(item.name, raw_name);
        assert_eq!(item.raw, raw);
    }

    #[test]
    fn prepared_event_status_sanitizes_name_but_keeps_raw_root_item() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        let raw_name = unsafe_type_name();
        let raw = json!({ "name": raw_name, "data": { "id": 11 } });

        seed_type_hierarchy_request(&mut app, &path, 1, 1);

        app.handle_lsp_type_hierarchy_prepared(
            7,
            path.clone(),
            version,
            0,
            0,
            Some(vec![item(&raw_name, path, raw.clone())]),
            None,
        );

        assert_safe_status_detail(
            &app.status,
            "Loaded type hierarchy for ",
            TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS,
        );
        let root = app.type_hierarchy_root.as_ref().expect("root item");
        assert_eq!(root.name, raw_name);
        assert_eq!(root.raw, raw);
    }

    #[test]
    fn relation_event_status_sanitizes_name_but_keeps_raw_relation_items() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        let raw_name = unsafe_type_name();
        let root_raw = json!({ "name": raw_name, "role": "root" });
        let relation_raw = json!({ "name": raw_name, "role": "relation" });
        let root_item = item(&raw_name, path.clone(), root_raw.clone());
        let relation = item(&raw_name, path.clone(), relation_raw.clone());
        seed_type_hierarchy_root(&mut app, &path, root_item.clone());

        app.handle_lsp_type_hierarchy_supertypes(
            7,
            path,
            version,
            root_item,
            Some(vec![relation]),
            None,
        );

        assert_safe_status_detail(
            &app.status,
            "Loaded 1 supertype for ",
            TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS,
        );
        let stored_root = app.type_hierarchy_root.as_ref().expect("root item");
        assert_eq!(stored_root.name, raw_name);
        assert_eq!(stored_root.raw, root_raw);
        let stored_relation = app
            .type_hierarchy_supertypes
            .first()
            .expect("relation item");
        assert_eq!(stored_relation.name, raw_name);
        assert_eq!(stored_relation.raw, relation_raw);
    }

    #[test]
    fn type_hierarchy_relations_filter_destinations_outside_workspace() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let outside = PathBuf::from("outside/main.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        let root_item = item("Root", path.clone(), json!({"role": "root"}));
        let inside = item("Inside", path.clone(), json!({"role": "inside"}));
        let outside = item("Outside", outside, json!({"role": "outside"}));
        seed_type_hierarchy_root(&mut app, &path, root_item.clone());

        app.handle_lsp_type_hierarchy_supertypes(
            7,
            path.clone(),
            version,
            root_item.clone(),
            Some(vec![outside.clone(), inside.clone()]),
            None,
        );

        assert_eq!(app.type_hierarchy_supertypes, vec![inside.clone()]);
        assert_eq!(app.status, "Loaded 1 supertype for Root");

        app.handle_lsp_type_hierarchy_subtypes(
            7,
            path,
            version,
            root_item,
            Some(vec![outside, inside.clone()]),
            None,
        );

        assert_eq!(app.type_hierarchy_subtypes, vec![inside]);
        assert_eq!(app.status, "Loaded 1 subtype for Root");
    }

    #[test]
    fn type_hierarchy_relations_ignore_stale_buffer_id() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        let root_item = item("Root", path.clone(), json!({"role": "root"}));
        let relation = item("Inside", path.clone(), json!({"role": "inside"}));
        seed_type_hierarchy_root(&mut app, &path, root_item.clone());
        app.status = "unchanged".to_owned();

        app.handle_lsp_type_hierarchy_supertypes(
            8,
            path,
            version,
            root_item,
            Some(vec![relation]),
            None,
        );

        assert!(app.type_hierarchy_supertypes.is_empty());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn prepared_type_hierarchy_ignores_stale_request_location() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        seed_type_hierarchy_request(&mut app, &path, 9, 1);
        app.status = "unchanged".to_owned();

        app.handle_lsp_type_hierarchy_prepared(
            7,
            path.clone(),
            version,
            0,
            0,
            Some(vec![item("Root", path, json!({"role": "root"}))]),
            None,
        );

        assert!(app.type_hierarchy_root.is_none());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn prepared_type_hierarchy_ignores_root_from_different_active_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let other_path = root.join("src/lib.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        seed_type_hierarchy_request(&mut app, &path, 1, 1);

        app.handle_lsp_type_hierarchy_prepared(
            7,
            path,
            version,
            0,
            0,
            Some(vec![item("Root", other_path, json!({"role": "root"}))]),
            None,
        );

        assert!(app.type_hierarchy_root.is_none());
        assert_eq!(app.status, "No type hierarchy at 1:1");
    }

    #[test]
    fn type_hierarchy_relations_ignore_root_item_from_different_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let other_path = root.join("src/lib.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        let raw = json!({"role": "root"});
        let root_item = item("Root", path.clone(), raw.clone());
        let stale_item = item("Root", other_path, raw);
        let relation = item("Inside", path.clone(), json!({"role": "inside"}));
        seed_type_hierarchy_root(&mut app, &path, root_item);
        app.status = "unchanged".to_owned();

        app.handle_lsp_type_hierarchy_supertypes(
            7,
            path,
            version,
            stale_item,
            Some(vec![relation]),
            None,
        );

        assert!(app.type_hierarchy_supertypes.is_empty());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn type_hierarchy_relations_filter_invalid_item_ranges() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        let root_item = item("Root", path.clone(), json!({"role": "root"}));
        let valid = item("Inside", path.clone(), json!({"role": "inside"}));
        let mut invalid = item("Invalid", path.clone(), json!({"role": "invalid"}));
        invalid.end_column = 0;
        seed_type_hierarchy_root(&mut app, &path, root_item.clone());

        app.handle_lsp_type_hierarchy_subtypes(
            7,
            path,
            version,
            root_item,
            Some(vec![invalid, valid.clone()]),
            None,
        );

        assert_eq!(app.type_hierarchy_subtypes, vec![valid]);
        assert_eq!(app.status, "Loaded 1 subtype for Root");
    }

    #[test]
    fn type_hierarchy_item_current_check_rejects_reversed_ranges() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let mut item = item("Invalid", path, json!({"role": "invalid"}));
        item.column = 8;
        item.end_column = 7;

        assert!(!type_hierarchy_item_is_current(&root, &item));
    }

    #[test]
    fn type_hierarchy_relations_clamp_stale_selection_after_shrink() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.txt");
        let (mut app, version) = app_with_active_buffer(root, path.clone());
        let root_item = item("Root", path.clone(), json!({"role": "root"}));
        let relation = item("Inside", path.clone(), json!({"role": "inside"}));
        seed_type_hierarchy_root(&mut app, &path, root_item.clone());
        app.type_hierarchy_selected = 5;

        app.handle_lsp_type_hierarchy_supertypes(
            7,
            path,
            version,
            root_item,
            Some(vec![relation]),
            None,
        );

        assert_eq!(app.type_hierarchy_selected, 0);
    }

    fn unsafe_error_text() -> String {
        format!(
            "first line\nsecond line \u{202e}{}",
            "very-long-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn unsafe_type_name() -> String {
        format!(
            "Unsafe\nName \u{202e}{}",
            "very-long-type-name-".repeat(TYPE_HIERARCHY_STATUS_NAME_MAX_CHARS)
        )
    }

    fn assert_safe_status_detail(status: &str, prefix: &str, max_chars: usize) {
        let detail = status
            .strip_prefix(prefix)
            .unwrap_or_else(|| panic!("unexpected status: {status}"));

        assert!(!detail.contains('\n'), "{status}");
        assert!(!detail.contains('\u{202e}'), "{status}");
        assert!(detail.contains("..."), "{status}");
        assert!(detail.chars().count() <= max_chars, "{status}");
    }

    fn item(name: &str, path: PathBuf, raw: Value) -> LspTypeHierarchyItem {
        LspTypeHierarchyItem {
            name: name.to_owned(),
            detail: None,
            kind: 5,
            path,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 6,
            raw,
        }
    }

    fn seed_type_hierarchy_request(
        app: &mut KuroyaApp,
        path: &std::path::Path,
        line: usize,
        column: usize,
    ) {
        app.type_hierarchy_open = true;
        app.type_hierarchy_path = Some(path.to_path_buf());
        app.type_hierarchy_line = line;
        app.type_hierarchy_column = column;
    }

    fn seed_type_hierarchy_root(
        app: &mut KuroyaApp,
        path: &std::path::Path,
        root: LspTypeHierarchyItem,
    ) {
        seed_type_hierarchy_request(app, path, 1, 1);
        app.type_hierarchy_root = Some(root);
    }

    fn app_with_active_buffer(root: PathBuf, path: PathBuf) -> (KuroyaApp, u64) {
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path), "type Root\n".to_owned());
        buffer.set_single_cursor(0);
        let version = buffer.version();
        app.buffers.push(buffer);
        app.panes[0].active = Some(7);
        app.active = Some(7);
        (app, version)
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
}
