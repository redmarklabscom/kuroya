use super::{SourceControlStageSectionKind, labels::source_control_status_path_label_cow};
use eframe::egui::{Color32, FontFamily, FontId};
use kuroya_core::{
    GitChangeStage, GitFileStatus, GitSmartCommitChanges, GitStatusEntry,
    clamp_git_input_validation_length, clamp_scm_input_font_size, clamp_scm_input_line_count,
};
use std::{borrow::Cow, collections::BTreeSet};

pub(crate) fn source_control_commit_input_visible(show_commit_input: bool) -> bool {
    show_commit_input
}

pub(crate) fn source_control_empty_changes_commit_input_visible(
    show_commit_input: bool,
    raw_entry_count: usize,
    entries: &[GitStatusEntry],
) -> bool {
    raw_entry_count == 0
        && entries.is_empty()
        && source_control_commit_input_visible(show_commit_input)
}

pub(crate) fn source_control_view_action_button_visible(show_action_button: bool) -> bool {
    show_action_button
}

pub(crate) fn source_control_commit_action_button_visible(
    show_input_action_button: bool,
    show_commit_action_button: bool,
) -> bool {
    show_input_action_button && show_commit_action_button
}

pub(crate) fn source_control_commit_input_rows(
    message: &str,
    min_line_count: usize,
    max_line_count: usize,
) -> usize {
    let min_line_count = clamp_scm_input_line_count(min_line_count);
    let max_line_count = clamp_scm_input_line_count(max_line_count).max(min_line_count);
    let message_lines = message.split('\n').count().max(1);
    message_lines.clamp(min_line_count, max_line_count)
}

pub(crate) fn source_control_commit_input_rows_for_mode(
    use_editor_as_commit_input: bool,
    message: &str,
    min_line_count: usize,
    max_line_count: usize,
) -> usize {
    if use_editor_as_commit_input {
        source_control_commit_input_rows(message, min_line_count, max_line_count)
    } else {
        1
    }
}

pub(crate) fn source_control_commit_input_font(
    family: &str,
    font_size: f32,
    _editor_font_size: f32,
    _ui_font_size: f32,
) -> FontId {
    let family = family.trim();
    let font_size = clamp_scm_input_font_size(font_size);
    if family.eq_ignore_ascii_case("editor") {
        FontId::new(font_size, FontFamily::Monospace)
    } else if family.eq_ignore_ascii_case("default") || family.is_empty() {
        FontId::new(font_size, FontFamily::Proportional)
    } else {
        FontId::new(font_size, FontFamily::Name(family.to_owned().into()))
    }
}

pub(crate) fn source_control_verbose_commit_preview(
    entries: &[GitStatusEntry],
    verbose_commit: bool,
    use_editor_as_commit_input: bool,
) -> Option<String> {
    if !verbose_commit || !use_editor_as_commit_input {
        return None;
    }

    let mut preview = String::new();
    for entry in entries
        .iter()
        .filter(|entry| entry.stage == GitChangeStage::Staged)
    {
        if preview.is_empty() {
            preview = String::with_capacity(64 + entries.len().saturating_mul(32));
            preview.push_str("# Changes to be committed:\n#");
        }
        preview.push_str("\n#\t");
        preview.push_str(source_control_status_commit_label(entry.status));
        preview.push_str(": ");
        let path_label = source_control_status_path_label_cow(&entry.path);
        preview.push_str(path_label.as_ref());
    }
    (!preview.is_empty()).then_some(preview)
}

pub(crate) fn source_control_clear_commit_input(
    message: &mut String,
    clear_requested: bool,
) -> bool {
    if clear_requested && !message.is_empty() {
        message.clear();
        true
    } else {
        false
    }
}

pub(crate) fn record_source_control_commit_history(
    history: &mut Vec<String>,
    message: &str,
    limit: usize,
) {
    if limit == 0 {
        return;
    }
    let Some(message) = normalized_source_control_commit_history_message(message) else {
        return;
    };
    record_normalized_source_control_commit_history(history, message.into_owned(), limit);
}

fn record_source_control_commit_history_owned(
    history: &mut Vec<String>,
    message: String,
    limit: usize,
) {
    if limit == 0 {
        return;
    }
    let start = message.len() - message.trim_start().len();
    let end = start + message[start..].trim_end().len();
    if start == end {
        return;
    }
    let trimmed = &message[start..end];
    let message = if trimmed.as_bytes().contains(&b'\r') {
        normalize_source_control_commit_history_line_endings(trimmed).into_owned()
    } else if start == 0 && end == message.len() {
        message
    } else {
        trimmed.to_owned()
    };
    record_normalized_source_control_commit_history(history, message, limit);
}

fn normalized_source_control_commit_history_message(message: &str) -> Option<Cow<'_, str>> {
    let message = message.trim();
    if message.is_empty() {
        return None;
    }
    Some(normalize_source_control_commit_history_line_endings(
        message,
    ))
}

fn normalize_source_control_commit_history_line_endings(message: &str) -> Cow<'_, str> {
    if !message.as_bytes().contains(&b'\r') {
        return Cow::Borrowed(message);
    }

    let mut normalized = String::with_capacity(message.len());
    let mut chars = message.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(ch);
        }
    }
    Cow::Owned(normalized)
}

fn record_normalized_source_control_commit_history(
    history: &mut Vec<String>,
    message: String,
    limit: usize,
) {
    history.retain(|item| item != &message);
    history.push(message);
    if history.len() > limit {
        let remove_count = history.len() - limit;
        history.drain(0..remove_count);
    }
}

pub(crate) fn normalize_source_control_commit_history(
    history: Vec<String>,
    limit: usize,
) -> Vec<String> {
    let mut normalized = Vec::with_capacity(history.len().min(limit));
    for message in history {
        record_source_control_commit_history_owned(&mut normalized, message, limit);
    }
    normalized
}

pub(crate) fn source_control_commit_history_message(
    history: &[String],
    current_index: &mut Option<usize>,
    direction: isize,
) -> Option<String> {
    if history.is_empty() || direction == 0 {
        return None;
    }
    let last = history.len() - 1;
    let next_index = match (*current_index, direction.is_positive()) {
        (Some(index), true) => (index + 1).min(last),
        (Some(index), false) => index.saturating_sub(1),
        (None, true) => 0,
        (None, false) => last,
    };
    *current_index = Some(next_index);
    history.get(next_index).cloned()
}

pub(crate) fn source_control_stage_label(stage: GitChangeStage) -> &'static str {
    match stage {
        GitChangeStage::Unstaged => "Changes",
        GitChangeStage::Staged => "Staged Changes",
    }
}

pub(crate) fn source_control_stage_section_label(
    kind: SourceControlStageSectionKind,
) -> &'static str {
    match kind {
        SourceControlStageSectionKind::Changes | SourceControlStageSectionKind::TrackedChanges => {
            "Changes"
        }
        SourceControlStageSectionKind::UntrackedChanges => "Untracked Changes",
        SourceControlStageSectionKind::StagedChanges => "Staged Changes",
    }
}

#[cfg(test)]
pub(crate) fn source_control_commit_enabled(
    entries: &[GitStatusEntry],
    message: &str,
    smart_commit_enabled: bool,
    suggest_smart_commit: bool,
    smart_commit_changes: GitSmartCommitChanges,
    confirm_empty_commits: bool,
) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    source_control_commit_enabled_from_stats(
        message,
        smart_commit_enabled,
        suggest_smart_commit,
        confirm_empty_commits,
        source_control_commit_stats(entries, smart_commit_changes),
    )
}

pub(super) fn source_control_commit_enabled_from_stats(
    message: &str,
    smart_commit_enabled: bool,
    suggest_smart_commit: bool,
    _confirm_empty_commits: bool,
    stats: SourceControlCommitStats,
) -> bool {
    if message.trim().is_empty() {
        return false;
    }
    if stats.has_conflicts {
        return false;
    }
    stats.staged_count > 0
        || ((smart_commit_enabled || suggest_smart_commit) && stats.smart_commit_count > 0)
        || stats.smart_commit_count == 0
}

pub(crate) fn source_control_commit_tooltip(
    staged_count: usize,
    message: &str,
    has_conflicts: bool,
    smart_commit_enabled: bool,
    suggest_smart_commit: bool,
    smart_commit_count: usize,
    confirm_empty_commits: bool,
) -> &'static str {
    if message.trim().is_empty() {
        "Enter a commit message before committing"
    } else if has_conflicts {
        "Resolve merge conflicts before committing"
    } else if staged_count > 0 {
        "Commit staged changes (Ctrl+Enter)"
    } else if smart_commit_enabled && smart_commit_count > 0 {
        "Smart commit eligible changes (Ctrl+Enter)"
    } else if suggest_smart_commit && smart_commit_count > 0 {
        "Stage eligible changes and commit (Ctrl+Enter)"
    } else if smart_commit_count > 0 {
        "Stage changes before committing"
    } else if confirm_empty_commits {
        "Confirm empty commit (Ctrl+Enter)"
    } else {
        "Create empty commit (Ctrl+Enter)"
    }
}

pub(crate) fn source_control_commit_input_validation_diagnostics(
    message: &str,
    input_validation: bool,
    line_length: usize,
    subject_length: usize,
) -> Vec<String> {
    if !input_validation || message.is_empty() {
        return Vec::new();
    }

    let line_length = clamp_git_input_validation_length(line_length);
    let subject_length = subject_length.max(1);
    let mut diagnostics = Vec::with_capacity(2);
    let mut lines = message.lines().enumerate();
    if let Some((_, subject)) = lines.next() {
        let subject_chars = subject.chars().count();
        if subject_chars > subject_length {
            diagnostics.push(format!(
                "Subject is {subject_chars} characters, above the {subject_length} character limit"
            ));
        }
    }

    for (line_index, line) in lines {
        let line_chars = line.chars().count();
        if line_chars > line_length {
            diagnostics.push(format!(
                "Line {} is {line_chars} characters, above the {line_length} character limit",
                line_index + 1
            ));
            break;
        }
    }

    diagnostics
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SourceControlCommitStats {
    pub(super) staged_count: usize,
    pub(super) smart_commit_count: usize,
    pub(super) has_conflicts: bool,
}

pub(super) fn source_control_commit_stats(
    entries: &[GitStatusEntry],
    smart_commit_changes: GitSmartCommitChanges,
) -> SourceControlCommitStats {
    let mut staged_count = 0;
    let mut has_conflicts = false;
    let mut smart_commit_paths = BTreeSet::new();
    for entry in entries {
        if entry.stage == GitChangeStage::Staged {
            staged_count += 1;
        }
        if entry.status == GitFileStatus::Conflicted {
            has_conflicts = true;
        }
        if source_control_smart_commit_entry_included(entry, smart_commit_changes) {
            smart_commit_paths.insert(&entry.path);
        }
    }

    SourceControlCommitStats {
        staged_count,
        smart_commit_count: smart_commit_paths.len(),
        has_conflicts,
    }
}

#[cfg(test)]
pub(crate) fn source_control_smart_commit_count(
    entries: &[GitStatusEntry],
    smart_commit_changes: GitSmartCommitChanges,
) -> usize {
    let mut paths = BTreeSet::new();
    for entry in entries {
        if source_control_smart_commit_entry_included(entry, smart_commit_changes) {
            paths.insert(&entry.path);
        }
    }
    paths.len()
}

fn source_control_smart_commit_entry_included(
    entry: &GitStatusEntry,
    smart_commit_changes: GitSmartCommitChanges,
) -> bool {
    if entry.stage != GitChangeStage::Unstaged {
        return false;
    }
    match entry.status {
        GitFileStatus::Modified | GitFileStatus::Deleted | GitFileStatus::Renamed => true,
        GitFileStatus::Untracked => smart_commit_changes == GitSmartCommitChanges::All,
        GitFileStatus::Added | GitFileStatus::Conflicted => false,
    }
}

pub(crate) fn source_control_status_marker(status: GitFileStatus) -> &'static str {
    match status {
        GitFileStatus::Modified => "M",
        GitFileStatus::Added => "A",
        GitFileStatus::Deleted => "D",
        GitFileStatus::Renamed => "R",
        GitFileStatus::Untracked => "?",
        GitFileStatus::Conflicted => "!",
    }
}

pub(crate) fn source_control_status_label(status: GitFileStatus) -> &'static str {
    match status {
        GitFileStatus::Modified => "Modified",
        GitFileStatus::Added => "Added",
        GitFileStatus::Deleted => "Deleted",
        GitFileStatus::Renamed => "Renamed",
        GitFileStatus::Untracked => "Untracked",
        GitFileStatus::Conflicted => "Conflicted",
    }
}

fn source_control_status_commit_label(status: GitFileStatus) -> &'static str {
    match status {
        GitFileStatus::Modified => "modified",
        GitFileStatus::Added => "added",
        GitFileStatus::Deleted => "deleted",
        GitFileStatus::Renamed => "renamed",
        GitFileStatus::Untracked => "untracked",
        GitFileStatus::Conflicted => "conflicted",
    }
}

pub(super) fn source_control_status_color(status: GitFileStatus) -> Color32 {
    match status {
        GitFileStatus::Added | GitFileStatus::Untracked => Color32::from_rgb(89, 168, 105),
        GitFileStatus::Deleted => Color32::from_rgb(224, 108, 117),
        GitFileStatus::Renamed => Color32::from_rgb(86, 156, 214),
        GitFileStatus::Conflicted => Color32::from_rgb(244, 191, 117),
        GitFileStatus::Modified => Color32::from_rgb(220, 220, 170),
    }
}
