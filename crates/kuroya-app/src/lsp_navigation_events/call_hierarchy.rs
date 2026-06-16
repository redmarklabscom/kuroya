use crate::{
    KuroyaApp,
    lsp_runtime::lsp_command_queue_failed_status,
    path_display::{display_error_label_cow, sanitized_display_label_cow},
    ui_state::clamp_selection,
    workspace_state::{
        active_buffer_path_version_matches, lsp_event_path_is_current, paths_match_lexically,
    },
};
use kuroya_core::{BufferId, LspCallHierarchyCall, LspCallHierarchyItem};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

const CALL_HIERARCHY_STATUS_NAME_MAX_CHARS: usize = 120;
const CALL_HIERARCHY_DETAIL_PATH_MAX_CHARS: usize = 160;
const CALL_HIERARCHY_ITEM_DETAIL_MAX_CHARS: usize = 320;
const CALL_HIERARCHY_DIRECTION_MAX_CHARS: usize = 32;

impl KuroyaApp {
    pub(super) fn handle_lsp_call_hierarchy_prepared(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        line: usize,
        column: usize,
        items: Option<Vec<LspCallHierarchyItem>>,
        error: Option<String>,
    ) {
        if !self.call_hierarchy_prepare_response_matches(id, &path, version, line, column) {
            return;
        }

        if let Some(error) = error {
            self.clear_call_hierarchy();
            self.status = call_hierarchy_failed_status(&error);
            return;
        }

        let Some(root) = items.and_then(|items| {
            items.into_iter().find(|item| {
                call_hierarchy_item_path_matches_request(&self.workspace.root, &path, &item.path)
                    && call_hierarchy_item_location_is_valid(item)
            })
        }) else {
            self.clear_call_hierarchy();
            self.status = format!("No call hierarchy at {}:{}", line + 1, column + 1);
            return;
        };

        self.call_hierarchy_open = true;
        self.call_hierarchy_root = Some(root.clone());
        self.call_hierarchy_incoming.clear();
        self.call_hierarchy_outgoing.clear();
        self.call_hierarchy_selected = 0;
        self.call_hierarchy_path = Some(path.clone());
        self.call_hierarchy_line = line + 1;
        self.call_hierarchy_column = column + 1;
        self.status = loaded_call_hierarchy_status(&root.name);

        if let Some(client) = self.ensure_lsp_for_buffer(id) {
            let root_detail = call_hierarchy_item_detail(&root);
            if client.call_hierarchy_incoming(id, path.clone(), version, root.clone()) {
                self.record_lsp_client_trace("callHierarchy/incomingCalls", root_detail.clone());
            } else {
                self.status = lsp_command_queue_failed_status("callHierarchy/incomingCalls");
            }
            if client.call_hierarchy_outgoing(id, path, version, root) {
                self.record_lsp_client_trace("callHierarchy/outgoingCalls", root_detail);
            } else {
                self.status = lsp_command_queue_failed_status("callHierarchy/outgoingCalls");
            }
        }
    }

    pub(super) fn handle_lsp_call_hierarchy_incoming(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspCallHierarchyItem,
        calls: Option<Vec<LspCallHierarchyCall>>,
        error: Option<String>,
    ) {
        if !self.call_hierarchy_event_matches(id, &path, version, &item) {
            return;
        }
        if let Some(error) = error {
            self.status = incoming_calls_failed_status(&error);
            return;
        }
        if let Some(mut calls) = calls {
            calls.retain(|call| call_hierarchy_call_is_current(&self.workspace.root, call));
            let count = calls.len();
            self.call_hierarchy_open = true;
            self.call_hierarchy_incoming = calls;
            self.clamp_call_hierarchy_selection();
            self.status = call_hierarchy_count_status("incoming", count, &item.name);
        }
    }

    pub(super) fn handle_lsp_call_hierarchy_outgoing(
        &mut self,
        id: BufferId,
        path: PathBuf,
        version: u64,
        item: LspCallHierarchyItem,
        calls: Option<Vec<LspCallHierarchyCall>>,
        error: Option<String>,
    ) {
        if !self.call_hierarchy_event_matches(id, &path, version, &item) {
            return;
        }
        if let Some(error) = error {
            self.status = outgoing_calls_failed_status(&error);
            return;
        }
        if let Some(mut calls) = calls {
            calls.retain(|call| call_hierarchy_call_is_current(&self.workspace.root, call));
            let count = calls.len();
            self.call_hierarchy_open = true;
            self.call_hierarchy_outgoing = calls;
            self.clamp_call_hierarchy_selection();
            self.status = call_hierarchy_count_status("outgoing", count, &item.name);
        }
    }

    fn call_hierarchy_prepare_response_matches(
        &self,
        id: BufferId,
        path: &std::path::Path,
        version: u64,
        line: usize,
        column: usize,
    ) -> bool {
        self.active == Some(id)
            && self.call_hierarchy_request_matches(path, line + 1, column + 1)
            && lsp_event_path_is_current(&self.workspace.root, path)
            && active_buffer_path_version_matches(self.active_buffer(), path, version)
    }

    fn call_hierarchy_event_matches(
        &self,
        id: BufferId,
        path: &std::path::Path,
        version: u64,
        item: &LspCallHierarchyItem,
    ) -> bool {
        self.active == Some(id)
            && self.call_hierarchy_request_path_matches(path)
            && call_hierarchy_item_path_matches_request(&self.workspace.root, path, &item.path)
            && call_hierarchy_item_location_is_valid(item)
            && lsp_event_path_is_current(&self.workspace.root, path)
            && active_buffer_path_version_matches(self.active_buffer(), path, version)
            && self
                .call_hierarchy_root
                .as_ref()
                .is_some_and(|root| root.raw == item.raw)
    }

    fn call_hierarchy_request_matches(
        &self,
        path: &std::path::Path,
        line: usize,
        column: usize,
    ) -> bool {
        self.call_hierarchy_open
            && self.call_hierarchy_request_path_matches(path)
            && self.call_hierarchy_line == line
            && self.call_hierarchy_column == column
    }

    fn call_hierarchy_request_path_matches(&self, path: &std::path::Path) -> bool {
        self.call_hierarchy_path
            .as_deref()
            .is_some_and(|request_path| paths_match_lexically(request_path, path))
    }

    fn clamp_call_hierarchy_selection(&mut self) {
        let row_count = self.call_hierarchy_incoming.len() + self.call_hierarchy_outgoing.len();
        clamp_selection(&mut self.call_hierarchy_selected, row_count);
    }
}

fn call_hierarchy_item_path_matches_request(
    workspace_root: &Path,
    request_path: &Path,
    item_path: &Path,
) -> bool {
    lsp_event_path_is_current(workspace_root, item_path)
        && paths_match_lexically(request_path, item_path)
}

fn call_hierarchy_call_is_current(workspace_root: &Path, call: &LspCallHierarchyCall) -> bool {
    lsp_event_path_is_current(workspace_root, &call.item.path)
        && call_hierarchy_item_location_is_valid(&call.item)
        && call.ranges.iter().all(call_hierarchy_range_is_valid)
}

fn call_hierarchy_item_location_is_valid(item: &LspCallHierarchyItem) -> bool {
    one_based_range_is_valid(item.line, item.column, item.end_line, item.end_column)
}

fn call_hierarchy_range_is_valid(range: &kuroya_core::LspCallHierarchyRange) -> bool {
    one_based_range_is_valid(range.line, range.column, range.end_line, range.end_column)
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

fn call_hierarchy_item_detail(item: &LspCallHierarchyItem) -> String {
    let detail_path = call_hierarchy_detail_path(&item.path);
    let name = call_hierarchy_status_name(&item.name);
    let detail = format!("{}:{}:{} {}", detail_path, item.line, item.column, name);
    call_hierarchy_owned_display_label(
        detail,
        CALL_HIERARCHY_ITEM_DETAIL_MAX_CHARS,
        "call hierarchy item",
    )
}

fn call_hierarchy_detail_path(path: &Path) -> Cow<'_, str> {
    let path = path.as_os_str().to_string_lossy();
    call_hierarchy_cow_display_label(path, CALL_HIERARCHY_DETAIL_PATH_MAX_CHARS, ".")
}

fn call_hierarchy_failed_status(error: &str) -> String {
    format!("Call hierarchy failed: {}", display_error_label_cow(error))
}

fn loaded_call_hierarchy_status(name: &str) -> String {
    format!(
        "Loaded call hierarchy for {}",
        call_hierarchy_status_name(name)
    )
}

fn incoming_calls_failed_status(error: &str) -> String {
    format!("Incoming calls failed: {}", display_error_label_cow(error))
}

fn outgoing_calls_failed_status(error: &str) -> String {
    format!("Outgoing calls failed: {}", display_error_label_cow(error))
}

fn call_hierarchy_status_name(name: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(name, CALL_HIERARCHY_STATUS_NAME_MAX_CHARS, "Unnamed")
}

fn call_hierarchy_count_status(direction: &str, count: usize, name: &str) -> String {
    let direction = call_hierarchy_direction_label(direction);
    let name = call_hierarchy_status_name(name);
    match count {
        0 => format!("No {direction} calls for {name}"),
        1 => format!("Loaded 1 {direction} call for {name}"),
        _ => format!("Loaded {count} {direction} calls for {name}"),
    }
}

fn call_hierarchy_direction_label(direction: &str) -> Cow<'_, str> {
    match direction {
        "incoming" | "outgoing" => Cow::Borrowed(direction),
        _ => sanitized_display_label_cow(direction, CALL_HIERARCHY_DIRECTION_MAX_CHARS, "call"),
    }
}

fn call_hierarchy_cow_display_label<'a>(
    value: Cow<'a, str>,
    max_chars: usize,
    fallback: &str,
) -> Cow<'a, str> {
    match value {
        Cow::Borrowed(value) => sanitized_display_label_cow(value, max_chars, fallback),
        Cow::Owned(value) => Cow::Owned(call_hierarchy_owned_display_label(
            value, max_chars, fallback,
        )),
    }
}

fn call_hierarchy_owned_display_label(value: String, max_chars: usize, fallback: &str) -> String {
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
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, sanitized_display_label},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, LspCallHierarchyRange, TextBuffer, Workspace};
    use serde_json::{Value, json};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn call_hierarchy_error_statuses_sanitize_and_bound_lsp_error_text() {
        let error = unsafe_error_text("first line\nsecond line");

        assert_sanitized_error_status(
            &call_hierarchy_failed_status(&error),
            "Call hierarchy failed: ",
        );
        assert_sanitized_error_status(
            &incoming_calls_failed_status(&error),
            "Incoming calls failed: ",
        );
        assert_sanitized_error_status(
            &outgoing_calls_failed_status(&error),
            "Outgoing calls failed: ",
        );
    }

    #[test]
    fn call_hierarchy_name_statuses_sanitize_and_bound_lsp_item_names() {
        let name = unsafe_name_text("Root\nTarget");

        assert_sanitized_name_status(
            &loaded_call_hierarchy_status(&name),
            "Loaded call hierarchy for ",
        );
        assert_sanitized_name_status(
            &call_hierarchy_count_status("incoming", 0, &name),
            "No incoming calls for ",
        );
        assert_sanitized_name_status(
            &call_hierarchy_count_status("outgoing", 1, &name),
            "Loaded 1 outgoing call for ",
        );
        assert_sanitized_name_status(
            &call_hierarchy_count_status("incoming", 2, &name),
            "Loaded 2 incoming calls for ",
        );
    }

    #[test]
    fn call_hierarchy_count_status_sanitizes_direction() {
        let status = call_hierarchy_count_status("in\ncoming\u{202e}", 1, "Root");

        assert_safe_status_text(&status);
        assert_eq!(status, "Loaded 1 in coming call for Root");
    }

    #[test]
    fn call_hierarchy_direction_label_borrows_built_ins() {
        for direction in ["incoming", "outgoing"] {
            let label = call_hierarchy_direction_label(direction);

            assert_eq!(label, direction);
            assert!(
                matches!(label, Cow::Borrowed(_)),
                "expected built-in direction {direction:?} to borrow"
            );
        }
    }

    #[test]
    fn call_hierarchy_direction_label_borrows_clean_custom_directions() {
        for direction in ["references", "calls-\u{03bb}"] {
            let label = call_hierarchy_direction_label(direction);

            assert_eq!(label, direction);
            assert!(
                matches!(label, Cow::Borrowed(_)),
                "expected clean direction {direction:?} to borrow"
            );
        }
    }

    #[test]
    fn call_hierarchy_direction_label_owns_dirty_truncated_and_fallback_directions() {
        let cases = [
            "in\ncoming\u{202e}",
            "very-long-custom-call-hierarchy-direction",
            "\n\u{202e}\t",
        ];

        for direction in cases {
            let label = call_hierarchy_direction_label(direction);

            assert_eq!(
                label.as_ref(),
                sanitized_display_label(direction, CALL_HIERARCHY_DIRECTION_MAX_CHARS, "call")
            );
            assert!(
                matches!(label, Cow::Owned(_)),
                "expected direction {direction:?} to own"
            );
        }
    }

    #[test]
    fn call_hierarchy_count_status_matches_sanitized_custom_direction_labels() {
        let cases = [
            "references",
            "calls-\u{03bb}",
            "in\ncoming\u{202e}",
            "very-long-custom-call-hierarchy-direction",
            "\n\u{202e}\t",
        ];

        for direction in cases {
            let expected_direction =
                sanitized_display_label(direction, CALL_HIERARCHY_DIRECTION_MAX_CHARS, "call");

            assert_eq!(
                call_hierarchy_count_status(direction, 2, "Root"),
                format!("Loaded 2 {expected_direction} calls for Root")
            );
        }
    }

    #[test]
    fn call_hierarchy_status_name_uses_blank_name_fallback() {
        assert_eq!(
            loaded_call_hierarchy_status("\n\u{202e}\t"),
            "Loaded call hierarchy for Unnamed"
        );
    }

    #[test]
    fn call_hierarchy_item_detail_sanitizes_path_and_name_display_only() {
        let path = PathBuf::from("workspace").join(format!(
            "bad\npath\u{202e}-{}.rs",
            "very-long-path-".repeat(CALL_HIERARCHY_DETAIL_PATH_MAX_CHARS)
        ));
        let name = unsafe_name_text("Caller\nTarget");
        let raw = json!({ "name": name, "data": { "path": "raw\npath\u{202e}" } });
        let item = hierarchy_item(&path, name.clone(), None, raw.clone());

        let detail = call_hierarchy_item_detail(&item);

        assert_safe_status_text(&detail);
        assert!(detail.contains("..."), "{detail}");
        assert!(
            detail.chars().count() <= CALL_HIERARCHY_ITEM_DETAIL_MAX_CHARS,
            "{detail}"
        );
        assert_eq!(item.path, path);
        assert_eq!(item.name, name);
        assert_eq!(item.raw, raw);
    }

    #[test]
    fn prepared_call_hierarchy_status_is_display_only_and_preserves_raw_root() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        app.workspace_trusted = false;
        let version = app.buffer(7).expect("buffer").version();
        let raw_name = unsafe_name_text("Root\nTarget");
        let raw_detail = unsafe_name_text("detail\ntext");
        let raw_payload = json!({
            "name": raw_name,
            "detail": raw_detail,
            "data": { "token": "raw\npayload\u{202e}" },
        });
        let item = hierarchy_item(
            &path,
            raw_name.clone(),
            Some(raw_detail.clone()),
            raw_payload.clone(),
        );

        seed_call_hierarchy_request(&mut app, &path, 1, 1);

        app.handle_lsp_call_hierarchy_prepared(7, path, version, 0, 0, Some(vec![item]), None);

        assert_sanitized_name_status(&app.status, "Loaded call hierarchy for ");
        let stored = app
            .call_hierarchy_root
            .as_ref()
            .expect("call hierarchy root");
        assert_eq!(stored.name, raw_name);
        assert_eq!(stored.detail.as_deref(), Some(raw_detail.as_str()));
        assert_eq!(stored.raw, raw_payload);
    }

    #[test]
    fn incoming_and_outgoing_call_statuses_are_display_only_and_preserve_raw_calls() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        let raw_root_name = unsafe_name_text("Root\nTarget");
        let raw_call_name = unsafe_name_text("Caller\nTarget");
        let root_payload = json!({ "id": "root", "name": raw_root_name });
        let call_payload =
            json!({ "id": "call", "name": raw_call_name, "display": "raw\nvalue\u{202e}" });
        let root_item = hierarchy_item(&path, raw_root_name.clone(), None, root_payload);
        let call_item = hierarchy_item(&path, raw_call_name.clone(), None, call_payload.clone());
        let call = LspCallHierarchyCall {
            item: call_item,
            ranges: Vec::new(),
        };
        seed_call_hierarchy_root(&mut app, &path, root_item.clone());

        app.handle_lsp_call_hierarchy_incoming(
            7,
            path.clone(),
            version,
            root_item.clone(),
            Some(vec![call.clone()]),
            None,
        );

        assert_sanitized_name_status(&app.status, "Loaded 1 incoming call for ");
        assert_eq!(app.call_hierarchy_incoming[0].item.name, raw_call_name);
        assert_eq!(app.call_hierarchy_incoming[0].item.raw, call_payload);

        app.handle_lsp_call_hierarchy_outgoing(
            7,
            path,
            version,
            root_item,
            Some(vec![call.clone()]),
            None,
        );

        assert_sanitized_name_status(&app.status, "Loaded 1 outgoing call for ");
        assert_eq!(app.call_hierarchy_outgoing[0].item.name, raw_call_name);
        assert_eq!(app.call_hierarchy_outgoing[0].item.raw, call_payload);
    }

    #[test]
    fn call_hierarchy_relations_filter_destinations_outside_workspace() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let outside = PathBuf::from("outside/main.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        let root_item = hierarchy_item(&path, "Root".to_owned(), None, json!({"id": "root"}));
        let inside_call = LspCallHierarchyCall {
            item: hierarchy_item(&path, "Inside".to_owned(), None, json!({"id": "inside"})),
            ranges: Vec::new(),
        };
        let outside_call = LspCallHierarchyCall {
            item: hierarchy_item(
                &outside,
                "Outside".to_owned(),
                None,
                json!({"id": "outside"}),
            ),
            ranges: Vec::new(),
        };
        seed_call_hierarchy_root(&mut app, &path, root_item.clone());

        app.handle_lsp_call_hierarchy_incoming(
            7,
            path.clone(),
            version,
            root_item.clone(),
            Some(vec![outside_call.clone(), inside_call.clone()]),
            None,
        );

        assert_eq!(app.call_hierarchy_incoming, vec![inside_call.clone()]);
        assert_eq!(app.status, "Loaded 1 incoming call for Root");

        app.handle_lsp_call_hierarchy_outgoing(
            7,
            path,
            version,
            root_item,
            Some(vec![outside_call, inside_call.clone()]),
            None,
        );

        assert_eq!(app.call_hierarchy_outgoing, vec![inside_call]);
        assert_eq!(app.status, "Loaded 1 outgoing call for Root");
    }

    #[test]
    fn call_hierarchy_relations_ignore_stale_buffer_id() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        let root_item = hierarchy_item(&path, "Root".to_owned(), None, json!({"id": "root"}));
        let call = LspCallHierarchyCall {
            item: hierarchy_item(&path, "Inside".to_owned(), None, json!({"id": "inside"})),
            ranges: Vec::new(),
        };
        seed_call_hierarchy_root(&mut app, &path, root_item.clone());
        app.status = "unchanged".to_owned();

        app.handle_lsp_call_hierarchy_incoming(8, path, version, root_item, Some(vec![call]), None);

        assert!(app.call_hierarchy_incoming.is_empty());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn prepared_call_hierarchy_ignores_stale_request_location() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        seed_call_hierarchy_request(&mut app, &path, 9, 1);
        app.status = "unchanged".to_owned();

        app.handle_lsp_call_hierarchy_prepared(
            7,
            path.clone(),
            version,
            0,
            0,
            Some(vec![hierarchy_item(
                &path,
                "Root".to_owned(),
                None,
                json!({"id": "root"}),
            )]),
            None,
        );

        assert!(app.call_hierarchy_root.is_none());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn prepared_call_hierarchy_ignores_root_from_different_active_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other_path = root.join("src/lib.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        seed_call_hierarchy_request(&mut app, &path, 1, 1);

        app.handle_lsp_call_hierarchy_prepared(
            7,
            path,
            version,
            0,
            0,
            Some(vec![hierarchy_item(
                &other_path,
                "Root".to_owned(),
                None,
                json!({"id": "root"}),
            )]),
            None,
        );

        assert!(app.call_hierarchy_root.is_none());
        assert_eq!(app.status, "No call hierarchy at 1:1");
    }

    #[test]
    fn call_hierarchy_relations_ignore_root_item_from_different_path() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let other_path = root.join("src/lib.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        let raw = json!({"id": "root"});
        let root_item = hierarchy_item(&path, "Root".to_owned(), None, raw.clone());
        let stale_item = hierarchy_item(&other_path, "Root".to_owned(), None, raw);
        let call = LspCallHierarchyCall {
            item: hierarchy_item(&path, "Inside".to_owned(), None, json!({"id": "inside"})),
            ranges: Vec::new(),
        };
        seed_call_hierarchy_root(&mut app, &path, root_item);
        app.status = "unchanged".to_owned();

        app.handle_lsp_call_hierarchy_incoming(
            7,
            path,
            version,
            stale_item,
            Some(vec![call]),
            None,
        );

        assert!(app.call_hierarchy_incoming.is_empty());
        assert_eq!(app.status, "unchanged");
    }

    #[test]
    fn call_hierarchy_relations_filter_invalid_call_ranges() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        let root_item = hierarchy_item(&path, "Root".to_owned(), None, json!({"id": "root"}));
        let valid_call = LspCallHierarchyCall {
            item: hierarchy_item(&path, "Inside".to_owned(), None, json!({"id": "inside"})),
            ranges: vec![LspCallHierarchyRange {
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
            }],
        };
        let invalid_call = LspCallHierarchyCall {
            item: hierarchy_item(&path, "Invalid".to_owned(), None, json!({"id": "invalid"})),
            ranges: vec![LspCallHierarchyRange {
                line: 3,
                column: 8,
                end_line: 3,
                end_column: 7,
            }],
        };
        seed_call_hierarchy_root(&mut app, &path, root_item.clone());

        app.handle_lsp_call_hierarchy_outgoing(
            7,
            path,
            version,
            root_item,
            Some(vec![invalid_call, valid_call.clone()]),
            None,
        );

        assert_eq!(app.call_hierarchy_outgoing, vec![valid_call]);
        assert_eq!(app.status, "Loaded 1 outgoing call for Root");
    }

    #[test]
    fn call_hierarchy_relations_clamp_stale_selection_after_shrink() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_with_active_buffer(root, path.clone(), "fn main() {}\n", 0, 0);
        let version = app.buffer(7).expect("buffer").version();
        let root_item = hierarchy_item(&path, "Root".to_owned(), None, json!({"id": "root"}));
        let call = LspCallHierarchyCall {
            item: hierarchy_item(&path, "Inside".to_owned(), None, json!({"id": "inside"})),
            ranges: Vec::new(),
        };
        seed_call_hierarchy_root(&mut app, &path, root_item.clone());
        app.call_hierarchy_selected = 5;

        app.handle_lsp_call_hierarchy_incoming(7, path, version, root_item, Some(vec![call]), None);

        assert_eq!(app.call_hierarchy_selected, 0);
    }

    fn hierarchy_item(
        path: &std::path::Path,
        name: String,
        detail: Option<String>,
        raw: Value,
    ) -> LspCallHierarchyItem {
        LspCallHierarchyItem {
            name,
            detail,
            kind: 12,
            path: path.to_path_buf(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 4,
            raw,
        }
    }

    fn seed_call_hierarchy_request(
        app: &mut KuroyaApp,
        path: &std::path::Path,
        line: usize,
        column: usize,
    ) {
        app.call_hierarchy_open = true;
        app.call_hierarchy_path = Some(path.to_path_buf());
        app.call_hierarchy_line = line;
        app.call_hierarchy_column = column;
    }

    fn seed_call_hierarchy_root(
        app: &mut KuroyaApp,
        path: &std::path::Path,
        root: LspCallHierarchyItem,
    ) {
        seed_call_hierarchy_request(app, path, 1, 1);
        app.call_hierarchy_root = Some(root);
    }

    fn app_with_active_buffer(
        root: PathBuf,
        path: PathBuf,
        text: &str,
        line: usize,
        column: usize,
    ) -> KuroyaApp {
        let mut app = app_for_test(root);
        let mut buffer = TextBuffer::from_text(7, Some(path), text.to_owned());
        buffer.set_single_cursor(buffer.line_column_to_char(line, column));
        app.active = Some(7);
        app.buffers.push(buffer);
        app
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

    fn unsafe_error_text(prefix: &str) -> String {
        format!(
            "{prefix}\u{202e}{}tail\u{2029}",
            "very-long-lsp-error-".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS)
        )
    }

    fn unsafe_name_text(prefix: &str) -> String {
        format!(
            "{prefix}\u{202e}{}tail\u{2029}",
            "very-long-lsp-name-".repeat(CALL_HIERARCHY_STATUS_NAME_MAX_CHARS)
        )
    }

    fn assert_sanitized_error_status(status: &str, prefix: &str) {
        assert_safe_status_text(status);
        assert!(status.starts_with(prefix), "{status}");
        assert!(status.contains("..."), "{status}");
        assert!(
            status.chars().count() <= prefix.chars().count() + DISPLAY_ERROR_LABEL_MAX_CHARS,
            "{status}"
        );
    }

    fn assert_sanitized_name_status(status: &str, prefix: &str) {
        assert_safe_status_text(status);
        assert!(status.starts_with(prefix), "{status}");
        assert!(status.contains("..."), "{status}");
        assert!(
            status.chars().count() <= prefix.chars().count() + CALL_HIERARCHY_STATUS_NAME_MAX_CHARS,
            "{status}"
        );
    }

    fn assert_safe_status_text(status: &str) {
        assert!(
            !status.chars().any(is_unsafe_status_char),
            "status contains unsafe display characters: {status:?}"
        );
    }

    fn is_unsafe_status_char(ch: char) -> bool {
        ch.is_control()
            || matches!(
                ch,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{2028}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    }
}
