use crate::{
    KuroyaApp,
    devtools_async_tasks::path_detail,
    file_io::read_utf8_text_file_with_limit,
    large_file_mode::buffer_uses_large_file_mode,
    path_display::{compact_path, display_error_label_cow, display_path_label_cow},
    source_control_runtime::{
        invalidate_source_control_load_request_id_state,
        reserve_source_control_load_request_id_state, source_control_load_event_matches,
    },
    ui_events::UiEvent,
    workspace_trust::workspace_path_contains_lexically,
};
use kuroya_core::{
    GitBlameLine, LanguageId, TextBuffer, blame_file_for_text_with_options_and_short_hash_length,
};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Write as _,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug)]
struct SourceControlBlamePaths {
    io_path: PathBuf,
    key_path: PathBuf,
}

impl KuroyaApp {
    pub(crate) fn open_active_file_blame(&mut self) {
        let Some(path) = self.active_file_or_diff_source_path("open blame") else {
            return;
        };
        self.open_file_blame(path);
    }

    pub(crate) fn open_file_blame(&mut self, path: PathBuf) {
        self.request_file_blame(path, true);
    }

    pub(crate) fn active_git_blame_status_bar_text(&mut self) -> Option<String> {
        if !self.settings.git_blame_status_bar_item_enabled {
            return None;
        }

        if self
            .active_buffer()
            .is_some_and(buffer_uses_large_file_mode)
        {
            return None;
        }

        let (path, line_number) = self.active_file_blame_target()?;
        let paths = self.source_control_blame_paths(&path)?;
        self.sync_source_control_blame_settings();
        self.ensure_file_blame_cached_with_paths(&path, &paths);

        let key_path = &paths.key_path;
        let lines = self.source_control_blame_lines_for_key_or_path(key_path, &path);
        if (self
            .source_control_blame_in_flight_request_ids
            .contains_key(key_path)
            || self
                .source_control_blame_reload_queued_paths
                .contains(key_path))
            && lines.is_none()
        {
            return Some("Blame loading".to_owned());
        }

        let lines = lines?;
        git_blame_status_bar_label(
            lines,
            line_number,
            &self.settings.git_blame_status_bar_item_template,
        )
    }

    fn active_file_blame_target(&self) -> Option<(PathBuf, usize)> {
        let buffer = self.active_buffer()?;
        let path = buffer.path()?.to_path_buf();
        let line_number = buffer.cursor_position().line + 1;
        Some((path, line_number))
    }

    pub(crate) fn ensure_file_blame_cached(&mut self, path: PathBuf) {
        self.sync_source_control_blame_settings();
        let Some(paths) = self.source_control_blame_paths(&path) else {
            return;
        };
        self.ensure_file_blame_cached_with_paths(&path, &paths);
    }

    fn ensure_file_blame_cached_with_paths(
        &mut self,
        path: &Path,
        paths: &SourceControlBlamePaths,
    ) {
        let key_path = &paths.key_path;
        if self
            .source_control_blame_lines_for_key_or_path(key_path, path)
            .is_some()
            || self
                .source_control_blame_in_flight_request_ids
                .contains_key(key_path)
            || self
                .source_control_blame_reload_queued_paths
                .contains(key_path)
        {
            return;
        }
        self.request_file_blame_with_paths(path.to_path_buf(), paths.clone(), false);
    }

    pub(crate) fn request_file_blame(&mut self, path: PathBuf, open_view: bool) {
        self.sync_source_control_blame_settings();
        let Some(paths) = self.source_control_blame_paths(&path) else {
            if open_view {
                self.status = could_not_blame_status(&path, "path is outside workspace");
            }
            return;
        };
        self.request_file_blame_with_paths(path, paths, open_view);
    }

    fn request_file_blame_with_paths(
        &mut self,
        path: PathBuf,
        paths: SourceControlBlamePaths,
        open_view: bool,
    ) {
        let SourceControlBlamePaths {
            io_path: request_path,
            key_path,
        } = paths;
        if open_view {
            self.source_control_blame_open_view_paths
                .insert(key_path.clone());
        }
        let report_loading = open_view
            || self
                .source_control_blame_open_view_paths
                .contains(&key_path);
        self.source_control_blame_pending_path = Some(key_path.clone());
        if self
            .source_control_blame_in_flight_request_ids
            .contains_key(&key_path)
            || self
                .source_control_blame_reload_queued_paths
                .contains(&key_path)
        {
            self.source_control_blame_load_opens_view =
                !self.source_control_blame_open_view_paths.is_empty();
            if report_loading {
                self.status = loading_blame_status(&path);
            }
            return;
        }
        let Some(request_id) = self.begin_source_control_blame_request(&key_path) else {
            if report_loading {
                self.status = loading_blame_status(&path);
            }
            return;
        };
        let event_root = self.workspace.root.clone();
        let git_root = self.source_control_git_operation_root();
        let operation_root = git_root.clone();
        let max_bytes = self.diff_options().max_file_size_bytes;
        let ignore_whitespace = self.settings.git_blame_ignore_whitespace;
        let short_hash_length = self.settings.git_commit_short_hash_length;
        let tx = self.tx.clone();
        let event_path = path.clone();
        self.source_control_blame_load_opens_view =
            !self.source_control_blame_open_view_paths.is_empty();
        if report_loading {
            self.status = loading_blame_status(&path);
        }
        self.record_async_task_started("Git Blame", path_detail(&path));
        self.runtime.spawn_blocking(move || {
            let result = read_blame_text(&request_path, max_bytes)
                .map_err(anyhow::Error::msg)
                .and_then(|text| {
                    blame_file_for_text_with_options_and_short_hash_length(
                        &git_root,
                        &request_path,
                        &text,
                        ignore_whitespace,
                        short_hash_length,
                    )
                    .map(|lines| (lines, text))
                });
            let event = match result {
                Ok((lines, text)) => UiEvent::GitBlameLoaded {
                    request_id,
                    root: event_root,
                    operation_root,
                    path: event_path,
                    lines,
                    text,
                },
                Err(error) => UiEvent::GitBlameFailed {
                    request_id,
                    root: event_root,
                    operation_root,
                    path: event_path,
                    error: error.to_string(),
                },
            };
            let _ = crate::ui_event_channel::send_ui_event(&tx, event);
        });
    }

    pub(crate) fn apply_git_blame_loaded(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        path: PathBuf,
        lines: Vec<GitBlameLine>,
        text: String,
    ) {
        let Some(key_path) = self.source_control_blame_key(&path) else {
            return;
        };
        if !self.source_control_blame_event_matches(&root, &operation_root, &key_path, request_id) {
            return;
        }

        self.finish_matching_source_control_blame_event(&key_path);
        let blame_view = self
            .source_control_blame_open_view_paths
            .remove(&key_path)
            .then(|| format_git_blame_view(&lines, &text));
        self.cache_source_control_blame_lines(&path, &key_path, lines);

        if let Some(view) = blame_view {
            self.open_blame_buffer(path, view);
        }
        self.source_control_blame_load_opens_view =
            !self.source_control_blame_open_view_paths.is_empty();
    }

    pub(crate) fn apply_git_blame_failed(
        &mut self,
        request_id: u64,
        root: PathBuf,
        operation_root: PathBuf,
        path: PathBuf,
        error: String,
    ) {
        let Some(key_path) = self.source_control_blame_key(&path) else {
            return;
        };
        if !self.source_control_blame_event_matches(&root, &operation_root, &key_path, request_id) {
            return;
        }

        self.finish_matching_source_control_blame_event(&key_path);
        if self.source_control_blame_open_view_paths.remove(&key_path) {
            self.status = could_not_blame_status(&path, &error);
        } else {
            self.cache_source_control_blame_lines(&path, &key_path, Vec::new());
        }
        self.source_control_blame_load_opens_view =
            !self.source_control_blame_open_view_paths.is_empty();
    }

    fn begin_source_control_blame_request(&mut self, path: &Path) -> Option<u64> {
        let request_id = reserve_source_control_load_request_id_state(
            &mut self.source_control_blame_next_request_id,
            &mut self.source_control_blame_active_request_id,
        );
        let path = path.to_path_buf();
        self.source_control_blame_active_request_ids
            .insert(path.clone(), request_id);
        match self.source_control_blame_in_flight_request_ids.entry(path) {
            Entry::Occupied(entry) => {
                self.source_control_blame_reload_queued_paths
                    .insert(entry.key().to_path_buf());
                None
            }
            Entry::Vacant(entry) => {
                entry.insert(request_id);
                Some(request_id)
            }
        }
    }

    pub(crate) fn finish_source_control_blame_request(
        &mut self,
        path: &Path,
        request_id: u64,
    ) -> bool {
        let Some(key_path) = self.source_control_blame_key(path) else {
            return false;
        };
        if self
            .source_control_blame_in_flight_request_ids
            .get(&key_path)
            != Some(&request_id)
        {
            return false;
        }
        self.source_control_blame_in_flight_request_ids
            .remove(&key_path);
        let reload_queued = self
            .source_control_blame_reload_queued_paths
            .remove(&key_path);
        if reload_queued {
            self.source_control_blame_active_request_ids
                .remove(&key_path);
            if self.source_control_blame_pending_path.as_deref() == Some(key_path.as_path()) {
                self.refresh_source_control_blame_pending_path();
            }
        }
        reload_queued
    }

    fn source_control_blame_event_matches(
        &self,
        root: &Path,
        operation_root: &Path,
        path: &Path,
        request_id: u64,
    ) -> bool {
        source_control_load_event_matches(
            &self.workspace.root,
            root,
            request_id,
            self.source_control_blame_active_request_ids
                .get(path)
                .copied()
                .unwrap_or_default(),
        ) && self.source_control_git_operation_root_matches(operation_root)
    }

    fn finish_matching_source_control_blame_event(&mut self, path: &Path) {
        self.source_control_blame_active_request_ids.remove(path);
        if self.source_control_blame_pending_path.as_deref() == Some(path) {
            self.refresh_source_control_blame_pending_path();
        }
    }

    fn refresh_source_control_blame_pending_path(&mut self) {
        self.source_control_blame_pending_path = self
            .source_control_blame_in_flight_request_ids
            .keys()
            .next()
            .cloned()
            .or_else(|| {
                self.source_control_blame_reload_queued_paths
                    .iter()
                    .next()
                    .cloned()
            });
    }

    pub(crate) fn sync_source_control_blame_settings(&mut self) {
        if self.source_control_blame_ignore_whitespace == self.settings.git_blame_ignore_whitespace
        {
            return;
        }
        self.source_control_blame_ignore_whitespace = self.settings.git_blame_ignore_whitespace;
        self.source_control_blame_pending_path = None;
        self.source_control_blame_load_opens_view = false;
        self.source_control_blame_cache.clear();
        invalidate_source_control_load_request_id_state(
            &mut self.source_control_blame_next_request_id,
            &mut self.source_control_blame_active_request_id,
        );
        self.source_control_blame_active_request_ids.clear();
        self.source_control_blame_in_flight_request_ids.clear();
        self.source_control_blame_reload_queued_paths.clear();
        self.source_control_blame_open_view_paths.clear();
    }

    pub(crate) fn clear_source_control_blame_for_path(&mut self, path: &Path) {
        let Some(key_path) = self.source_control_blame_key(path) else {
            return;
        };
        let root = self.workspace.root.clone();
        self.source_control_blame_cache.retain(|cached_path, _| {
            source_control_blame_key_for_path(&root, cached_path).as_deref() != Some(&key_path)
        });
        let had_active = self
            .source_control_blame_active_request_ids
            .remove(&key_path)
            .is_some();
        let had_in_flight = self
            .source_control_blame_in_flight_request_ids
            .remove(&key_path)
            .is_some();
        let had_queued = self
            .source_control_blame_reload_queued_paths
            .remove(&key_path);
        let had_open_view = self.source_control_blame_open_view_paths.remove(&key_path);
        let was_pending = self.source_control_blame_pending_path.as_deref() == Some(&key_path);
        let had_request_state =
            had_active || had_in_flight || had_queued || had_open_view || was_pending;
        if had_request_state {
            if was_pending {
                self.refresh_source_control_blame_pending_path();
            }
            self.source_control_blame_load_opens_view =
                !self.source_control_blame_open_view_paths.is_empty();
            invalidate_source_control_load_request_id_state(
                &mut self.source_control_blame_next_request_id,
                &mut self.source_control_blame_active_request_id,
            );
        }
    }

    fn source_control_blame_key(&self, path: &Path) -> Option<PathBuf> {
        self.source_control_blame_paths(path)
            .map(|paths| paths.key_path)
    }

    fn source_control_blame_paths(&self, path: &Path) -> Option<SourceControlBlamePaths> {
        source_control_blame_paths_for_path(&self.workspace.root, path)
    }

    pub(crate) fn source_control_blame_lines_for_path(
        &self,
        path: &Path,
    ) -> Option<&[GitBlameLine]> {
        let key_path = self.source_control_blame_key(path)?;
        self.source_control_blame_lines_for_key_or_path(&key_path, path)
    }

    fn source_control_blame_lines_for_key_or_path(
        &self,
        key_path: &Path,
        path: &Path,
    ) -> Option<&[GitBlameLine]> {
        self.source_control_blame_cache
            .get(key_path)
            .or_else(|| self.source_control_blame_cache.get(path))
            .map(Vec::as_slice)
    }

    fn cache_source_control_blame_lines(
        &mut self,
        original_path: &Path,
        key_path: &Path,
        lines: Vec<GitBlameLine>,
    ) {
        if original_path == key_path {
            self.source_control_blame_cache
                .insert(key_path.to_path_buf(), lines);
        } else {
            self.source_control_blame_cache
                .insert(key_path.to_path_buf(), lines.clone());
            self.source_control_blame_cache
                .insert(original_path.to_path_buf(), lines);
        }
    }

    fn open_blame_buffer(&mut self, path: PathBuf, view: String) {
        let label = format!("{} (Blame)", blame_label_for_path(&path));
        if let Some(existing_id) = self
            .virtual_buffer_labels
            .iter()
            .find_map(|(id, existing)| (existing == &label).then_some(*id))
        {
            if let Some(buffer) = self.buffer_mut(existing_id) {
                buffer.replace_from_disk(view);
                buffer.set_read_only(true);
            }
            self.set_active_buffer(existing_id);
            self.status = updated_blame_status(&path);
            return;
        }

        let id = self.next_id();
        let mut buffer = TextBuffer::from_text_with_language(id, None, view, LanguageId::PlainText);
        buffer.set_word_separators(self.settings.word_separators.clone());
        buffer.set_read_only(true);
        self.buffers.push(buffer);
        self.virtual_buffer_labels.insert(id, label);
        self.set_active_buffer(id);
        self.status = opened_blame_status(&path);
    }
}

pub(crate) fn source_control_blame_key_for_path(
    workspace_root: &Path,
    path: &Path,
) -> Option<PathBuf> {
    source_control_blame_paths_for_path(workspace_root, path).map(|paths| paths.key_path)
}

fn source_control_blame_paths_for_path(
    workspace_root: &Path,
    path: &Path,
) -> Option<SourceControlBlamePaths> {
    let io_path = source_control_blame_io_path_for_path(workspace_root, path)?;
    let key_path = normalize_blame_cache_key(&io_path);
    Some(SourceControlBlamePaths { io_path, key_path })
}

fn source_control_blame_io_path_for_path(workspace_root: &Path, path: &Path) -> Option<PathBuf> {
    let root = normalize_blame_path(workspace_root);
    let normalized_path = normalize_blame_path(path);
    if path.is_absolute() {
        return workspace_path_contains_lexically(&root, &normalized_path)
            .then_some(normalized_path);
    }

    if !relative_blame_path_stays_inside_workspace(path) {
        return None;
    }
    if workspace_path_contains_lexically(&root, &normalized_path) {
        return Some(normalized_path);
    }

    let joined = normalize_blame_path(&root.join(normalized_path));
    if workspace_path_contains_lexically(&root, &joined) {
        Some(joined)
    } else {
        None
    }
}

fn relative_blame_path_stays_inside_workspace(path: &Path) -> bool {
    if path.is_absolute() {
        return false;
    }
    let mut depth = 0usize;
    for component in path.components() {
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

fn normalize_blame_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::ParentDir) | None => {
                    normalized.push(component.as_os_str());
                }
                Some(Component::Prefix(_)) | Some(Component::RootDir) => {}
                Some(Component::CurDir) => {}
            },
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn normalize_blame_cache_key(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

pub(crate) fn read_blame_text(path: &Path, max_bytes: usize) -> Result<String, String> {
    let text = read_utf8_text_file_with_limit(path, max_bytes)?;
    if text.as_bytes().contains(&0) {
        return Err("binary file skipped".to_owned());
    }
    Ok(text)
}

pub(crate) fn format_git_blame_view(lines: &[GitBlameLine], text: &str) -> String {
    let indexed_lines = blame_lines_match_source_order(lines);
    let blame_by_line = (!indexed_lines).then(|| {
        let mut blame_by_line = HashMap::with_capacity(lines.len());
        blame_by_line.extend(lines.iter().map(|line| (line.line_number, line)));
        blame_by_line
    });
    let mut output =
        String::with_capacity(text.len().saturating_add(lines.len().saturating_mul(128)));
    for (index, source) in text.lines().enumerate() {
        let line_number = index + 1;
        let blame = if indexed_lines {
            lines.get(index)
        } else {
            blame_by_line
                .as_ref()
                .and_then(|blame_by_line| blame_by_line.get(&line_number).copied())
        };
        if let Some(blame) = blame {
            let _ = writeln!(
                output,
                "{line_number:>6} {} {:<20} {} | {source}",
                blame_row_field(&blame.short_oid, 16),
                blame_row_field(&blame.author, 20),
                blame_row_field(&blame.summary, 72)
            );
        } else {
            let _ = writeln!(
                output,
                "{line_number:>6} {:<8} {:<20} (uncommitted) | {source}",
                "--------", "Unknown"
            );
        }
    }
    output
}

fn blame_lines_match_source_order(lines: &[GitBlameLine]) -> bool {
    lines
        .iter()
        .enumerate()
        .all(|(index, line)| line.line_number == index + 1)
}

pub(crate) fn git_blame_status_bar_label(
    lines: &[GitBlameLine],
    line_number: usize,
    template: &str,
) -> Option<String> {
    git_blame_status_bar_label_at(lines, line_number, template, unix_now_seconds())
}

pub(crate) fn git_blame_status_bar_label_at(
    lines: &[GitBlameLine],
    line_number: usize,
    template: &str,
    now_seconds: i64,
) -> Option<String> {
    let blame = git_blame_line_for_line(lines, line_number)?;
    non_empty_template(render_git_blame_template(
        template,
        blame,
        now_seconds,
        BlameTemplateFieldLimits {
            author: 18,
            subject: 48,
        },
    ))
}

pub(crate) fn git_blame_editor_decoration_label(
    lines: &[GitBlameLine],
    line_number: usize,
    template: &str,
) -> Option<String> {
    git_blame_editor_decoration_label_at(lines, line_number, template, unix_now_seconds())
}

pub(crate) fn git_blame_editor_decoration_label_at(
    lines: &[GitBlameLine],
    line_number: usize,
    template: &str,
    now_seconds: i64,
) -> Option<String> {
    let blame = git_blame_line_for_line(lines, line_number)?;
    non_empty_template(render_git_blame_template(
        template,
        blame,
        now_seconds,
        BlameTemplateFieldLimits {
            author: 18,
            subject: 48,
        },
    ))
}

pub(crate) fn git_blame_editor_decoration_hover_text(
    lines: &[GitBlameLine],
    line_number: usize,
    template: &str,
    disable_hover: bool,
) -> Option<String> {
    git_blame_editor_decoration_hover_text_at(
        lines,
        line_number,
        template,
        disable_hover,
        unix_now_seconds(),
    )
}

pub(crate) fn git_blame_editor_decoration_hover_text_at(
    lines: &[GitBlameLine],
    line_number: usize,
    template: &str,
    disable_hover: bool,
    now_seconds: i64,
) -> Option<String> {
    if disable_hover {
        return None;
    }

    git_blame_editor_decoration_label_at(lines, line_number, template, now_seconds)
}

fn git_blame_line_for_line(lines: &[GitBlameLine], line_number: usize) -> Option<&GitBlameLine> {
    if line_number == 0 {
        return None;
    }
    if let Some(line) = lines
        .get(line_number - 1)
        .filter(|line| line.line_number == line_number)
    {
        return Some(line);
    }
    lines.iter().find(|line| line.line_number == line_number)
}

#[derive(Debug, Clone, Copy)]
struct BlameTemplateFieldLimits {
    author: usize,
    subject: usize,
}

fn render_git_blame_template(
    template: &str,
    blame: &GitBlameLine,
    now_seconds: i64,
    limits: BlameTemplateFieldLimits,
) -> String {
    let mut values = BlameTemplateValues::new(blame, now_seconds, limits);
    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        rendered.push_str(&rest[..start]);
        let placeholder = &rest[start..];
        if let Some(next) = push_blame_template_placeholder(&mut rendered, placeholder, &mut values)
        {
            rest = next;
        } else {
            rendered.push_str("${");
            rest = &placeholder[2..];
        }
    }
    rendered.push_str(rest);
    rendered
}

struct BlameTemplateValues<'a> {
    blame: &'a GitBlameLine,
    now_seconds: i64,
    limits: BlameTemplateFieldLimits,
    author: Option<String>,
    summary: Option<String>,
    short_hash: Option<String>,
    age: Option<String>,
}

impl<'a> BlameTemplateValues<'a> {
    fn new(blame: &'a GitBlameLine, now_seconds: i64, limits: BlameTemplateFieldLimits) -> Self {
        Self {
            blame,
            now_seconds,
            limits,
            author: None,
            summary: None,
            short_hash: None,
            age: None,
        }
    }

    fn author(&mut self) -> &str {
        if self.author.is_none() {
            self.author = Some(blame_row_field(&self.blame.author, self.limits.author));
        }
        self.author.as_deref().unwrap_or_default()
    }

    fn summary(&mut self) -> &str {
        if self.summary.is_none() {
            self.summary = Some(blame_row_field(&self.blame.summary, self.limits.subject));
        }
        self.summary.as_deref().unwrap_or_default()
    }

    fn short_hash(&mut self) -> &str {
        if self.short_hash.is_none() {
            self.short_hash = Some(blame_row_field(&self.blame.short_oid, 16));
        }
        self.short_hash.as_deref().unwrap_or_default()
    }

    fn age(&mut self) -> &str {
        if self.age.is_none() {
            self.age = Some(format_relative_time(
                self.blame.author_time_seconds,
                self.now_seconds,
            ));
        }
        self.age.as_deref().unwrap_or_default()
    }
}

fn push_blame_template_placeholder<'a>(
    rendered: &mut String,
    placeholder: &'a str,
    values: &mut BlameTemplateValues<'_>,
) -> Option<&'a str> {
    const AUTHOR_NAME: &str = "${authorName}";
    const AUTHOR: &str = "${author}";
    const SUBJECT: &str = "${subject}";
    const SUMMARY: &str = "${summary}";
    const HASH: &str = "${hash}";
    const SHORT_HASH: &str = "${shortHash}";
    const AUTHOR_DATE_AGO: &str = "${authorDateAgo}";

    let token = if placeholder.starts_with(AUTHOR_NAME) {
        rendered.push_str(values.author());
        AUTHOR_NAME
    } else if placeholder.starts_with(AUTHOR) {
        rendered.push_str(values.author());
        AUTHOR
    } else if placeholder.starts_with(SUBJECT) {
        rendered.push_str(values.summary());
        SUBJECT
    } else if placeholder.starts_with(SUMMARY) {
        rendered.push_str(values.summary());
        SUMMARY
    } else if placeholder.starts_with(SHORT_HASH) {
        rendered.push_str(values.short_hash());
        SHORT_HASH
    } else if placeholder.starts_with(HASH) {
        rendered.push_str(values.short_hash());
        HASH
    } else if placeholder.starts_with(AUTHOR_DATE_AGO) {
        rendered.push_str(values.age());
        AUTHOR_DATE_AGO
    } else {
        return None;
    };
    Some(&placeholder[token.len()..])
}

fn non_empty_template(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn format_relative_time(then_seconds: i64, now_seconds: i64) -> String {
    let elapsed = now_seconds.saturating_sub(then_seconds).max(0);
    if elapsed < 60 {
        return "just now".to_owned();
    }
    let minutes = elapsed / 60;
    if minutes < 60 {
        return plural_time(minutes, "minute");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return plural_time(hours, "hour");
    }
    let days = hours / 24;
    if days < 30 {
        return plural_time(days, "day");
    }
    let months = days / 30;
    if months < 12 {
        return plural_time(months, "month");
    }
    plural_time(days / 365, "year")
}

fn plural_time(value: i64, unit: &str) -> String {
    if value == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{value} {unit}s ago")
    }
}

fn unix_now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn blame_row_field(value: &str, max_chars: usize) -> String {
    truncate_blame_field(&single_line_blame_field(value), max_chars)
}

fn single_line_blame_field(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_control() {
            pending_space = !sanitized.is_empty();
            continue;
        }
        if character.is_whitespace() {
            pending_space = !sanitized.is_empty();
            continue;
        }
        if pending_space && !sanitized.ends_with(' ') {
            sanitized.push(' ');
        }
        pending_space = false;
        sanitized.push(character);
    }
    sanitized
}

fn truncate_blame_field(value: &str, max_chars: usize) -> String {
    if max_chars <= 3 {
        return if value.chars().count() <= max_chars {
            value.to_owned()
        } else {
            ".".repeat(max_chars)
        };
    }
    let Some((truncate_at, _)) = value.char_indices().nth(max_chars - 3) else {
        return value.to_owned();
    };
    let Some(_) = value[truncate_at..].chars().nth(3) else {
        return value.to_owned();
    };
    let mut truncated = value[..truncate_at].to_owned();
    truncated.push_str("...");
    truncated
}

fn blame_label_for_path(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| compact_path(path))
}

fn blame_path_status(prefix: &'static str, path: &Path) -> String {
    let label = display_path_label_cow(path);
    let mut status = String::with_capacity(prefix.len() + label.len());
    status.push_str(prefix);
    status.push_str(&label);
    status
}

fn loading_blame_status(path: &Path) -> String {
    blame_path_status("Loading blame for ", path)
}

fn opened_blame_status(path: &Path) -> String {
    blame_path_status("Opened blame for ", path)
}

fn updated_blame_status(path: &Path) -> String {
    blame_path_status("Updated blame for ", path)
}

fn could_not_blame_status(path: &Path, error: &str) -> String {
    let path = display_path_label_cow(path);
    let error = display_error_label_cow(error);
    let mut status = String::with_capacity("Could not blame : ".len() + path.len() + error.len());
    status.push_str("Could not blame ");
    status.push_str(&path);
    status.push_str(": ");
    status.push_str(&error);
    status
}

#[cfg(test)]
mod tests {
    use super::{
        could_not_blame_status, git_blame_status_bar_label_at, loading_blame_status,
        opened_blame_status, read_blame_text, source_control_blame_key_for_path,
        source_control_blame_paths_for_path, updated_blame_status,
    };
    use crate::path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS};
    use crate::source_control_runtime::source_control_app_for_test;
    use kuroya_core::GitBlameLine;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("kuroya-blame-text-{}-{name}", std::process::id()))
    }

    #[test]
    fn read_blame_text_rejects_utf8_file_with_nul_bytes_as_binary() {
        let path = temp_path("binary.dat");
        fs::write(&path, b"binary\0text\n").unwrap();

        let error = read_blame_text(&path, 99).unwrap_err();

        assert_eq!(error, "binary file skipped");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_blame_text_accepts_plain_utf8_text() {
        let path = temp_path("text.rs");
        fs::write(&path, b"fn main() {}\n").unwrap();

        let text = read_blame_text(&path, 99).unwrap();

        assert_eq!(text, "fn main() {}\n");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn blame_paths_resolve_relative_io_path_and_cache_key_together() {
        let root = PathBuf::from("workspace");
        let path = PathBuf::from("src")
            .join(".")
            .join("generated")
            .join("..")
            .join("main.rs");
        let expected_io_path = root.join("src").join("main.rs");

        let paths = source_control_blame_paths_for_path(&root, &path).unwrap();

        assert_eq!(paths.io_path, expected_io_path);
        assert_eq!(
            paths.key_path,
            source_control_blame_key_for_path(&root, &path).unwrap()
        );
    }

    #[cfg(windows)]
    #[test]
    fn blame_paths_preserve_raw_io_case_while_cache_key_matches_case_aliases() {
        let root = PathBuf::from(r"C:\Repo");
        let path = PathBuf::from(r"C:\Repo").join("SRC").join("MAIN.rs");

        let paths = source_control_blame_paths_for_path(&root, &path).unwrap();

        assert_eq!(paths.io_path, path);
        assert_eq!(paths.key_path, PathBuf::from(r"c:\repo\src\main.rs"));
    }

    #[test]
    fn blame_path_statuses_preserve_exact_clean_path_text() {
        let path = Path::new("workspace/src/main.rs");

        assert_eq!(loading_blame_status(path), "Loading blame for main.rs");
        assert_eq!(opened_blame_status(path), "Opened blame for main.rs");
        assert_eq!(updated_blame_status(path), "Updated blame for main.rs");
        assert_eq!(
            could_not_blame_status(path, "path is outside workspace"),
            "Could not blame main.rs: path is outside workspace"
        );
    }

    #[test]
    fn blame_path_statuses_sanitize_and_bound_display_labels() {
        let path = Path::new("workspace/src")
            .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(24)));

        let statuses = [
            ("Loading blame for ", loading_blame_status(&path)),
            ("Opened blame for ", opened_blame_status(&path)),
            ("Updated blame for ", updated_blame_status(&path)),
        ];

        for (prefix, status) in statuses {
            assert!(status.starts_with(prefix));
            assert!(!status.contains('\n'));
            assert!(!status.contains('\u{202e}'));
            assert!(status.contains("..."));
            assert!(
                status.chars().count() <= prefix.chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS
            );
        }
    }

    #[test]
    fn blame_failure_status_sanitizes_and_bounds_path_and_error_labels() {
        let path = Path::new("workspace/src")
            .join(format!("bad\r{}\u{202e}tail.rs", "very-long-".repeat(24)));
        let error = format!(
            "fatal: first line\nsecond line \u{2066}{}\u{202e}",
            "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        );

        let status = could_not_blame_status(&path, &error);

        assert!(status.starts_with("Could not blame "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\r'));
        assert!(!status.contains('\u{202e}'));
        assert!(!status.contains('\u{2066}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not blame ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + ": ".chars().count()
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn queued_blame_completion_unblocks_reload_without_applying_stale_content() {
        let root = PathBuf::from("workspace");
        let path = PathBuf::from("src/main.rs");
        let key_path = source_control_blame_key_for_path(&root, &path).unwrap();
        let mut app = source_control_app_for_test(root.clone(), true);
        let request_id = app.begin_source_control_blame_request(&key_path).unwrap();
        app.source_control_blame_pending_path = Some(key_path.clone());
        app.source_control_blame_reload_queued_paths
            .insert(key_path.clone());
        app.source_control_blame_open_view_paths
            .insert(key_path.clone());

        assert!(app.finish_source_control_blame_request(&path, request_id));

        app.apply_git_blame_loaded(
            request_id,
            root.clone(),
            root,
            path.clone(),
            vec![blame_line(1, "Old", "stale")],
            "old\n".to_owned(),
        );

        assert!(!app.source_control_blame_cache.contains_key(&key_path));
        assert!(app.source_control_blame_open_view_paths.contains(&key_path));
        assert!(
            !app.source_control_blame_active_request_ids
                .contains_key(&key_path)
        );
        assert_eq!(app.source_control_blame_pending_path, None);
    }

    #[test]
    fn blame_status_label_falls_back_for_sparse_lines_and_sanitizes_fields() {
        let lines = vec![
            blame_line(2, "Other", "different line"),
            blame_line(10, "\tAda\nLovelace", "fix\rmetadata"),
        ];

        assert_eq!(
            git_blame_status_bar_label_at(&lines, 10, "${authorName}: ${subject}", 1_700_000_000),
            Some("Ada Lovelace: fix metadata".to_owned())
        );
        assert_eq!(
            git_blame_status_bar_label_at(&lines, 1, "${authorName}", 1_700_000_000),
            None
        );
    }

    fn blame_line(line_number: usize, author: &str, summary: &str) -> GitBlameLine {
        GitBlameLine {
            line_number,
            short_oid: "12345678".to_owned(),
            author: author.to_owned(),
            author_time_seconds: 1_700_000_000,
            summary: summary.to_owned(),
        }
    }
}
