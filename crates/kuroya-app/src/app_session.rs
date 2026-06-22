use crate::{
    KuroyaApp,
    buffer_find_history::{MAX_BUFFER_FIND_HISTORY, buffer_find_history_enabled},
    command_palette_items::{
        MAX_COMMAND_PALETTE_QUERY_MEMORY, MAX_COMMAND_PALETTE_RECENT_COMMANDS,
        normalize_command_palette_query_memory,
    },
    folding::session_fold_states,
    lsp_workspace_symbol_ranking::{
        MAX_WORKSPACE_SYMBOL_QUERY_MEMORY, normalize_workspace_symbol_query_memory,
    },
    persistence::{
        AppState, PersistedClosedFileEntry, PersistedNavigationLocation, PersistedSession,
        PersistedSourceControlSortMode, PersistedSourceControlViewMode,
    },
    persistence_session::normalize_persisted_session_paths_for_restore,
    project_search_state::MAX_PROJECT_SEARCH_RECENT_QUERIES,
    quick_open::{MAX_QUICK_OPEN_QUERY_MEMORY, MAX_QUICK_OPEN_RECENT_FILES},
    recovery::{
        RECOVERY_BUFFER_MAX_BYTES, RECOVERY_SESSION_MAX_BYTES, RecoverySnapshotDraft,
        recovery_snapshot_draft_for_buffers, recovery_snapshot_for_buffers,
    },
    session_state::{
        editor_row_height, merged_recent_projects, recent_projects_with_recorded,
        session_history_states, session_pane_view_states, session_recovery_history_states,
        session_recovery_view_states, session_view_states,
    },
    source_control_panel::{
        SOURCE_CONTROL_COMMIT_HISTORY_LIMIT, SourceControlSortMode, SourceControlViewMode,
        normalize_source_control_commit_history,
    },
    workspace_trust::{trusted_workspace_paths_match, workspace_path_stays_within_root_lexically},
};
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

const MAX_SESSION_EXPLORER_EXPANDED_PATHS: usize = 512;

#[derive(Debug, Clone)]
pub(crate) struct SessionSaveSnapshot {
    session: PersistedSession,
    recovery: RecoverySnapshotDraft,
}

impl SessionSaveSnapshot {
    pub(crate) fn into_persisted_session(mut self) -> PersistedSession {
        let recovery = self.recovery.into_recovery_snapshot();
        self.session.recovery = recovery.recovered;
        self.session.recovery_skipped = recovery.skipped;
        let workspace_root = self.session.workspace_root.clone();
        normalize_persisted_session_paths_for_restore(&workspace_root, &mut self.session);
        self.session
    }
}

pub(crate) fn persisted_explorer_expanded_paths(
    root: &Path,
    expanded: &HashSet<PathBuf>,
) -> Vec<PathBuf> {
    let mut paths = expanded
        .iter()
        .filter_map(|path| workspace_descendant_path_for_session(root, path))
        .collect::<Vec<_>>();
    paths.sort();
    dedupe_session_paths_lexically(&mut paths);
    paths.truncate(MAX_SESSION_EXPLORER_EXPANDED_PATHS);
    paths
}

pub(crate) fn restored_explorer_expanded_paths(
    root: &Path,
    expanded: impl IntoIterator<Item = PathBuf>,
) -> HashSet<PathBuf> {
    let expanded = expanded.into_iter();
    let (lower_bound, _) = expanded.size_hint();
    let mut paths = HashSet::with_capacity(lower_bound);
    let mut seen = HashSet::with_capacity(lower_bound);
    for path in expanded {
        let Some(path) = workspace_descendant_path_for_session(root, &path) else {
            continue;
        };
        if seen.insert(session_path_dedupe_key(&path)) {
            paths.insert(path);
        }
    }
    paths
}

pub(crate) fn workspace_descendant_path_for_session(root: &Path, path: &Path) -> Option<PathBuf> {
    if !workspace_path_stays_within_root_lexically(root, path)
        || trusted_workspace_paths_match(root, path)
    {
        return None;
    }
    lexically_normalize_session_path(path)
}

fn dedupe_session_paths_lexically(paths: &mut Vec<PathBuf>) {
    let mut seen = HashSet::with_capacity(paths.len());
    paths.retain(|path| seen.insert(session_path_dedupe_key(path)));
}

fn collect_session_vec<T>(capacity: usize, items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut collected = Vec::with_capacity(capacity);
    collected.extend(items);
    collected
}

fn collect_capped_session_vec<T>(
    source_len: usize,
    max_len: usize,
    items: impl IntoIterator<Item = T>,
) -> Vec<T> {
    collect_session_vec(source_len.min(max_len), items.into_iter().take(max_len))
}

fn collect_session_vec_deque<T>(items: std::collections::VecDeque<T>) -> Vec<T> {
    collect_session_vec(items.len(), items)
}

fn lexically_normalize_session_path(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(component) => normalized.push(component),
        }
    }
    Some(normalized)
}

#[cfg(windows)]
pub(crate) fn session_path_dedupe_key(path: &Path) -> PathBuf {
    let key = path.as_os_str().to_string_lossy();
    if key.is_ascii() {
        let mut key = key.into_owned();
        key.make_ascii_lowercase();
        PathBuf::from(key)
    } else {
        PathBuf::from(key.to_lowercase())
    }
}

#[cfg(not(windows))]
pub(crate) fn session_path_dedupe_key(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub(crate) fn persisted_source_control_view_mode(
    mode: SourceControlViewMode,
) -> PersistedSourceControlViewMode {
    match mode {
        SourceControlViewMode::List => PersistedSourceControlViewMode::List,
        SourceControlViewMode::Tree => PersistedSourceControlViewMode::Tree,
    }
}

pub(crate) fn source_control_view_mode_from_persisted(
    mode: PersistedSourceControlViewMode,
) -> SourceControlViewMode {
    match mode {
        PersistedSourceControlViewMode::List => SourceControlViewMode::List,
        PersistedSourceControlViewMode::Tree => SourceControlViewMode::Tree,
    }
}

pub(crate) fn persisted_source_control_sort_mode(
    mode: SourceControlSortMode,
) -> PersistedSourceControlSortMode {
    match mode {
        SourceControlSortMode::Path => PersistedSourceControlSortMode::Path,
        SourceControlSortMode::Name => PersistedSourceControlSortMode::Name,
        SourceControlSortMode::Status => PersistedSourceControlSortMode::Status,
    }
}

pub(crate) fn source_control_sort_mode_from_persisted(
    mode: PersistedSourceControlSortMode,
) -> SourceControlSortMode {
    match mode {
        PersistedSourceControlSortMode::Path => SourceControlSortMode::Path,
        PersistedSourceControlSortMode::Name => SourceControlSortMode::Name,
        PersistedSourceControlSortMode::Status => SourceControlSortMode::Status,
    }
}

impl KuroyaApp {
    pub(crate) fn record_recent_project(&mut self, root: PathBuf) {
        self.recent_projects = recent_projects_with_recorded(&self.recent_projects, root);
    }

    pub(crate) fn merge_recent_projects(&mut self, projects: Vec<PathBuf>) {
        self.recent_projects = merged_recent_projects(&self.recent_projects, &projects);
    }

    pub(crate) fn save_app_state(&self) -> anyhow::Result<()> {
        let app_state = AppState {
            recent_projects: self.recent_projects.clone(),
            trusted_workspaces: self.trusted_workspaces.clone(),
            vim_keybindings: Some(self.app_state_vim_keybindings),
            vim: Some(self.app_state_vim.clone()),
        };
        #[cfg(test)]
        if let Some(path) = &self.app_state_path_override {
            return app_state.save_to_path(path);
        }
        app_state.save()
    }

    pub(crate) fn build_session(&self) -> PersistedSession {
        let mut session = self.build_session_without_recovery();
        self.populate_recovery_session_state(&mut session);
        let recovery_snapshot = recovery_snapshot_for_buffers(
            &self.buffers,
            RECOVERY_BUFFER_MAX_BYTES,
            RECOVERY_SESSION_MAX_BYTES,
        );
        session.recovery = recovery_snapshot.recovered;
        session.recovery_skipped = recovery_snapshot.skipped;
        normalize_persisted_session_paths_for_restore(&self.workspace.root, &mut session);
        session
    }

    pub(crate) fn build_session_save_snapshot(&self) -> SessionSaveSnapshot {
        let mut session = self.build_session_without_recovery();
        self.populate_recovery_session_state(&mut session);
        SessionSaveSnapshot {
            session,
            recovery: recovery_snapshot_draft_for_buffers(
                &self.buffers,
                RECOVERY_BUFFER_MAX_BYTES,
                RECOVERY_SESSION_MAX_BYTES,
            ),
        }
    }

    fn populate_recovery_session_state(&self, session: &mut PersistedSession) {
        let row_height = editor_row_height(self.settings.font_size, self.settings.line_height);
        session.recovery_view_states = session_recovery_view_states(
            &self.buffers,
            &self.panes,
            &self.editor_scroll_offsets,
            &self.editor_horizontal_scroll_offsets,
            self.active_pane,
            row_height,
            RECOVERY_BUFFER_MAX_BYTES,
            RECOVERY_SESSION_MAX_BYTES,
        );
        session.recovery_history_states = session_recovery_history_states(
            &self.buffers,
            self.active,
            RECOVERY_BUFFER_MAX_BYTES,
            RECOVERY_SESSION_MAX_BYTES,
        );
    }

    fn build_session_without_recovery(&self) -> PersistedSession {
        let mut open_files = Vec::with_capacity(self.buffers.len());
        let mut active_path = None;
        for buffer in &self.buffers {
            let Some(path) = buffer.path() else {
                continue;
            };
            if self.active == Some(buffer.id()) {
                active_path = Some(path.clone());
            }
            open_files.push(path.clone());
        }
        let pane_paths = collect_session_vec(
            self.panes.len(),
            self.panes.iter().map(|pane| {
                pane.active
                    .and_then(|id| self.buffer(id))
                    .and_then(|buffer| buffer.path().cloned())
            }),
        );
        let pane_weights =
            collect_session_vec(self.panes.len(), self.panes.iter().map(|pane| pane.weight));
        let active_pane_index = self
            .panes
            .iter()
            .position(|pane| pane.id == self.active_pane);
        let row_height = editor_row_height(self.settings.font_size, self.settings.line_height);
        let view_states = session_view_states(
            &self.buffers,
            &self.panes,
            &self.editor_scroll_offsets,
            &self.editor_horizontal_scroll_offsets,
            self.active_pane,
            row_height,
        );
        let pane_view_states = session_pane_view_states(
            &self.buffers,
            &self.panes,
            &self.editor_scroll_offsets,
            &self.editor_horizontal_scroll_offsets,
            row_height,
        );
        let fold_states = session_fold_states(&self.folded_ranges);
        let history_states = session_history_states(&self.buffers, self.active);

        PersistedSession {
            workspace_root: self.workspace.root.clone(),
            open_files,
            active_path,
            pane_paths,
            pane_weights,
            active_pane_index,
            view_states,
            pane_view_states,
            history_states,
            recovery_view_states: Vec::new(),
            recovery_history_states: Vec::new(),
            fold_states,
            explorer_width: self.explorer_width,
            explorer_expanded: persisted_explorer_expanded_paths(
                &self.workspace.root,
                &self.explorer_expanded,
            ),
            explorer_revealed_path: self
                .explorer_revealed_path
                .as_ref()
                .and_then(|path| workspace_descendant_path_for_session(&self.workspace.root, path)),
            project_search_open: self.project_search,
            project_search_placement: self.project_search_placement,
            project_search_width: self.project_search_width,
            project_search_query: self.project_search_query.clone(),
            project_search_case_sensitive: self.project_search_case_sensitive,
            project_search_whole_word: self.project_search_whole_word,
            project_search_include: self.project_search_include.clone(),
            project_search_exclude: self.project_search_exclude.clone(),
            project_search_recent: collect_capped_session_vec(
                self.project_search_recent.len(),
                MAX_PROJECT_SEARCH_RECENT_QUERIES,
                self.project_search_recent.iter().cloned(),
            ),
            buffer_find_open: self.buffer_find_open,
            buffer_find_query: self.buffer_find_query.clone(),
            buffer_find_replacement: self.buffer_find_replacement.clone(),
            buffer_find_case_sensitive: self.buffer_find_case_sensitive,
            buffer_find_whole_word: self.buffer_find_whole_word,
            buffer_find_regex: self.buffer_find_regex,
            buffer_find_preserve_case: self.buffer_find_preserve_case,
            buffer_find_query_history: if buffer_find_history_enabled(self.settings.find_history) {
                collect_capped_session_vec(
                    self.buffer_find_query_history.len(),
                    MAX_BUFFER_FIND_HISTORY,
                    self.buffer_find_query_history.iter().cloned(),
                )
            } else {
                Vec::new()
            },
            buffer_find_replacement_history: if buffer_find_history_enabled(
                self.settings.find_replace_history,
            ) {
                collect_capped_session_vec(
                    self.buffer_find_replacement_history.len(),
                    MAX_BUFFER_FIND_HISTORY,
                    self.buffer_find_replacement_history.iter().cloned(),
                )
            } else {
                Vec::new()
            },
            settings_panel_open: self.settings_panel_open,
            theme_picker_open: self.theme_picker_open,
            keybindings_open: self.keybindings_open,
            symbols_panel_open: self.symbols_panel,
            symbols_panel_placement: self.symbols_panel_placement,
            symbols_panel_width: self.symbols_panel_width,
            diagnostics_panel_open: self.diagnostics_panel,
            diagnostics_panel_placement: self.diagnostics_panel_placement,
            diagnostics_panel_width: self.diagnostics_panel_width,
            source_control_open: self.source_control,
            source_control_placement: self.source_control_placement,
            source_control_width: self.source_control_width,
            source_control_query: self.source_control_query.clone(),
            source_control_view: persisted_source_control_view_mode(self.source_control_view),
            source_control_sort: persisted_source_control_sort_mode(self.source_control_sort),
            source_control_commit_message: self.source_control_commit_message.clone(),
            source_control_commit_history: normalize_source_control_commit_history(
                self.source_control_commit_history.clone(),
                SOURCE_CONTROL_COMMIT_HISTORY_LIMIT,
            ),
            source_control_stash_message: self.source_control_stash_message.clone(),
            source_control_stashes_open: self.source_control_stashes_open,
            source_control_history_open: self.source_control_history_open,
            source_control_history_query: self.source_control_history_query.clone(),
            source_control_unstaged_collapsed: self.source_control_unstaged_collapsed,
            source_control_untracked_collapsed: self.source_control_untracked_collapsed,
            source_control_staged_collapsed: self.source_control_staged_collapsed,
            terminal_visible: self.terminal.visible,
            terminal_height: self.terminal_height,
            terminal_sessions: self.terminal.terminal_session_snapshots(),
            terminal_active_session: self.terminal.terminal_active_session_for_restore(),
            terminal_split_view: self.terminal.terminal_split_view_for_restore(),
            terminal_split_weights: self.terminal.terminal_split_weights_for_restore(),
            recent_projects: self.recent_projects.clone(),
            quick_open_recent_files: collect_capped_session_vec(
                self.quick_open_recent_files.len(),
                MAX_QUICK_OPEN_RECENT_FILES,
                self.quick_open_recent_files.iter().cloned(),
            ),
            quick_open_query_memory: collect_capped_session_vec(
                self.quick_open_query_memory.len(),
                MAX_QUICK_OPEN_QUERY_MEMORY,
                self.quick_open_query_memory.iter().cloned(),
            ),
            workspace_symbol_query_memory: collect_session_vec_deque(
                normalize_workspace_symbol_query_memory(
                    self.workspace_symbol_query_memory.iter().cloned(),
                    &self.workspace.root,
                    MAX_WORKSPACE_SYMBOL_QUERY_MEMORY,
                ),
            ),
            command_recent: collect_capped_session_vec(
                self.command_recent.len(),
                MAX_COMMAND_PALETTE_RECENT_COMMANDS,
                self.command_recent.iter().cloned(),
            ),
            command_query_memory: collect_session_vec_deque(
                normalize_command_palette_query_memory(
                    self.command_query_memory.iter().cloned(),
                    MAX_COMMAND_PALETTE_QUERY_MEMORY,
                ),
            ),
            navigation_back: collect_session_vec(
                self.navigation_back.len(),
                self.navigation_back
                    .iter()
                    .map(PersistedNavigationLocation::from_navigation_location),
            ),
            navigation_forward: collect_session_vec(
                self.navigation_forward.len(),
                self.navigation_forward
                    .iter()
                    .map(PersistedNavigationLocation::from_navigation_location),
            ),
            closed_files: collect_session_vec(
                self.closed_files.len(),
                self.closed_files
                    .iter()
                    .map(PersistedClosedFileEntry::from_closed_file_entry),
            ),
            recovery: Vec::new(),
            recovery_skipped: Vec::new(),
        }
    }
}
