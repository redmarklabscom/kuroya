use super::{TerminalPane, TerminalSession, search::terminal_plain_text};
use crate::persistence::{PersistedTerminalProcessStatus, PersistedTerminalSession};
use std::path::{Component, Path, PathBuf};

const MAX_RESTORED_TERMINAL_SESSIONS: usize = 12;
const MAX_PERSISTED_TERMINAL_SCROLLBACK_BYTES: usize = 256 * 1024;
const MAX_PERSISTED_TERMINAL_SCROLLBACK_TOTAL_BYTES: usize = 1024 * 1024;
const PERSISTED_TERMINAL_SCROLLBACK_SANITIZE_LOOKBACK_BYTES: usize = 4 * 1024;
const PERSISTED_TERMINAL_LABEL_MAX_CHARS: usize = 120;
const PERSISTED_TERMINAL_LABEL_MAX_UTF8_BYTES: usize = PERSISTED_TERMINAL_LABEL_MAX_CHARS * 4;
const PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER: &str = "...";
const PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER_CHARS: usize = 3;

impl TerminalPane {
    pub(crate) fn terminal_session_snapshots(&self) -> Vec<PersistedTerminalSession> {
        let indices = terminal_session_snapshot_indices(self.sessions.len(), self.active_session);
        let mut scrollbacks = persisted_terminal_scrollback_by_session(
            &self.sessions,
            &indices,
            self.active_session,
            MAX_PERSISTED_TERMINAL_SCROLLBACK_TOTAL_BYTES,
        )
        .into_iter();
        let mut launch_cwd = None::<PathBuf>;
        let mut snapshots = Vec::with_capacity(indices.len());

        for session_index in indices {
            let scrollback = scrollbacks.next().unwrap_or_default();
            let Some(session) = self.sessions.get(session_index) else {
                continue;
            };
            let process_label =
                normalized_persisted_terminal_label(session.process_label.as_deref());
            let scrollback_offset = if scrollback.is_empty() {
                0
            } else {
                session.scrollback()
            };
            let cwd = session
                .initial_cwd
                .clone()
                .unwrap_or_else(|| launch_cwd.get_or_insert_with(|| self.launch_cwd()).clone());
            snapshots.push(PersistedTerminalSession {
                cwd: Some(cwd),
                scrollback,
                scrollback_offset,
                custom_title: normalized_persisted_terminal_label(session.custom_title.as_deref()),
                process_status: persisted_terminal_process_status(session, process_label.as_ref()),
                process_label,
                window_title: normalized_persisted_terminal_label(
                    session.parser.callbacks().window_title.as_deref(),
                ),
            });
        }

        snapshots
    }

    pub(crate) fn terminal_active_session_for_restore(&self) -> usize {
        let indices = terminal_session_snapshot_indices(self.sessions.len(), self.active_session);
        if indices.is_empty() {
            return 0;
        }
        indices
            .iter()
            .position(|index| *index == self.active_session)
            .unwrap_or_else(|| self.active_session.min(indices.len().saturating_sub(1)))
    }

    pub(crate) fn terminal_split_view_for_restore(&self) -> bool {
        self.split_view
    }

    pub(crate) fn terminal_split_weights_for_restore(&self) -> Vec<f32> {
        let indices = terminal_session_snapshot_indices(self.sessions.len(), self.active_session);
        let mut weights = Vec::with_capacity(indices.len());

        for index in indices {
            weights.push(restored_terminal_split_weight(&self.split_weights, index));
        }

        weights
    }

    pub(crate) fn restore_terminal_sessions(
        &mut self,
        snapshots: &[PersistedTerminalSession],
        active_session: usize,
        split_view: bool,
        split_weights: &[f32],
        allow_auto_start_shell: bool,
    ) {
        if snapshots.is_empty() {
            return;
        }

        let launch_cwd = self.launch_cwd();
        let prepared_snapshots = restored_terminal_session_snapshots(
            snapshots,
            active_session,
            &self.cwd,
            &launch_cwd,
            allow_auto_start_shell,
            MAX_PERSISTED_TERMINAL_SCROLLBACK_TOTAL_BYTES,
        );
        if prepared_snapshots.is_empty() {
            self.close_sessions_for_restore();
            self.sessions.clear();
            self.split_weights.clear();
            self.selected_session_id = None;
            self.selected_text = None;
            self.selection_drag = None;
            self.pending_paste_session_id = None;
            self.pending_kill_session_id = None;
            self.pending_rename_session_id = None;
            self.rename_session_input.clear();
            self.pending_multiline_paste = None;
            self.search_cache = super::TerminalSearchCache::default();
            self.search_match = 0;
            self.prune_stale_session_state();
            return;
        }
        let restored_active_session =
            restored_terminal_active_session(&prepared_snapshots, active_session);
        let restored_split_weights =
            restored_terminal_split_weights_for_snapshots(split_weights, &prepared_snapshots);

        self.close_sessions_for_restore();
        self.sessions.clear();
        self.split_weights.clear();
        self.active_session = 0;
        self.next_session_id = self.next_session_id.max(1);
        self.sessions.reserve(prepared_snapshots.len());

        for snapshot in prepared_snapshots {
            let id = self.next_session_id;
            self.next_session_id += 1;
            let mut session = TerminalSession::new(id, self.last_size, self.scrollback_rows);
            session.auto_start_shell = allow_auto_start_shell
                && snapshot.process_label_allows_auto_start
                && snapshot.process_status.is_none()
                && snapshot.can_auto_start_shell;
            session.initial_cwd = Some(snapshot.cwd);
            session.custom_title = snapshot.custom_title;
            session.process_label = snapshot.process_label;
            restore_terminal_process_status(&mut session, snapshot.process_status);
            session.parser.callbacks_mut().window_title = snapshot.window_title;
            restore_prepared_terminal_scrollback(&mut session, snapshot.scrollback);
            restore_terminal_scrollback_offset(&mut session, snapshot.scrollback_offset);
            self.sessions.push(session);
        }

        if self.sessions.is_empty() {
            return;
        }

        self.active_session = restored_active_session;
        self.split_view = split_view && self.sessions.len() > 1;
        self.split_weights = restored_split_weights;
        self.selected_session_id = None;
        self.selected_text = None;
        self.selection_drag = None;
        self.pending_paste_session_id = None;
        self.pending_kill_session_id = None;
        self.pending_rename_session_id = None;
        self.rename_session_input.clear();
        self.pending_multiline_paste = None;
        self.search_cache = super::TerminalSearchCache::default();
        self.search_match = 0;
        self.prune_stale_session_state();
    }

    fn close_sessions_for_restore(&mut self) {
        for session in &mut self.sessions {
            session.close();
        }
    }
}

struct RestoredTerminalSessionSnapshot {
    snapshot_index: usize,
    cwd: PathBuf,
    can_auto_start_shell: bool,
    custom_title: Option<String>,
    process_label: Option<String>,
    process_label_allows_auto_start: bool,
    process_status: Option<PersistedTerminalProcessStatus>,
    window_title: Option<String>,
    scrollback: String,
    scrollback_offset: usize,
}

fn persisted_terminal_process_status(
    session: &TerminalSession,
    process_label: Option<&String>,
) -> Option<PersistedTerminalProcessStatus> {
    process_label?;
    if session.started {
        return Some(PersistedTerminalProcessStatus::Running);
    }
    if session.last_process_terminal_error {
        return Some(PersistedTerminalProcessStatus::TerminalError);
    }
    match session.last_process_exit_code {
        Some(exit_code) => Some(PersistedTerminalProcessStatus::Exited {
            exit_code: Some(exit_code),
        }),
        None => Some(PersistedTerminalProcessStatus::Stopped),
    }
}

fn restore_terminal_process_status(
    session: &mut TerminalSession,
    status: Option<PersistedTerminalProcessStatus>,
) {
    if session.process_label.is_none() {
        return;
    }
    match status {
        Some(PersistedTerminalProcessStatus::Exited { exit_code }) => {
            session.last_process_exit_code = exit_code;
            session.last_process_terminal_error = false;
        }
        Some(PersistedTerminalProcessStatus::TerminalError) => {
            session.last_process_exit_code = None;
            session.last_process_terminal_error = true;
        }
        Some(PersistedTerminalProcessStatus::Running)
        | Some(PersistedTerminalProcessStatus::Stopped)
        | Some(PersistedTerminalProcessStatus::Unknown)
        | None => {
            session.last_process_exit_code = None;
            session.last_process_terminal_error = false;
        }
    }
}

fn restored_terminal_session_snapshots(
    snapshots: &[PersistedTerminalSession],
    active_session: usize,
    workspace_root: &Path,
    fallback_cwd: &Path,
    allow_auto_start_shell: bool,
    max_total_bytes: usize,
) -> Vec<RestoredTerminalSessionSnapshot> {
    let snapshot_indices = terminal_session_snapshot_indices(snapshots.len(), active_session);
    if snapshot_indices.is_empty() {
        return Vec::new();
    }

    let restored_scrollback_active_session = snapshot_indices
        .iter()
        .position(|index| *index == active_session)
        .unwrap_or_else(|| active_session.min(snapshot_indices.len().saturating_sub(1)));
    let mut scrollbacks = restored_terminal_scrollback_by_snapshot(
        snapshots,
        &snapshot_indices,
        restored_scrollback_active_session,
        max_total_bytes,
    )
    .into_iter();

    let mut restored = Vec::with_capacity(snapshot_indices.len());
    for snapshot_index in snapshot_indices {
        let Some(snapshot) = snapshots.get(snapshot_index) else {
            continue;
        };
        let raw_process_label = snapshot.process_label.as_deref();
        let process_label = normalized_persisted_terminal_label(raw_process_label);
        let custom_title = normalized_persisted_terminal_label(snapshot.custom_title.as_deref());
        let process_label_allows_auto_start = restored_terminal_process_label_allows_auto_start(
            raw_process_label,
            process_label.as_deref(),
        );
        let process_status = snapshot.process_status;
        let restored_cwd =
            restored_terminal_cwd(snapshot.cwd.as_deref(), workspace_root, fallback_cwd);
        let can_auto_start_shell = restored_cwd.can_auto_start_shell;
        let window_title = normalized_persisted_terminal_label(snapshot.window_title.as_deref());
        let scrollback = scrollbacks.next().unwrap_or_default();

        if !restored_terminal_snapshot_has_state(
            allow_auto_start_shell,
            can_auto_start_shell,
            process_label_allows_auto_start,
            process_status,
            custom_title.as_deref(),
            process_label.as_deref(),
            window_title.as_deref(),
            &scrollback,
        ) {
            continue;
        }

        restored.push(RestoredTerminalSessionSnapshot {
            snapshot_index,
            cwd: restored_cwd.path,
            can_auto_start_shell,
            custom_title,
            process_label,
            process_label_allows_auto_start,
            process_status,
            window_title,
            scrollback,
            scrollback_offset: snapshot.scrollback_offset,
        });
    }

    restored
}

fn restored_terminal_snapshot_has_state(
    allow_auto_start_shell: bool,
    can_auto_start_shell: bool,
    process_label_allows_auto_start: bool,
    process_status: Option<PersistedTerminalProcessStatus>,
    custom_title: Option<&str>,
    process_label: Option<&str>,
    window_title: Option<&str>,
    scrollback: &str,
) -> bool {
    !scrollback.is_empty()
        || custom_title.is_some()
        || process_label.is_some()
        || window_title.is_some()
        || (allow_auto_start_shell
            && process_label_allows_auto_start
            && process_status.is_none()
            && can_auto_start_shell)
}

fn restored_terminal_active_session(
    snapshots: &[RestoredTerminalSessionSnapshot],
    active_session: usize,
) -> usize {
    snapshots
        .iter()
        .position(|snapshot| snapshot.snapshot_index == active_session)
        .or_else(|| {
            snapshots
                .iter()
                .position(|snapshot| snapshot.snapshot_index > active_session)
        })
        .unwrap_or_else(|| snapshots.len().saturating_sub(1))
}

fn restored_terminal_split_weights_for_snapshots(
    weights: &[f32],
    snapshots: &[RestoredTerminalSessionSnapshot],
) -> Vec<f32> {
    let mut restored = Vec::with_capacity(snapshots.len());

    for snapshot in snapshots {
        restored.push(restored_terminal_split_weight(
            weights,
            snapshot.snapshot_index,
        ));
    }

    restored
}

fn restored_terminal_scrollback_by_snapshot(
    snapshots: &[PersistedTerminalSession],
    snapshot_indices: &[usize],
    active_snapshot_position: usize,
    max_total_bytes: usize,
) -> Vec<String> {
    let mut scrollbacks = vec![String::new(); snapshot_indices.len()];
    let mut remaining_bytes = max_total_bytes;

    restore_terminal_scrollback_for_snapshot(
        snapshots,
        snapshot_indices,
        &mut scrollbacks,
        active_snapshot_position,
        &mut remaining_bytes,
    );

    for snapshot_position in 0..snapshot_indices.len() {
        if snapshot_position == active_snapshot_position {
            continue;
        }
        restore_terminal_scrollback_for_snapshot(
            snapshots,
            snapshot_indices,
            &mut scrollbacks,
            snapshot_position,
            &mut remaining_bytes,
        );
    }

    scrollbacks
}

fn restore_terminal_scrollback_for_snapshot(
    snapshots: &[PersistedTerminalSession],
    snapshot_indices: &[usize],
    scrollbacks: &mut [String],
    snapshot_position: usize,
    remaining_bytes: &mut usize,
) {
    if *remaining_bytes == 0 {
        return;
    }

    let Some(snapshot_index) = snapshot_indices.get(snapshot_position).copied() else {
        return;
    };
    let Some(snapshot) = snapshots.get(snapshot_index) else {
        return;
    };
    let limit = (*remaining_bytes).min(MAX_PERSISTED_TERMINAL_SCROLLBACK_BYTES);
    let scrollback = persisted_terminal_scrollback_with_budget(&snapshot.scrollback, limit);
    *remaining_bytes = remaining_bytes.saturating_sub(scrollback.charged_bytes);
    if let Some(slot) = scrollbacks.get_mut(snapshot_position) {
        *slot = scrollback.text;
    }
}

fn terminal_session_snapshot_indices(session_count: usize, active_session: usize) -> Vec<usize> {
    let snapshot_count = session_count.min(MAX_RESTORED_TERMINAL_SESSIONS);
    let mut indices =
        Vec::with_capacity(snapshot_count + usize::from(active_session < session_count));
    indices.extend(0..snapshot_count);
    if active_session < session_count && !indices.contains(&active_session) {
        if indices.len() == MAX_RESTORED_TERMINAL_SESSIONS {
            indices.pop();
        }
        indices.push(active_session);
    }
    indices
}

fn persisted_terminal_scrollback_by_session(
    sessions: &[TerminalSession],
    indices: &[usize],
    active_session: usize,
    max_total_bytes: usize,
) -> Vec<String> {
    let mut scrollbacks = vec![String::new(); indices.len()];
    let mut remaining_bytes = max_total_bytes;

    if let Some(active_snapshot_index) = indices.iter().position(|index| *index == active_session) {
        persist_terminal_scrollback_for_snapshot(
            sessions,
            indices,
            &mut scrollbacks,
            active_snapshot_index,
            &mut remaining_bytes,
        );
    }

    for snapshot_index in 0..indices.len() {
        if indices.get(snapshot_index) == Some(&active_session) {
            continue;
        }
        persist_terminal_scrollback_for_snapshot(
            sessions,
            indices,
            &mut scrollbacks,
            snapshot_index,
            &mut remaining_bytes,
        );
    }

    scrollbacks
}

fn persist_terminal_scrollback_for_snapshot(
    sessions: &[TerminalSession],
    indices: &[usize],
    scrollbacks: &mut [String],
    snapshot_index: usize,
    remaining_bytes: &mut usize,
) {
    if *remaining_bytes == 0 {
        return;
    }

    let Some(session_index) = indices.get(snapshot_index).copied() else {
        return;
    };
    let Some(session) = sessions.get(session_index) else {
        return;
    };
    let limit = (*remaining_bytes).min(MAX_PERSISTED_TERMINAL_SCROLLBACK_BYTES);
    let scrollback = persisted_terminal_scrollback_with_budget(&session.search_buffer, limit);
    *remaining_bytes = remaining_bytes.saturating_sub(scrollback.charged_bytes);
    if let Some(slot) = scrollbacks.get_mut(snapshot_index) {
        *slot = scrollback.text;
    }
}

#[cfg(test)]
fn restored_terminal_split_weights(weights: &[f32], session_count: usize) -> Vec<f32> {
    if session_count == 0 {
        return Vec::new();
    }

    (0..session_count)
        .map(|index| restored_terminal_split_weight(weights, index))
        .collect()
}

fn restored_terminal_split_weight(weights: &[f32], index: usize) -> f32 {
    let Some(weight) = weights.get(index).copied() else {
        return 1.0;
    };
    if weight.is_finite() && weight > 0.0 {
        weight
    } else {
        1.0
    }
}

struct RestoredTerminalCwd {
    path: PathBuf,
    can_auto_start_shell: bool,
}

enum TerminalRestoreWorkspacePrefixedCwd {
    NotPrefixed,
    Candidate(PathBuf),
    Unsafe,
}

fn restored_terminal_cwd(
    cwd: Option<&Path>,
    workspace_root: &Path,
    fallback: &Path,
) -> RestoredTerminalCwd {
    let Some(cwd) = cwd else {
        return RestoredTerminalCwd {
            path: fallback.to_path_buf(),
            can_auto_start_shell: false,
        };
    };
    if super::contains_terminal_cwd_control(cwd.to_string_lossy().as_ref()) {
        return RestoredTerminalCwd {
            path: fallback.to_path_buf(),
            can_auto_start_shell: false,
        };
    }
    let workspace_root = terminal_restore_lexical_normalize_path(workspace_root);
    match terminal_restore_workspace_prefixed_relative_cwd(cwd, &workspace_root) {
        TerminalRestoreWorkspacePrefixedCwd::Candidate(resolved) => {
            if terminal_restore_cwd_is_inside_workspace(&workspace_root, &resolved)
                && resolved.is_dir()
            {
                return RestoredTerminalCwd {
                    path: resolved,
                    can_auto_start_shell: true,
                };
            }
            return RestoredTerminalCwd {
                path: fallback.to_path_buf(),
                can_auto_start_shell: false,
            };
        }
        TerminalRestoreWorkspacePrefixedCwd::Unsafe => {
            return RestoredTerminalCwd {
                path: fallback.to_path_buf(),
                can_auto_start_shell: false,
            };
        }
        TerminalRestoreWorkspacePrefixedCwd::NotPrefixed => {}
    }

    let resolved = if cwd.is_absolute() {
        if let Ok(relative) = cwd.strip_prefix(&workspace_root)
            && !terminal_restore_relative_cwd_stays_inside_workspace(relative)
        {
            return RestoredTerminalCwd {
                path: fallback.to_path_buf(),
                can_auto_start_shell: false,
            };
        }
        terminal_restore_lexical_normalize_path(cwd)
    } else {
        if !terminal_restore_relative_cwd_stays_inside_workspace(cwd) {
            return RestoredTerminalCwd {
                path: fallback.to_path_buf(),
                can_auto_start_shell: false,
            };
        }
        terminal_restore_lexical_normalize_path(&workspace_root.join(cwd))
    };
    if terminal_restore_cwd_is_inside_workspace(&workspace_root, &resolved) && resolved.is_dir() {
        RestoredTerminalCwd {
            path: resolved,
            can_auto_start_shell: true,
        }
    } else {
        RestoredTerminalCwd {
            path: fallback.to_path_buf(),
            can_auto_start_shell: false,
        }
    }
}

fn terminal_restore_cwd_is_inside_workspace(workspace_root: &Path, cwd: &Path) -> bool {
    crate::workspace_trust::workspace_path_stays_within_root_lexically(workspace_root, cwd)
        && crate::workspace_trust::workspace_path_contains_lexically(workspace_root, cwd)
}

fn terminal_restore_relative_cwd_stays_inside_workspace(cwd: &Path) -> bool {
    if cwd.is_absolute() {
        return false;
    }
    let mut depth = 0usize;
    for component in cwd.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
            }
            Component::Normal(_) => depth = depth.saturating_add(1),
            Component::Prefix(_) | Component::RootDir => return false,
        }
    }
    true
}

fn terminal_restore_workspace_prefixed_relative_cwd(
    cwd: &Path,
    workspace_root: &Path,
) -> TerminalRestoreWorkspacePrefixedCwd {
    if cwd.is_absolute() {
        return TerminalRestoreWorkspacePrefixedCwd::NotPrefixed;
    }
    let Some(workspace_name) = workspace_root.file_name() else {
        return TerminalRestoreWorkspacePrefixedCwd::NotPrefixed;
    };
    let mut components = cwd.components();
    let Some(Component::Normal(first)) = components.next() else {
        return TerminalRestoreWorkspacePrefixedCwd::NotPrefixed;
    };
    if first != workspace_name {
        return TerminalRestoreWorkspacePrefixedCwd::NotPrefixed;
    }

    let mut stripped = PathBuf::new();
    for component in components {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    stripped.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    stripped.pop();
                } else {
                    return TerminalRestoreWorkspacePrefixedCwd::Unsafe;
                }
            }
            Component::Normal(part) => stripped.push(part),
            Component::Prefix(_) | Component::RootDir => {
                return TerminalRestoreWorkspacePrefixedCwd::Unsafe;
            }
        }
    }

    TerminalRestoreWorkspacePrefixedCwd::Candidate(terminal_restore_lexical_normalize_path(
        &workspace_root.join(stripped),
    ))
}

fn terminal_restore_lexical_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut has_root = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                has_root = true;
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else if !has_root {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn restore_prepared_terminal_scrollback(session: &mut TerminalSession, scrollback: String) {
    if scrollback.is_empty() {
        return;
    }
    session.parser.process(scrollback.as_bytes());
    session.replace_search_buffer(scrollback);
}

fn restore_terminal_scrollback_offset(session: &mut TerminalSession, requested_offset: usize) {
    if requested_offset == 0 {
        session.parser.screen_mut().set_scrollback(0);
        return;
    }

    session.parser.screen_mut().set_scrollback(usize::MAX);
    let max_offset = session.parser.screen().scrollback();
    session
        .parser
        .screen_mut()
        .set_scrollback(requested_offset.min(max_offset));
}

#[cfg(test)]
fn persisted_terminal_scrollback(scrollback: &str) -> String {
    persisted_terminal_scrollback_with_limit(scrollback, MAX_PERSISTED_TERMINAL_SCROLLBACK_BYTES)
}

struct PersistedTerminalScrollback {
    text: String,
    charged_bytes: usize,
}

#[cfg(test)]
fn persisted_terminal_scrollback_with_limit(scrollback: &str, max_bytes: usize) -> String {
    persisted_terminal_scrollback_with_budget(scrollback, max_bytes).text
}

fn persisted_terminal_scrollback_with_budget(
    scrollback: &str,
    max_bytes: usize,
) -> PersistedTerminalScrollback {
    if max_bytes == 0 {
        return PersistedTerminalScrollback {
            text: String::new(),
            charged_bytes: 0,
        };
    }

    let charged_bytes = scrollback.len().min(max_bytes);
    let mut text = if scrollback.len() <= max_bytes {
        let sanitization = terminal_scrollback_sanitization(scrollback);
        if sanitization.needs_sanitizing() {
            sanitized_persisted_terminal_scrollback(scrollback, sanitization)
        } else {
            scrollback.to_owned()
        }
    } else {
        let raw_limit =
            max_bytes.saturating_add(PERSISTED_TERMINAL_SCROLLBACK_SANITIZE_LOOKBACK_BYTES);
        let raw_scrollback = bounded_persisted_terminal_scrollback_slice(scrollback, raw_limit);
        let sanitization = terminal_scrollback_sanitization(raw_scrollback);
        if sanitization.needs_sanitizing() {
            sanitized_persisted_terminal_scrollback(raw_scrollback, sanitization)
        } else {
            bounded_persisted_terminal_scrollback(raw_scrollback, max_bytes)
        }
    };

    truncate_persisted_terminal_scrollback_to_budget(&mut text, max_bytes);

    PersistedTerminalScrollback {
        charged_bytes: charged_bytes.max(text.len()).min(max_bytes),
        text,
    }
}

#[derive(Clone, Copy)]
struct TerminalScrollbackSanitization {
    needs_plain_text: bool,
    needs_bidi_filter: bool,
}

impl TerminalScrollbackSanitization {
    fn needs_sanitizing(self) -> bool {
        self.needs_plain_text || self.needs_bidi_filter
    }
}

fn terminal_scrollback_sanitization(scrollback: &str) -> TerminalScrollbackSanitization {
    let mut sanitization = TerminalScrollbackSanitization {
        needs_plain_text: false,
        needs_bidi_filter: false,
    };

    for ch in scrollback.chars() {
        if is_persisted_terminal_bidi_control(ch) {
            sanitization.needs_bidi_filter = true;
        } else if ch.is_control() && !matches!(ch, '\n' | '\t') {
            sanitization.needs_plain_text = true;
        }
    }

    sanitization
}

fn sanitized_persisted_terminal_scrollback(
    scrollback: &str,
    sanitization: TerminalScrollbackSanitization,
) -> String {
    debug_assert!(sanitization.needs_sanitizing());
    if !sanitization.needs_plain_text {
        return bidi_filtered_persisted_terminal_scrollback(scrollback);
    }

    let mut text = terminal_plain_text(scrollback.as_bytes());
    if sanitization.needs_bidi_filter {
        text.retain(|ch| !is_persisted_terminal_bidi_control(ch));
    }
    text
}

fn bidi_filtered_persisted_terminal_scrollback(scrollback: &str) -> String {
    let mut text = String::with_capacity(scrollback.len());
    for ch in scrollback.chars() {
        if !is_persisted_terminal_bidi_control(ch) {
            text.push(ch);
        }
    }
    text
}

fn bounded_persisted_terminal_scrollback(scrollback: &str, max_bytes: usize) -> String {
    bounded_persisted_terminal_scrollback_slice(scrollback, max_bytes).to_owned()
}

fn bounded_persisted_terminal_scrollback_slice(scrollback: &str, max_bytes: usize) -> &str {
    &scrollback[bounded_persisted_terminal_scrollback_start(scrollback, max_bytes)..]
}

fn truncate_persisted_terminal_scrollback_to_budget(scrollback: &mut String, max_bytes: usize) {
    let start = bounded_persisted_terminal_scrollback_start(scrollback, max_bytes);
    if start > 0 {
        drop(scrollback.drain(..start));
    }
}

fn bounded_persisted_terminal_scrollback_start(scrollback: &str, max_bytes: usize) -> usize {
    if scrollback.len() <= max_bytes {
        return 0;
    }

    let mut start = scrollback.len() - max_bytes;
    while !scrollback.is_char_boundary(start) {
        start += 1;
    }
    if start > 0
        && let Some(newline) = scrollback[start..].find('\n')
    {
        start += newline + 1;
    }
    start
}

fn normalized_persisted_terminal_label(label: Option<&str>) -> Option<String> {
    let label = label?.trim();
    if label.is_empty() {
        return None;
    }
    if is_simple_persisted_terminal_label(label) {
        return Some(bounded_ascii_persisted_terminal_label(label));
    }

    let mut normalized =
        String::with_capacity(label.len().min(PERSISTED_TERMINAL_LABEL_MAX_UTF8_BYTES));
    let mut char_count = 0usize;
    let mut truncation_marker_start = None::<usize>;
    let mut pending_separator = false;
    let mut last_output_space = false;

    for ch in label.chars() {
        if is_persisted_terminal_bidi_control(ch) {
            continue;
        }

        if is_persisted_terminal_label_separator(ch) {
            pending_separator = true;
            continue;
        }

        if pending_separator && ch == ' ' {
            continue;
        }

        if normalized.is_empty() && ch.is_whitespace() {
            pending_separator = false;
            continue;
        }

        if pending_separator && !normalized.is_empty() && !last_output_space {
            if !push_persisted_terminal_label_char(
                &mut normalized,
                ' ',
                &mut char_count,
                &mut truncation_marker_start,
            ) {
                break;
            }
        }
        pending_separator = false;

        if !push_persisted_terminal_label_char(
            &mut normalized,
            ch,
            &mut char_count,
            &mut truncation_marker_start,
        ) {
            break;
        }
        last_output_space = ch == ' ';
    }

    if normalized.is_empty() {
        None
    } else {
        trim_persisted_terminal_label_end(&mut normalized, &mut char_count);
        (!normalized.is_empty()).then_some(normalized)
    }
}

fn push_persisted_terminal_label_char(
    label: &mut String,
    ch: char,
    char_count: &mut usize,
    truncation_marker_start: &mut Option<usize>,
) -> bool {
    if *char_count == PERSISTED_TERMINAL_LABEL_MAX_CHARS {
        label.truncate(truncation_marker_start.unwrap_or(label.len()));
        label.push_str(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER);
        return false;
    }

    let prefix_chars = PERSISTED_TERMINAL_LABEL_MAX_CHARS
        .saturating_sub(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER_CHARS);
    if *char_count == prefix_chars {
        *truncation_marker_start = Some(label.len());
    }
    label.push(ch);
    *char_count += 1;
    true
}

fn trim_persisted_terminal_label_end(label: &mut String, char_count: &mut usize) {
    while label.chars().next_back().is_some_and(char::is_whitespace) {
        label.pop();
        *char_count = char_count.saturating_sub(1);
    }
}

#[cfg(test)]
fn bounded_persisted_terminal_label(label: &str) -> String {
    if label.len() <= PERSISTED_TERMINAL_LABEL_MAX_CHARS {
        return label.to_owned();
    }
    if label.is_ascii() {
        return bounded_ascii_persisted_terminal_label(label);
    }

    let prefix_chars = PERSISTED_TERMINAL_LABEL_MAX_CHARS
        .saturating_sub(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER_CHARS);
    let mut end = label.len();
    for (count, (index, _)) in label.char_indices().enumerate() {
        if count == prefix_chars {
            end = index;
        } else if count == PERSISTED_TERMINAL_LABEL_MAX_CHARS {
            let mut bounded =
                String::with_capacity(end + PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER.len());
            bounded.push_str(&label[..end]);
            bounded.push_str(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER);
            return bounded;
        }
    }
    label.to_owned()
}

fn bounded_ascii_persisted_terminal_label(label: &str) -> String {
    debug_assert!(label.is_ascii());
    if label.len() <= PERSISTED_TERMINAL_LABEL_MAX_CHARS {
        return label.to_owned();
    }

    let prefix_bytes = PERSISTED_TERMINAL_LABEL_MAX_CHARS
        .saturating_sub(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER_CHARS);
    let mut bounded =
        String::with_capacity(prefix_bytes + PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER.len());
    bounded.push_str(&label[..prefix_bytes]);
    bounded.push_str(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER);
    bounded
}

fn is_simple_persisted_terminal_label(label: &str) -> bool {
    label
        .as_bytes()
        .iter()
        .all(|byte| (b' '..=b'~').contains(byte))
}

fn is_persisted_terminal_label_separator(ch: char) -> bool {
    ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}')
}

fn is_persisted_terminal_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn restored_terminal_process_label_allows_auto_start(
    raw_label: Option<&str>,
    normalized_label: Option<&str>,
) -> bool {
    if normalized_label.is_some() {
        return false;
    }

    let Some(raw_label) = raw_label else {
        return true;
    };

    raw_label.chars().all(|ch| {
        ch.is_whitespace()
            && !is_persisted_terminal_label_separator(ch)
            && !is_persisted_terminal_bidi_control(ch)
    })
}

#[cfg(test)]
pub(super) fn restored_terminal_split_weights_for_test(
    weights: &[f32],
    session_count: usize,
) -> Vec<f32> {
    restored_terminal_split_weights(weights, session_count)
}

#[cfg(test)]
pub(super) fn persisted_terminal_scrollback_for_test(scrollback: &str) -> String {
    persisted_terminal_scrollback(scrollback)
}

#[cfg(test)]
pub(super) fn max_persisted_terminal_scrollback_total_bytes_for_test() -> usize {
    MAX_PERSISTED_TERMINAL_SCROLLBACK_TOTAL_BYTES
}

#[cfg(test)]
pub(super) fn normalized_persisted_terminal_label_for_test(label: Option<&str>) -> Option<String> {
    normalized_persisted_terminal_label(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_terminal_label_preserves_ordinary_text() {
        assert_eq!(
            normalized_persisted_terminal_label(Some("  cargo  test  ")).as_deref(),
            Some("cargo  test")
        );
    }

    #[test]
    fn persisted_terminal_label_collapses_controls_and_line_separators_and_removes_bidi() {
        assert_eq!(
            normalized_persisted_terminal_label(Some(
                "  Cargo\r\n\u{202e}Test\t\u{2066}Build\u{2028}Done\u{2029}\x1bNow  ",
            ))
            .as_deref(),
            Some("Cargo Test Build Done Now")
        );
    }

    #[test]
    fn persisted_terminal_label_returns_none_when_sanitized_blank() {
        assert_eq!(
            normalized_persisted_terminal_label(Some(
                "\u{061c}\u{200e}\u{200f}\u{202a}\u{202e}\u{2066}\u{2069}\x00\x1b\r\n\t\u{2028}\u{2029}",
            )),
            None
        );
    }

    #[test]
    fn persisted_terminal_label_does_not_keep_padding_after_leading_controls() {
        assert_eq!(
            normalized_persisted_terminal_label(Some("\x1b\t  Cargo\r\n Test")).as_deref(),
            Some("Cargo Test")
        );
    }

    #[test]
    fn persisted_terminal_label_truncates_with_ascii_marker_inside_display_budget() {
        let label = "x".repeat(PERSISTED_TERMINAL_LABEL_MAX_CHARS + 30);
        let normalized = normalized_persisted_terminal_label(Some(&label)).unwrap();

        assert_eq!(
            normalized.chars().count(),
            PERSISTED_TERMINAL_LABEL_MAX_CHARS
        );
        assert_eq!(
            normalized,
            format!(
                "{}{}",
                "x".repeat(
                    PERSISTED_TERMINAL_LABEL_MAX_CHARS
                        - PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER.len()
                ),
                PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER
            )
        );
        assert!(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER.is_ascii());
    }

    #[test]
    fn persisted_terminal_label_preserves_exact_ascii_display_budget() {
        let label = "x".repeat(PERSISTED_TERMINAL_LABEL_MAX_CHARS);
        let normalized = normalized_persisted_terminal_label(Some(&label)).unwrap();

        assert_eq!(normalized, label);
        assert!(!normalized.ends_with(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER));
    }

    #[test]
    fn bounded_persisted_terminal_label_preserves_multibyte_display_budget() {
        let label = "\u{e9}".repeat(PERSISTED_TERMINAL_LABEL_MAX_CHARS);
        let bounded = bounded_persisted_terminal_label(&label);

        assert_eq!(bounded, label);
        assert_eq!(bounded.chars().count(), PERSISTED_TERMINAL_LABEL_MAX_CHARS);
    }

    #[test]
    fn persisted_terminal_label_truncates_sanitized_text_inside_display_budget() {
        let label = format!(
            "{}\r\n{}",
            "x".repeat(PERSISTED_TERMINAL_LABEL_MAX_CHARS),
            "y".repeat(8)
        );
        let normalized = normalized_persisted_terminal_label(Some(&label)).unwrap();

        assert_eq!(
            normalized.chars().count(),
            PERSISTED_TERMINAL_LABEL_MAX_CHARS
        );
        assert!(normalized.ends_with(PERSISTED_TERMINAL_LABEL_TRUNCATION_MARKER));
        assert!(!normalized.chars().any(char::is_control));
    }

    #[test]
    fn terminal_session_snapshots_sanitize_labels_without_rewriting_raw_session_state() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        let mut session = TerminalSession::new(1, pane.last_size, pane.scrollback_rows);
        let custom_title = "  Terminal\r\n\u{202e}Main\t\u{2066}One  ".to_owned();
        let process_label = "  Cargo\r\n\u{202e}Test\t\u{2066}Build\x1bDone  ".to_owned();
        let window_title = "  Restored\r\n\u{202e}Title\t\u{2066}Done  ".to_owned();
        session.custom_title = Some(custom_title.clone());
        session.process_label = Some(process_label.clone());
        session.parser.callbacks_mut().window_title = Some(window_title.clone());
        pane.sessions.push(session);

        let snapshots = pane.terminal_session_snapshots();

        assert_eq!(
            snapshots[0].custom_title.as_deref(),
            Some("Terminal Main One")
        );
        assert_eq!(
            snapshots[0].process_label.as_deref(),
            Some("Cargo Test Build Done")
        );
        assert_eq!(
            snapshots[0].window_title.as_deref(),
            Some("Restored Title Done")
        );
        assert_eq!(
            pane.sessions[0].process_label.as_deref(),
            Some(process_label.as_str())
        );
        assert_eq!(
            pane.sessions[0].custom_title.as_deref(),
            Some(custom_title.as_str())
        );
        assert_eq!(
            pane.sessions[0].parser.callbacks().window_title.as_deref(),
            Some(window_title.as_str())
        );
    }

    #[test]
    fn terminal_persistence_snapshots_preserve_prepared_scrollback_order() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        let mut first = TerminalSession::new(1, pane.last_size, pane.scrollback_rows);
        let mut second = TerminalSession::new(2, pane.last_size, pane.scrollback_rows);
        first.replace_search_buffer("first\n".to_owned());
        second.replace_search_buffer("second\n".to_owned());
        pane.sessions.push(first);
        pane.sessions.push(second);

        let snapshots = pane.terminal_session_snapshots();

        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].scrollback, "first\n");
        assert_eq!(snapshots[1].scrollback, "second\n");
        assert_eq!(pane.sessions[0].search_buffer, "first\n");
        assert_eq!(pane.sessions[1].search_buffer, "second\n");
    }

    #[test]
    fn restored_control_only_process_label_does_not_enable_auto_start_shell() {
        assert!(!restored_terminal_process_label_allows_auto_start(
            Some("\x1b\u{202e}\u{2066}"),
            None
        ));
        assert!(restored_terminal_process_label_allows_auto_start(
            Some("   "),
            None
        ));
        assert!(restored_terminal_process_label_allows_auto_start(
            None, None
        ));
        assert!(!restored_terminal_process_label_allows_auto_start(
            Some("cargo test"),
            Some("cargo test")
        ));
    }

    #[test]
    fn restore_terminal_sessions_rejects_control_only_process_label() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        let snapshots = vec![PersistedTerminalSession {
            cwd: Some(std::path::PathBuf::from(".")),
            scrollback: "restored\n".to_owned(),
            scrollback_offset: 0,
            custom_title: None,
            process_label: Some("\x1b\u{202e}\u{2066}".to_owned()),
            process_status: None,
            window_title: Some("Restored\r\n\u{202e}Title\t\u{2066}Done".to_owned()),
        }];

        pane.restore_terminal_sessions(&snapshots, 0, false, &[1.0], true);

        assert_eq!(pane.sessions.len(), 1);
        assert!(pane.sessions[0].process_label.is_none());
        assert_eq!(
            pane.sessions[0].parser.callbacks().window_title.as_deref(),
            Some("Restored Title Done")
        );
        assert!(!pane.sessions[0].auto_start_shell);
    }

    #[test]
    fn restore_terminal_sessions_preserves_custom_title_without_other_state() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        let snapshots = vec![PersistedTerminalSession {
            cwd: Some(std::path::PathBuf::from(".")),
            scrollback: String::new(),
            scrollback_offset: 0,
            custom_title: Some("  Build\r\n\u{202e}Main  ".to_owned()),
            process_label: None,
            process_status: None,
            window_title: None,
        }];

        pane.restore_terminal_sessions(&snapshots, 0, false, &[1.0], false);

        assert_eq!(pane.sessions.len(), 1);
        assert_eq!(pane.sessions[0].custom_title.as_deref(), Some("Build Main"));
        assert!(!pane.sessions[0].auto_start_shell);
    }

    #[test]
    fn terminal_active_session_for_restore_is_zero_without_sessions() {
        let pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);

        assert_eq!(pane.terminal_active_session_for_restore(), 0);
    }

    #[test]
    fn restored_scrollback_budget_charges_sanitized_raw_bytes() {
        let escape_only = "\x1b[31m";
        let accounted = persisted_terminal_scrollback_with_budget(escape_only, escape_only.len());

        assert_eq!(accounted.text, "");
        assert_eq!(accounted.charged_bytes, escape_only.len());

        let snapshots = vec![
            PersistedTerminalSession {
                cwd: Some(std::path::PathBuf::from(".")),
                scrollback: escape_only.to_owned(),
                scrollback_offset: 0,
                custom_title: None,
                process_label: None,
                process_status: None,
                window_title: None,
            },
            PersistedTerminalSession {
                cwd: Some(std::path::PathBuf::from(".")),
                scrollback: "visible\n".to_owned(),
                scrollback_offset: 0,
                custom_title: None,
                process_label: None,
                process_status: None,
                window_title: None,
            },
        ];

        let scrollbacks =
            restored_terminal_scrollback_by_snapshot(&snapshots, &[0, 1], 0, escape_only.len());

        assert_eq!(scrollbacks, vec![String::new(), String::new()]);
    }

    #[test]
    fn restore_terminal_sessions_skips_inert_invalid_snapshots_and_remaps_layout() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        let snapshots = vec![
            PersistedTerminalSession {
                cwd: None,
                scrollback: "\x1b[31m".to_owned(),
                scrollback_offset: 12,
                custom_title: None,
                process_label: None,
                process_status: None,
                window_title: None,
            },
            PersistedTerminalSession {
                cwd: None,
                scrollback: "kept\n".to_owned(),
                scrollback_offset: 0,
                custom_title: None,
                process_label: None,
                process_status: None,
                window_title: None,
            },
        ];

        pane.restore_terminal_sessions(&snapshots, 0, true, &[0.25, 0.75], true);

        assert_eq!(pane.sessions.len(), 1);
        assert_eq!(pane.active_session, 0);
        assert!(!pane.split_view);
        assert_eq!(pane.split_weights, vec![0.75]);
        assert_eq!(pane.sessions[0].search_buffer, "kept\n");
        assert!(!pane.sessions[0].auto_start_shell);
    }

    #[test]
    fn restore_terminal_sessions_keeps_empty_valid_auto_start_snapshot() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        let snapshots = vec![PersistedTerminalSession {
            cwd: Some(std::path::PathBuf::from(".")),
            scrollback: String::new(),
            scrollback_offset: 0,
            custom_title: None,
            process_label: None,
            process_status: None,
            window_title: None,
        }];

        pane.restore_terminal_sessions(&snapshots, 0, false, &[1.0], true);

        assert_eq!(pane.sessions.len(), 1);
        assert!(pane.sessions[0].auto_start_shell);
    }

    #[test]
    fn restore_terminal_sessions_clamps_stale_scrollback_offset() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        pane.last_size = portable_pty::PtySize {
            rows: 3,
            cols: 20,
            pixel_width: 0,
            pixel_height: 0,
        };
        let snapshots = vec![PersistedTerminalSession {
            cwd: Some(std::path::PathBuf::from(".")),
            scrollback: "line0\nline1\nline2\nline3\nline4\n".to_owned(),
            scrollback_offset: usize::MAX,
            custom_title: None,
            process_label: None,
            process_status: None,
            window_title: None,
        }];

        pane.restore_terminal_sessions(&snapshots, 0, false, &[1.0], true);

        let restored_offset = pane.sessions[0].scrollback();
        pane.sessions[0]
            .parser
            .screen_mut()
            .set_scrollback(usize::MAX);
        let max_offset = pane.sessions[0].scrollback();
        assert_eq!(restored_offset, max_offset);
    }

    #[test]
    fn persisted_terminal_scrollback_sanitizes_control_sequences() {
        let scrollback = persisted_terminal_scrollback_with_limit("first\x1b[31mred\x1b[0m\n", 128);

        assert_eq!(scrollback, "firstred\n");
    }

    #[test]
    fn persisted_terminal_scrollback_sanitizes_bidi_controls() {
        let scrollback = persisted_terminal_scrollback_with_limit(
            "first\u{202e}spoof\u{2066}\nsecond\u{200f}\tkeep\n",
            128,
        );

        assert_eq!(scrollback, "firstspoof\nsecond\tkeep\n");
        assert!(!scrollback.chars().any(is_persisted_terminal_bidi_control));
    }

    #[test]
    fn persisted_terminal_scrollback_sanitizes_controls_and_bidi_controls() {
        let scrollback = persisted_terminal_scrollback_with_limit(
            "first\x1b[31mred\u{202e}spoof\x1b[0m\nsecond\u{2066}keep\n",
            128,
        );

        assert_eq!(scrollback, "firstredspoof\nsecondkeep\n");
        assert!(!scrollback.chars().any(is_persisted_terminal_bidi_control));
    }

    #[test]
    fn restore_terminal_sessions_sanitizes_bidi_scrollback() {
        let mut pane = TerminalPane::new(std::path::PathBuf::from("."), 100, 12.0, 1.2);
        let snapshots = vec![PersistedTerminalSession {
            cwd: Some(std::path::PathBuf::from(".")),
            scrollback: "restored\u{202e}spoof\nsafe\u{2066}text\n".to_owned(),
            scrollback_offset: 0,
            custom_title: None,
            process_label: None,
            process_status: None,
            window_title: None,
        }];

        pane.restore_terminal_sessions(&snapshots, 0, false, &[1.0], true);

        assert_eq!(pane.sessions[0].search_buffer, "restoredspoof\nsafetext\n");
        assert!(
            !pane.sessions[0]
                .search_buffer
                .chars()
                .any(is_persisted_terminal_bidi_control)
        );
    }
}
