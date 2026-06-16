use crate::{
    KuroyaApp,
    devtools_async_tasks::git_scan_task_detail,
    path_display::compact_path,
    project_index_cache::{load_project_index_cache_unverified, save_project_index_cache},
    syntax::PluginSyntaxLoad,
    theme::selected_theme_index_with_plugins,
    ui_events::UiEvent,
};
use kuroya_core::{
    GitAutoRepositoryDetection, GitOpenRepositoryInParentFolders, GitSnapshot, ProjectIndex,
    ProjectSearchIndex, SearchOptions, discover_workspace_plugins,
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(crate) const WORKSPACE_PLUGIN_RELOAD_DEBOUNCE: Duration = Duration::from_millis(250);
pub(crate) const WORKSPACE_REFRESH_DEBOUNCE: Duration = Duration::from_millis(250);
pub(crate) const WORKSPACE_REFRESH_MAX_WAIT: Duration = Duration::from_secs(2);
const GIT_REPOSITORY_SCAN_MAX_CHILDREN_PER_FOLDER: usize = 2_048;
const GIT_REPOSITORY_SCAN_MAX_VISITED_FOLDERS: usize = 10_000;
const PROJECT_INDEX_MAX_FILES: usize = 40_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PendingWorkspaceRefresh {
    first_seen: Instant,
    last_seen: Instant,
}

impl PendingWorkspaceRefresh {
    fn new(now: Instant) -> Self {
        Self {
            first_seen: now,
            last_seen: now,
        }
    }

    fn record_change(&mut self, now: Instant) {
        self.last_seen = now;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitScanRootCacheEntry {
    key: GitScanRootCacheKey,
    scan_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitScanRootCacheKey {
    workspace_root: PathBuf,
    mode: GitAutoRepositoryDetection,
    repository_scan_max_depth: usize,
    ignored_folders: Vec<String>,
    open_editor_paths: Vec<PathBuf>,
}

impl KuroyaApp {
    pub(crate) fn spawn_index(&mut self) {
        self.clear_explorer_directory_cache();
        if self.workspace_placeholder {
            self.invalidate_workspace_index_requests();
            self.status = "No folder open".to_owned();
            return;
        }
        let Some(request_id) = self.begin_workspace_index_request() else {
            return;
        };
        let root = self.workspace.root.clone();
        let tx = self.tx.clone();
        let allow_cached_preview = self.index.files().is_empty();
        self.record_async_task_started("Index Workspace", compact_path(&root));
        self.runtime.spawn_blocking(move || {
            if let Some(cache) = load_project_index_cache_unverified(&root, PROJECT_INDEX_MAX_FILES)
            {
                if allow_cached_preview {
                    let _ = crate::ui_event_channel::send_critical_ui_event(
                        &tx,
                        UiEvent::CachedIndex {
                            request_id,
                            root: root.clone(),
                            index: cache.index.clone(),
                        },
                    );
                }
                let fresh_signature = ProjectIndex::scan_signature(&root, PROJECT_INDEX_MAX_FILES);
                if cache.signature == fresh_signature {
                    let index = cache.index;
                    let _ = crate::ui_event_channel::send_critical_ui_event(
                        &tx,
                        UiEvent::Indexed {
                            request_id,
                            root: root.clone(),
                            index: index.clone(),
                            search_index: ProjectSearchIndex::default(),
                        },
                    );
                    send_project_search_indexed(&tx, request_id, root, &index);
                    return;
                }
            }
            let (index, signature) =
                ProjectIndex::rebuild_with_signature(&root, PROJECT_INDEX_MAX_FILES);
            let _ = save_project_index_cache(&root, &index, signature);
            let _ = crate::ui_event_channel::send_critical_ui_event(
                &tx,
                UiEvent::Indexed {
                    request_id,
                    root: root.clone(),
                    index: index.clone(),
                    search_index: ProjectSearchIndex::default(),
                },
            );
            send_project_search_indexed(&tx, request_id, root, &index);
        });
    }

    pub(crate) fn spawn_git_scan(&mut self) -> bool {
        if self.workspace_placeholder {
            self.invalidate_git_scan();
            self.status = "No folder open".to_owned();
            return false;
        }
        if !self.settings.git_enabled {
            self.invalidate_git_scan();
            return false;
        }
        if matches!(
            self.settings.git_auto_repository_detection,
            GitAutoRepositoryDetection::False
        ) {
            self.invalidate_git_scan();
            return false;
        }

        let Some(request_id) = self.begin_git_scan_request() else {
            return false;
        };
        let root = self.workspace.root.clone();
        let open_editor_paths = self
            .buffers
            .iter()
            .filter_map(|buffer| buffer.path().cloned())
            .collect::<Vec<_>>();
        let root_cache_entry = self.git_scan_root_cache.clone();
        let ignored_repositories = self.settings.git_ignored_repositories.clone();
        let auto_repository_detection = self.settings.git_auto_repository_detection;
        let repository_scan_max_depth = self.settings.git_repository_scan_max_depth;
        let repository_scan_ignored_folders =
            self.settings.git_repository_scan_ignored_folders.clone();
        let tx = self.tx.clone();
        let status_limit = self.settings.git_status_limit;
        let ignore_submodules = self.settings.git_ignore_submodules;
        let detect_submodules = self.settings.git_detect_submodules;
        let detect_submodules_limit = self.settings.git_detect_submodules_limit;
        let similarity_threshold = self.settings.git_similarity_threshold;
        let open_parent_repository_mode = self.settings.git_open_repository_in_parent_folders;
        self.record_async_task_started("Git Scan", git_scan_task_detail(request_id, &root));
        self.runtime.spawn_blocking(move || {
            let mut scan_root = None;
            let mut next_root_cache_entry = None;
            let mut git = GitSnapshot::default();
            if !git_repository_ignored(&root, &ignored_repositories) {
                let (resolved_scan_root, resolved_root_cache_entry) =
                    resolved_cached_git_scan_root_for_auto_repository_detection(
                        root_cache_entry,
                        &root,
                        auto_repository_detection,
                        repository_scan_max_depth,
                        &repository_scan_ignored_folders,
                        &open_editor_paths,
                    );
                next_root_cache_entry = resolved_root_cache_entry;
                if let Some(resolved_scan_root) = resolved_scan_root {
                    let open_parent_repositories =
                        git_open_parent_repositories(open_parent_repository_mode)
                            && !git_repository_scan_folder_ignored(
                                &root,
                                &repository_scan_ignored_folders,
                            );
                    git = GitSnapshot::scan_with_status_options_and_parent_policy(
                        &resolved_scan_root,
                        status_limit,
                        ignore_submodules,
                        detect_submodules,
                        detect_submodules_limit,
                        similarity_threshold,
                        open_parent_repositories,
                    );
                    scan_root = Some(resolved_scan_root);
                }
            }
            let _ = crate::ui_event_channel::send_critical_ui_event(
                &tx,
                UiEvent::GitScanned {
                    request_id,
                    root,
                    scan_root,
                    root_cache_entry: next_root_cache_entry,
                    git,
                },
            );
        });
        true
    }

    pub(crate) fn spawn_git_auto_refresh(&mut self) -> bool {
        if !git_auto_refresh_enabled(self.settings.git_enabled, self.settings.git_autorefresh) {
            return false;
        }
        self.spawn_git_scan()
    }

    pub(crate) fn schedule_workspace_refresh(&mut self) {
        let now = Instant::now();
        if let Some(pending) = &mut self.pending_workspace_refresh {
            pending.record_change(now);
        } else {
            self.pending_workspace_refresh = Some(PendingWorkspaceRefresh::new(now));
        }
    }

    pub(crate) fn flush_pending_workspace_refresh(&mut self) -> usize {
        if !workspace_refresh_due(
            self.pending_workspace_refresh,
            Instant::now(),
            WORKSPACE_REFRESH_DEBOUNCE,
            WORKSPACE_REFRESH_MAX_WAIT,
        ) {
            return 0;
        }

        self.pending_workspace_refresh = None;
        self.spawn_index();
        1 + usize::from(self.spawn_git_auto_refresh())
    }

    fn begin_workspace_index_request(&mut self) -> Option<u64> {
        begin_workspace_index_request_state(
            &mut self.workspace_index_next_request_id,
            &mut self.workspace_index_active_request_id,
            &mut self.workspace_index_in_flight_request_id,
            &mut self.workspace_index_refresh_queued,
        )
    }

    pub(crate) fn finish_workspace_index_request(&mut self, request_id: u64) -> bool {
        finish_workspace_index_request_state(
            &mut self.workspace_index_in_flight_request_id,
            &mut self.workspace_index_refresh_queued,
            request_id,
        )
    }

    pub(crate) fn invalidate_workspace_index_requests(&mut self) {
        invalidate_workspace_index_request_state(
            &mut self.workspace_index_next_request_id,
            &mut self.workspace_index_active_request_id,
            &mut self.workspace_index_in_flight_request_id,
            &mut self.workspace_index_refresh_queued,
        );
    }

    fn begin_git_scan_request(&mut self) -> Option<u64> {
        begin_git_scan_request_state(
            &mut self.git_scan_next_request_id,
            &mut self.git_scan_active_request_id,
            &mut self.git_scan_in_flight_request_id,
            &mut self.git_scan_refresh_queued,
        )
    }

    pub(crate) fn finish_git_scan_request(&mut self, request_id: u64) -> bool {
        finish_git_scan_request_state(
            &mut self.git_scan_in_flight_request_id,
            &mut self.git_scan_refresh_queued,
            request_id,
        )
    }

    pub(crate) fn invalidate_git_scan_requests(&mut self) {
        invalidate_git_scan_request_state(
            &mut self.git_scan_next_request_id,
            &mut self.git_scan_active_request_id,
            &mut self.git_scan_in_flight_request_id,
            &mut self.git_scan_refresh_queued,
        );
    }

    pub(crate) fn invalidate_git_scan(&mut self) {
        self.invalidate_git_scan_requests();
        self.invalidate_virtual_source_control_open_requests();
        self.git_scan_root_cache = None;
        self.git = GitSnapshot::default();
        self.source_control_selected = 0;
    }

    pub(crate) fn sync_git_enabled_state(&mut self) {
        if self.settings.git_enabled {
            self.spawn_git_scan();
        } else {
            self.invalidate_git_scan();
            self.status = "Git is disabled".to_owned();
        }
    }

    pub(crate) fn sync_git_repository_filters_state(&mut self) {
        if !self.settings.git_enabled {
            self.invalidate_git_scan();
        } else if git_repository_ignored(
            &self.workspace.root,
            &self.settings.git_ignored_repositories,
        ) {
            self.invalidate_git_scan();
            self.status = "Git repository ignored by git.ignoredRepositories".to_owned();
        } else {
            self.spawn_git_scan();
        }
    }

    pub(crate) fn sync_git_autorefresh_state(&mut self, previous_git_autorefresh: bool) {
        if !previous_git_autorefresh && self.settings.git_autorefresh && self.settings.git_enabled {
            self.spawn_git_scan();
        }
    }

    pub(crate) fn spawn_plugin_discovery(&mut self) -> bool {
        self.pending_workspace_plugin_reload = None;
        if self.workspace_placeholder {
            self.invalidate_workspace_plugin_discovery();
            self.clear_workspace_plugins();
            self.status = "No folder open".to_owned();
            return false;
        }
        if !workspace_plugins_enabled(self.workspace_trusted) {
            self.invalidate_workspace_plugin_discovery();
            self.clear_workspace_plugins();
            self.status = workspace_plugins_restricted_status().to_owned();
            return false;
        }

        let Some(request_id) = self.begin_workspace_plugin_discovery_request() else {
            return false;
        };
        let root = self.workspace.root.clone();
        let tx = self.tx.clone();
        self.record_async_task_started("Workspace Plugins", compact_path(&root));
        self.runtime.spawn_blocking(move || {
            let event = match discover_workspace_plugins(&root) {
                Ok(mut discovery) => {
                    let syntax_load = PluginSyntaxLoad::from_plugins(&discovery.plugins);
                    discovery.errors.extend(syntax_load.errors.clone());
                    UiEvent::WorkspacePluginsLoaded {
                        request_id,
                        root,
                        plugins: discovery.plugins,
                        errors: discovery.errors,
                        syntax_load,
                    }
                }
                Err(error) => UiEvent::WorkspacePluginsFailed {
                    request_id,
                    root,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_critical_ui_event(&tx, event);
        });
        true
    }

    pub(crate) fn schedule_workspace_plugin_reload(&mut self) {
        self.pending_workspace_plugin_reload = Some(Instant::now());
    }

    pub(crate) fn flush_pending_workspace_plugin_reload(&mut self) -> usize {
        if !workspace_plugin_reload_due(
            self.pending_workspace_plugin_reload,
            Instant::now(),
            WORKSPACE_PLUGIN_RELOAD_DEBOUNCE,
        ) {
            return 0;
        }

        usize::from(self.spawn_plugin_discovery())
    }

    fn begin_workspace_plugin_discovery_request(&mut self) -> Option<u64> {
        begin_workspace_plugin_discovery_request_state(
            &mut self.workspace_plugins_next_request_id,
            &mut self.workspace_plugins_active_request_id,
            &mut self.workspace_plugins_in_flight_request_id,
            &mut self.workspace_plugins_reload_queued,
        )
    }

    pub(crate) fn finish_workspace_plugin_discovery_request(&mut self, request_id: u64) -> bool {
        finish_workspace_plugin_discovery_request_state(
            &mut self.workspace_plugins_in_flight_request_id,
            &mut self.workspace_plugins_reload_queued,
            request_id,
        )
    }

    pub(crate) fn invalidate_workspace_plugin_discovery_requests(&mut self) {
        invalidate_workspace_plugin_discovery_request_state(
            &mut self.workspace_plugins_next_request_id,
            &mut self.workspace_plugins_active_request_id,
            &mut self.workspace_plugins_in_flight_request_id,
            &mut self.workspace_plugins_reload_queued,
        );
    }

    pub(crate) fn invalidate_workspace_plugin_discovery(&mut self) {
        self.invalidate_workspace_plugin_discovery_requests();
    }

    pub(crate) fn clear_workspace_plugins(&mut self) {
        self.plugins.clear();
        self.plugin_errors.clear();
        self.plugin_runtimes = Default::default();
        self.plugin_activations = Default::default();
        self.plugin_commands = Default::default();
        self.plugin_languages = Default::default();
        self.plugin_themes = Default::default();
        self.plugin_syntaxes = Default::default();
        self.highlighter.reset_plugin_syntaxes();
        self.theme_picker_selected =
            selected_theme_index_with_plugins(&self.settings.theme, &self.plugin_themes);
    }
}

fn send_project_search_indexed(
    tx: &crate::ui_event_channel::Sender<UiEvent>,
    request_id: u64,
    root: PathBuf,
    index: &ProjectIndex,
) {
    let search_index = ProjectSearchIndex::build(index, SearchOptions::default().max_file_bytes);
    let _ = crate::ui_event_channel::send_critical_ui_event(
        tx,
        UiEvent::ProjectSearchIndexed {
            request_id,
            root,
            search_index,
        },
    );
}

fn reserve_startup_task_request_id_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
) -> u64 {
    *next_request_id = next_startup_task_request_id(*next_request_id);
    *active_request_id = *next_request_id;
    *active_request_id
}

fn next_startup_task_request_id(current: u64) -> u64 {
    match current.wrapping_add(1) {
        0 => 1,
        next => next,
    }
}

fn begin_startup_task_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    queued: &mut bool,
) -> Option<u64> {
    if in_flight_request_id.is_some() {
        if !*queued {
            let _ = reserve_startup_task_request_id_state(next_request_id, active_request_id);
            *queued = true;
        }
        None
    } else {
        let request_id = reserve_startup_task_request_id_state(next_request_id, active_request_id);
        *in_flight_request_id = Some(request_id);
        Some(request_id)
    }
}

fn finish_startup_task_request_state(
    in_flight_request_id: &mut Option<u64>,
    queued: &mut bool,
    request_id: u64,
) -> bool {
    if *in_flight_request_id != Some(request_id) {
        return false;
    }
    *in_flight_request_id = None;
    let should_spawn_queued = *queued;
    *queued = false;
    should_spawn_queued
}

fn invalidate_startup_task_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    queued: &mut bool,
) {
    let _ = reserve_startup_task_request_id_state(next_request_id, active_request_id);
    *in_flight_request_id = None;
    *queued = false;
}

fn begin_workspace_index_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    refresh_queued: &mut bool,
) -> Option<u64> {
    begin_startup_task_request_state(
        next_request_id,
        active_request_id,
        in_flight_request_id,
        refresh_queued,
    )
}

fn finish_workspace_index_request_state(
    in_flight_request_id: &mut Option<u64>,
    refresh_queued: &mut bool,
    request_id: u64,
) -> bool {
    finish_startup_task_request_state(in_flight_request_id, refresh_queued, request_id)
}

fn invalidate_workspace_index_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    refresh_queued: &mut bool,
) {
    invalidate_startup_task_request_state(
        next_request_id,
        active_request_id,
        in_flight_request_id,
        refresh_queued,
    );
}

fn begin_git_scan_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    refresh_queued: &mut bool,
) -> Option<u64> {
    begin_startup_task_request_state(
        next_request_id,
        active_request_id,
        in_flight_request_id,
        refresh_queued,
    )
}

fn finish_git_scan_request_state(
    in_flight_request_id: &mut Option<u64>,
    refresh_queued: &mut bool,
    request_id: u64,
) -> bool {
    finish_startup_task_request_state(in_flight_request_id, refresh_queued, request_id)
}

fn invalidate_git_scan_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    refresh_queued: &mut bool,
) {
    invalidate_startup_task_request_state(
        next_request_id,
        active_request_id,
        in_flight_request_id,
        refresh_queued,
    );
}

fn begin_workspace_plugin_discovery_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    reload_queued: &mut bool,
) -> Option<u64> {
    begin_startup_task_request_state(
        next_request_id,
        active_request_id,
        in_flight_request_id,
        reload_queued,
    )
}

fn finish_workspace_plugin_discovery_request_state(
    in_flight_request_id: &mut Option<u64>,
    reload_queued: &mut bool,
    request_id: u64,
) -> bool {
    finish_startup_task_request_state(in_flight_request_id, reload_queued, request_id)
}

fn invalidate_workspace_plugin_discovery_request_state(
    next_request_id: &mut u64,
    active_request_id: &mut u64,
    in_flight_request_id: &mut Option<u64>,
    reload_queued: &mut bool,
) {
    invalidate_startup_task_request_state(
        next_request_id,
        active_request_id,
        in_flight_request_id,
        reload_queued,
    );
}

pub(crate) fn workspace_plugins_enabled(workspace_trusted: bool) -> bool {
    workspace_trusted
}

pub(crate) fn workspace_plugins_restricted_status() -> &'static str {
    "Trust this workspace to enable workspace plugins"
}

pub(crate) fn workspace_plugin_reload_due(
    pending: Option<Instant>,
    now: Instant,
    debounce: Duration,
) -> bool {
    pending.is_some_and(|pending| now.saturating_duration_since(pending) >= debounce)
}

pub(crate) fn workspace_refresh_due(
    pending: Option<PendingWorkspaceRefresh>,
    now: Instant,
    debounce: Duration,
    max_wait: Duration,
) -> bool {
    pending.is_some_and(|pending| {
        now.saturating_duration_since(pending.last_seen) >= debounce
            || now.saturating_duration_since(pending.first_seen) >= max_wait
    })
}

pub(crate) fn git_auto_refresh_enabled(git_enabled: bool, git_autorefresh: bool) -> bool {
    git_enabled && git_autorefresh
}

pub(crate) fn git_open_parent_repositories(mode: GitOpenRepositoryInParentFolders) -> bool {
    !matches!(mode, GitOpenRepositoryInParentFolders::Never)
}

pub(crate) fn git_scan_root_for_auto_repository_detection(
    root: &Path,
    mode: GitAutoRepositoryDetection,
    repository_scan_max_depth: usize,
    ignored_folders: &[String],
    open_editor_paths: &[PathBuf],
) -> Option<PathBuf> {
    match mode {
        GitAutoRepositoryDetection::False => None,
        GitAutoRepositoryDetection::True => Some(root.to_path_buf()),
        GitAutoRepositoryDetection::SubFolders => {
            if git_repository_marker_exists(root) {
                Some(root.to_path_buf())
            } else {
                git_repository_in_subfolders(root, repository_scan_max_depth, ignored_folders)
            }
        }
        GitAutoRepositoryDetection::OpenEditors => open_editor_paths
            .iter()
            .filter_map(|path| git_repository_for_open_editor(root, path, ignored_folders))
            .next(),
    }
}

#[cfg(test)]
fn cached_git_scan_root_for_auto_repository_detection(
    cache: &mut Option<GitScanRootCacheEntry>,
    root: &Path,
    mode: GitAutoRepositoryDetection,
    repository_scan_max_depth: usize,
    ignored_folders: &[String],
    open_editor_paths: &[PathBuf],
) -> Option<PathBuf> {
    let (scan_root, next_cache) = resolved_cached_git_scan_root_for_auto_repository_detection(
        cache.clone(),
        root,
        mode,
        repository_scan_max_depth,
        ignored_folders,
        open_editor_paths,
    );
    *cache = next_cache;
    scan_root
}

fn resolved_cached_git_scan_root_for_auto_repository_detection(
    cache: Option<GitScanRootCacheEntry>,
    root: &Path,
    mode: GitAutoRepositoryDetection,
    repository_scan_max_depth: usize,
    ignored_folders: &[String],
    open_editor_paths: &[PathBuf],
) -> (Option<PathBuf>, Option<GitScanRootCacheEntry>) {
    if !git_scan_root_is_cacheable(mode) {
        return (
            git_scan_root_for_auto_repository_detection(
                root,
                mode,
                repository_scan_max_depth,
                ignored_folders,
                open_editor_paths,
            ),
            None,
        );
    }

    let key = git_scan_root_cache_key(
        root,
        mode,
        repository_scan_max_depth,
        ignored_folders,
        open_editor_paths,
    );
    if let Some(entry) = cache
        && entry.key == key
        && git_repository_marker_exists(&entry.scan_root)
    {
        return (Some(entry.scan_root.clone()), Some(entry));
    }

    let scan_root = git_scan_root_for_auto_repository_detection(
        root,
        mode,
        repository_scan_max_depth,
        ignored_folders,
        open_editor_paths,
    );
    let cache_entry = scan_root.as_ref().map(|scan_root| GitScanRootCacheEntry {
        key,
        scan_root: scan_root.clone(),
    });
    (scan_root, cache_entry)
}

fn git_scan_root_is_cacheable(mode: GitAutoRepositoryDetection) -> bool {
    matches!(
        mode,
        GitAutoRepositoryDetection::SubFolders | GitAutoRepositoryDetection::OpenEditors
    )
}

fn git_scan_root_cache_key(
    root: &Path,
    mode: GitAutoRepositoryDetection,
    repository_scan_max_depth: usize,
    ignored_folders: &[String],
    open_editor_paths: &[PathBuf],
) -> GitScanRootCacheKey {
    GitScanRootCacheKey {
        workspace_root: root.to_path_buf(),
        mode,
        repository_scan_max_depth: kuroya_core::clamp_git_repository_scan_max_depth(
            repository_scan_max_depth,
        ),
        ignored_folders: ignored_folders
            .iter()
            .map(|entry| entry.trim().to_owned())
            .filter(|entry| !entry.is_empty())
            .collect(),
        open_editor_paths: open_editor_paths.to_vec(),
    }
}

fn git_repository_for_open_editor(
    root: &Path,
    path: &Path,
    ignored_folders: &[String],
) -> Option<PathBuf> {
    let mut current = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };

    loop {
        if !path_is_or_inside(&current, root) {
            return None;
        }
        let ignored = !paths_match(&current, root)
            && git_repository_scan_folder_ignored(&current, ignored_folders);
        if !ignored && git_repository_marker_exists(&current) {
            return Some(current);
        }
        if paths_match(&current, root) || !current.pop() {
            return None;
        }
    }
}

fn git_repository_in_subfolders(
    root: &Path,
    max_depth: usize,
    ignored_folders: &[String],
) -> Option<PathBuf> {
    git_repository_in_subfolders_with_limits(
        root,
        max_depth,
        ignored_folders,
        GIT_REPOSITORY_SCAN_MAX_CHILDREN_PER_FOLDER,
        GIT_REPOSITORY_SCAN_MAX_VISITED_FOLDERS,
    )
}

fn git_repository_in_subfolders_with_limits(
    root: &Path,
    max_depth: usize,
    ignored_folders: &[String],
    max_children_per_folder: usize,
    max_visited_folders: usize,
) -> Option<PathBuf> {
    let max_depth = kuroya_core::clamp_git_repository_scan_max_depth(max_depth);
    if max_depth == 0 || max_children_per_folder == 0 || max_visited_folders == 0 {
        return None;
    }

    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    while let Some((folder, depth)) = pending.pop() {
        if visited >= max_visited_folders {
            return None;
        }
        visited += 1;

        if depth >= max_depth {
            continue;
        }

        let mut children = git_repository_scan_children(&folder, max_children_per_folder);
        children.retain(|child| !git_repository_scan_folder_ignored(child, ignored_folders));
        for child in &children {
            if git_repository_marker_exists(child) {
                return Some(child.clone());
            }
        }
        for child in children.into_iter().rev() {
            pending.push((child, depth + 1));
        }
    }
    None
}

fn git_repository_scan_children(folder: &Path, max_children: usize) -> Vec<PathBuf> {
    if max_children == 0 {
        return Vec::new();
    }
    let Ok(read_dir) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut children = Vec::new();
    for entry in read_dir.filter_map(Result::ok) {
        let path = entry.path();
        if !git_repository_scan_entry_is_dir(&entry, &path) {
            continue;
        }
        children.push(path);
        if children.len() >= max_children {
            break;
        }
    }
    children.sort();
    children
}

fn git_repository_scan_entry_is_dir(entry: &std::fs::DirEntry, path: &Path) -> bool {
    match entry.file_type() {
        Ok(file_type) if file_type.is_dir() => true,
        Ok(file_type) if file_type.is_file() => false,
        Ok(_) | Err(_) => path.is_dir(),
    }
}

fn git_repository_marker_exists(path: &Path) -> bool {
    match std::fs::metadata(path.join(".git")) {
        Ok(metadata) => metadata.is_dir() || metadata.is_file(),
        Err(_) => false,
    }
}

pub(crate) fn git_repository_ignored(root: &Path, ignored_repositories: &[String]) -> bool {
    ignored_repositories.iter().any(|entry| {
        let entry = entry.trim();
        if entry.is_empty() {
            return false;
        }

        let configured = PathBuf::from(entry);
        if configured.is_absolute() {
            paths_match(root, &configured)
        } else {
            paths_match(root, &root.join(&configured)) || root.ends_with(&configured)
        }
    })
}

pub(crate) fn git_repository_scan_folder_ignored(root: &Path, ignored_folders: &[String]) -> bool {
    ignored_folders.iter().any(|entry| {
        let entry = entry.trim();
        if entry.is_empty() {
            return false;
        }

        let configured = PathBuf::from(entry);
        if configured.is_absolute() {
            path_is_or_inside(root, &configured)
        } else {
            path_contains_relative_folder(root, &configured)
        }
    })
}

fn path_is_or_inside(path: &Path, folder: &Path) -> bool {
    let path = normalized_path_key(path);
    let folder = normalized_path_key(folder);
    !folder.is_empty()
        && (path == folder
            || path
                .strip_prefix(&folder)
                .is_some_and(|rest| rest.starts_with('/')))
}

fn path_contains_relative_folder(path: &Path, folder: &Path) -> bool {
    let path = normalized_path_key(path);
    let folder = normalized_path_key(folder);
    !folder.is_empty() && path_contains_relative_folder_key(&path, &folder)
}

fn path_contains_relative_folder_key(path: &str, folder: &str) -> bool {
    let mut search_from = 0;
    while let Some(relative_start) = path[search_from..].find(folder) {
        let start = search_from + relative_start;
        let end = start + folder.len();
        let starts_at_boundary = start == 0 || path.as_bytes()[start - 1] == b'/';
        let ends_at_boundary = end == path.len() || path.as_bytes()[end] == b'/';
        if starts_at_boundary && ends_at_boundary {
            return true;
        }
        search_from = end;
    }
    false
}

fn paths_match(left: &Path, right: &Path) -> bool {
    normalized_path_key(left) == normalized_path_key(right)
}

fn normalized_path_key(path: &Path) -> String {
    let normalized =
        std::fs::canonicalize(path).unwrap_or_else(|_| lexical_normalize_startup_path(path));
    let mut key = String::new();
    let mut first = true;
    for component in normalized.components() {
        if first {
            first = false;
        } else {
            key.push('/');
        }
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Prefix(prefix) => {
                push_normalized_path_key_text(&mut key, &prefix.as_os_str().to_string_lossy());
            }
            std::path::Component::RootDir => {}
            std::path::Component::ParentDir => key.push_str(".."),
            std::path::Component::Normal(part) => {
                push_normalized_path_key_text(&mut key, &part.to_string_lossy());
            }
        }
    }
    while key.len() > 1 && key.ends_with('/') {
        key.pop();
    }
    if cfg!(windows) {
        key.make_ascii_lowercase();
    }
    key
}

fn push_normalized_path_key_text(key: &mut String, text: &str) {
    if text.as_bytes().contains(&b'\\') {
        key.reserve(text.len());
        for ch in text.chars() {
            if ch == '\\' {
                key.push('/');
            } else {
                key.push(ch);
            }
        }
    } else {
        key.push_str(text);
    }
}

fn lexical_normalize_startup_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut has_root = false;
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => {
                has_root = true;
                normalized.push(component.as_os_str());
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let can_pop_normal = normalized
                    .components()
                    .next_back()
                    .is_some_and(|component| matches!(component, std::path::Component::Normal(_)));
                if can_pop_normal {
                    normalized.pop();
                } else if !has_root {
                    normalized.push("..");
                }
            }
            std::path::Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests;
