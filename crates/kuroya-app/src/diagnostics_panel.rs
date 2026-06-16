use crate::{
    KuroyaApp,
    diagnostic_location::diagnostic_jump_location,
    lsp_labels::{diagnostic_message_summary, severity_label},
    path_display::{sanitized_display_label_cow, sanitized_owned_display_label},
    theme::diagnostic_color,
    ui_icons::{IconKind, icon_label},
    ui_state::{
        handle_list_navigation_keys, plain_key_pressed, selected_row_scroll_offset,
        selection_page_step,
    },
};
use eframe::egui::{self, Align, Key, RichText, ScrollArea};
use kuroya_core::{Diagnostic, DiagnosticSet};
use std::{borrow::Cow, collections::HashMap, fmt::Write as _, ops::Range, path::Path};

#[cfg(test)]
use crate::path_display::compact_path;
#[cfg(test)]
use kuroya_core::DiagnosticSeverity;

const MAX_DIAGNOSTIC_PATH_LABEL_CHARS: usize = 80;
const MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES: usize = 128;
const MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES: usize = 256;

impl KuroyaApp {
    pub(crate) fn render_diagnostics_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            icon_label(
                ui,
                IconKind::Diagnostics,
                ui.visuals().widgets.inactive.fg_stroke.color,
                "Diagnostics",
            );
            ui.label(RichText::new("Diagnostics").strong());
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(diagnostic_panel_summary_label(&self.diagnostics)).small());
            });
        });
        ui.separator();

        let diagnostic_count = self.diagnostics.len();
        if diagnostic_count == 0 {
            self.diagnostics_panel_selected = 0;
            ui.add_space(24.0);
            ui.centered_and_justified(|ui| {
                icon_label(
                    ui,
                    IconKind::Diagnostics,
                    ui.visuals().widgets.inactive.fg_stroke.color,
                    "No diagnostics",
                );
                ui.label(RichText::new("No diagnostics").small());
            });
            return;
        }

        let mut open_target = None;
        let row_height = ui.spacing().interact_size.y.max(22.0);
        let viewport_height = ui.available_height();
        let focus_id = ui.make_persistent_id("diagnostics-panel-rows");
        let rows_focused = ui.memory(|memory| memory.has_focus(focus_id));
        let mut selected_index = diagnostic_panel_normalized_selection(
            self.diagnostics_panel_selected,
            diagnostic_count,
        );
        let mut scroll_to_selection = false;
        let mut keyboard_open_index = None;
        let mut keyboard_open_row_rendered = false;
        let mut row_click_open_attempted = false;

        if rows_focused {
            scroll_to_selection = ui.input(|input| {
                handle_list_navigation_keys(
                    input,
                    &mut selected_index,
                    diagnostic_count,
                    selection_page_step(row_height, viewport_height),
                )
            });
            if ui.input(|input| plain_key_pressed(input, Key::Enter)) {
                keyboard_open_index = Some(selected_index);
            }
        }

        let mut scroll_area = ScrollArea::vertical();
        if scroll_to_selection {
            scroll_area = scroll_area.vertical_scroll_offset(selected_row_scroll_offset(
                selected_index,
                diagnostic_count,
                row_height,
                viewport_height,
            ));
        }
        scroll_area.show_rows(ui, row_height, diagnostic_count, |ui, rows| {
            diagnostic_panel_for_each_visible_row(&self.diagnostics, rows, |row| {
                let DiagnosticPanelVisibleRow {
                    index,
                    diagnostic,
                    label,
                } = row;
                if diagnostic_panel_keyboard_open_matches_row(keyboard_open_index, index) {
                    keyboard_open_row_rendered = true;
                    if diagnostic_panel_keyboard_open_can_use_rendered_row(
                        open_target.is_some(),
                        row_click_open_attempted,
                    ) {
                        open_target = self.diagnostic_open_target(diagnostic);
                    }
                }
                if ui
                    .selectable_label(
                        index == selected_index,
                        RichText::new(label).color(diagnostic_color(diagnostic.severity)),
                    )
                    .clicked()
                {
                    ui.memory_mut(|memory| memory.request_focus(focus_id));
                    selected_index = index;
                    row_click_open_attempted = true;
                    open_target = self.diagnostic_open_target(diagnostic);
                }
            });
        });
        if let Some(index) = diagnostic_panel_keyboard_open_fallback_index(
            keyboard_open_index,
            keyboard_open_row_rendered,
            row_click_open_attempted,
            diagnostic_count,
            self.diagnostics.len(),
        ) {
            if let Some(diagnostic) = self.diagnostics.get_sorted(index) {
                open_target = self.diagnostic_open_target(diagnostic);
            }
        }
        self.diagnostics_panel_selected = selected_index;
        if let Some(target) = open_target {
            self.open_resolved_diagnostic_target(target);
        }
    }
}

struct DiagnosticPanelVisibleRow<'a> {
    index: usize,
    diagnostic: &'a Diagnostic,
    label: String,
}

fn diagnostic_panel_for_each_visible_row<'a>(
    diagnostics: &'a DiagnosticSet,
    rows: Range<usize>,
    render_row: impl FnMut(DiagnosticPanelVisibleRow<'a>),
) {
    let rows = diagnostic_panel_sanitized_visible_rows(rows, diagnostics.len());
    diagnostic_panel_for_each_sanitized_visible_row(diagnostics, rows, render_row);
}

fn diagnostic_panel_for_each_sanitized_visible_row<'a>(
    diagnostics: &'a DiagnosticSet,
    rows: Range<usize>,
    mut render_row: impl FnMut(DiagnosticPanelVisibleRow<'a>),
) {
    let visible_row_count = diagnostic_panel_visible_row_count(rows.clone());
    let mut path_cache = DiagnosticPanelPathLabelCache::with_capacity(visible_row_count);
    let mut message_cache = DiagnosticPanelMessageSummaryCache::with_capacity(visible_row_count);

    for (index, diagnostic) in diagnostics.sorted_range(rows) {
        render_row(DiagnosticPanelVisibleRow {
            index,
            diagnostic,
            label: diagnostic_panel_cached_row_label(
                diagnostic,
                &mut path_cache,
                &mut message_cache,
            ),
        });
    }
}

pub(crate) fn diagnostic_panel_normalized_selection(
    selection: usize,
    diagnostic_count: usize,
) -> usize {
    if diagnostic_count == 0 {
        0
    } else {
        selection.min(diagnostic_count - 1)
    }
}

fn diagnostic_panel_keyboard_open_matches_row(open_index: Option<usize>, row_index: usize) -> bool {
    open_index == Some(row_index)
}

fn diagnostic_panel_keyboard_open_can_use_rendered_row(
    has_open_target: bool,
    row_click_open_attempted: bool,
) -> bool {
    !has_open_target && !row_click_open_attempted
}

fn diagnostic_panel_keyboard_open_fallback_index(
    open_index: Option<usize>,
    selected_row_rendered: bool,
    row_click_open_attempted: bool,
    rendered_diagnostic_count: usize,
    current_diagnostic_count: usize,
) -> Option<usize> {
    if selected_row_rendered
        || row_click_open_attempted
        || rendered_diagnostic_count != current_diagnostic_count
    {
        None
    } else {
        open_index
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
struct DiagnosticPanelPreparedRow {
    index: usize,
    severity: DiagnosticSeverity,
    label: String,
}

#[cfg(test)]
fn diagnostic_panel_prepare_visible_rows(
    diagnostics: &DiagnosticSet,
    rows: Range<usize>,
) -> Vec<DiagnosticPanelPreparedRow> {
    let rows = diagnostic_panel_sanitized_visible_rows(rows, diagnostics.len());
    let visible_row_count = diagnostic_panel_visible_row_count(rows.clone());
    let mut display_rows = Vec::with_capacity(visible_row_count);
    diagnostic_panel_for_each_sanitized_visible_row(diagnostics, rows, |row| {
        display_rows.push(DiagnosticPanelPreparedRow {
            index: row.index,
            severity: row.diagnostic.severity,
            label: row.label,
        });
    });
    display_rows
}

fn diagnostic_panel_cached_row_label<'a>(
    diagnostic: &'a Diagnostic,
    path_cache: &mut DiagnosticPanelPathLabelCache<'a>,
    message_cache: &mut DiagnosticPanelMessageSummaryCache<'a>,
) -> String {
    let path_label = path_cache.label(diagnostic.path.as_path());
    let message = message_cache.summary(&diagnostic.message);
    diagnostic_panel_row_label_with_parts(diagnostic, path_label, message)
}

fn diagnostic_panel_sanitized_visible_rows(
    rows: Range<usize>,
    diagnostic_count: usize,
) -> Range<usize> {
    let start = rows.start.min(diagnostic_count);
    let end = rows.end.min(diagnostic_count);
    if start <= end {
        start..end
    } else {
        start..start
    }
}

fn diagnostic_panel_visible_row_count(rows: Range<usize>) -> usize {
    rows.end.saturating_sub(rows.start)
}

#[cfg(test)]
pub(crate) fn diagnostic_panel_row_label(diagnostic: &Diagnostic) -> String {
    let path = diagnostic_display_path(&diagnostic.path);
    diagnostic_panel_row_label_with_path(diagnostic, &path)
}

#[cfg(test)]
fn diagnostic_panel_row_label_with_path(diagnostic: &Diagnostic, path: &str) -> String {
    let message = diagnostic_message_summary(&diagnostic.message);
    diagnostic_panel_row_label_with_parts(diagnostic, path, &message)
}

fn diagnostic_panel_row_label_with_parts(
    diagnostic: &Diagnostic,
    path: &str,
    message: &str,
) -> String {
    let severity = severity_label(diagnostic.severity);
    let (line, column) = diagnostic_jump_location(diagnostic);
    let mut label = String::with_capacity(
        path.len()
            + decimal_digit_count(line)
            + decimal_digit_count(column)
            + severity.len()
            + message.len()
            + 6,
    );
    let _ = write!(label, "{path}:{line}:{column}  {severity}  {message}");
    label
}

fn decimal_digit_count(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        value.ilog10() as usize + 1
    }
}

pub(crate) fn diagnostic_display_path(path: &Path) -> String {
    diagnostic_display_path_cow(path).into_owned()
}

fn diagnostic_display_path_cow(path: &Path) -> Cow<'_, str> {
    match diagnostic_compact_path_label(path) {
        Cow::Borrowed(label) => {
            sanitized_display_label_cow(label, MAX_DIAGNOSTIC_PATH_LABEL_CHARS, ".")
        }
        Cow::Owned(label) => Cow::Owned(diagnostic_display_path_from_compact(label)),
    }
}

fn diagnostic_compact_path_label(path: &Path) -> Cow<'_, str> {
    if path.as_os_str().is_empty() {
        return Cow::Borrowed(".");
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .map(Cow::Borrowed)
        .unwrap_or_else(|| Cow::Owned(path.display().to_string()))
}

fn diagnostic_display_path_from_compact(compact: String) -> String {
    sanitized_owned_display_label(compact, MAX_DIAGNOSTIC_PATH_LABEL_CHARS, ".")
}

pub(crate) fn diagnostic_panel_summary_label(diagnostics: &DiagnosticSet) -> Cow<'static, str> {
    if diagnostics.is_empty() {
        return Cow::Borrowed("No diagnostics");
    }

    let counts = diagnostics.severity_counts();
    let mut summary = String::with_capacity(48);
    push_diagnostic_summary_part(&mut summary, counts.errors, "error", "errors");
    push_diagnostic_summary_part(&mut summary, counts.warnings, "warning", "warnings");
    push_diagnostic_summary_part(&mut summary, counts.infos, "info", "info");
    push_diagnostic_summary_part(&mut summary, counts.hints, "hint", "hints");
    Cow::Owned(summary)
}

fn push_diagnostic_summary_part(summary: &mut String, count: usize, singular: &str, plural: &str) {
    if count > 0 {
        if !summary.is_empty() {
            summary.push_str(", ");
        }
        let label = if count == 1 { singular } else { plural };
        let _ = write!(summary, "{count} {label}");
    }
}

#[cfg(test)]
pub(crate) fn diagnostic_panel_open_target(
    diagnostic: &Diagnostic,
) -> (std::path::PathBuf, usize, usize) {
    let (line, column) = diagnostic_jump_location(diagnostic);
    (diagnostic.path.clone(), line, column)
}

#[derive(Default)]
struct DiagnosticPanelPathLabelCache<'a> {
    path_indices: HashMap<&'a Path, usize>,
    labels: Vec<DiagnosticPanelPathLabelCacheEntry<'a>>,
    next_replace_index: usize,
}

struct DiagnosticPanelPathLabelCacheEntry<'a> {
    path: &'a Path,
    label: Cow<'a, str>,
}

impl<'a> DiagnosticPanelPathLabelCache<'a> {
    fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.min(MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES);
        Self {
            path_indices: HashMap::with_capacity(capacity),
            labels: Vec::with_capacity(capacity),
            next_replace_index: 0,
        }
    }

    fn label(&mut self, path: &'a Path) -> &str {
        if let Some(index) = self.path_indices.get(path).copied() {
            return self.labels[index].label.as_ref();
        }

        let label = diagnostic_display_path_cow(path);
        if self.labels.len() < MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES {
            let index = self.labels.len();
            self.labels
                .push(DiagnosticPanelPathLabelCacheEntry { path, label });
            self.path_indices.insert(path, index);
            return self.labels[index].label.as_ref();
        }

        let index = self.next_replace_index;
        self.path_indices.remove(self.labels[index].path);
        self.labels[index] = DiagnosticPanelPathLabelCacheEntry { path, label };
        self.path_indices.insert(path, index);
        self.next_replace_index =
            (self.next_replace_index + 1) % MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES;
        self.labels[index].label.as_ref()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.labels.len()
    }

    #[cfg(test)]
    fn is_cached(&self, path: &Path) -> bool {
        self.path_indices.contains_key(path)
    }

    #[cfg(test)]
    fn label_is_borrowed(&self, path: &Path) -> bool {
        self.path_indices
            .get(path)
            .and_then(|index| self.labels.get(*index))
            .is_some_and(|entry| matches!(entry.label, Cow::Borrowed(_)))
    }
}

#[derive(Default)]
struct DiagnosticPanelMessageSummaryCache<'a> {
    summary_indices: HashMap<&'a str, usize>,
    summaries: Vec<DiagnosticPanelMessageSummaryCacheEntry<'a>>,
    next_replace_index: usize,
}

struct DiagnosticPanelMessageSummaryCacheEntry<'a> {
    message: &'a str,
    summary: String,
}

impl<'a> DiagnosticPanelMessageSummaryCache<'a> {
    fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.min(MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES);
        Self {
            summary_indices: HashMap::with_capacity(capacity),
            summaries: Vec::with_capacity(capacity),
            next_replace_index: 0,
        }
    }

    fn summary(&mut self, message: &'a str) -> &str {
        if let Some(index) = self.summary_indices.get(message).copied() {
            return &self.summaries[index].summary;
        }

        let summary = diagnostic_message_summary(message);
        if self.summaries.len() < MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES {
            let index = self.summaries.len();
            self.summaries
                .push(DiagnosticPanelMessageSummaryCacheEntry { message, summary });
            self.summary_indices.insert(message, index);
            return &self.summaries[index].summary;
        }

        let index = self.next_replace_index;
        self.summary_indices.remove(self.summaries[index].message);
        self.summaries[index] = DiagnosticPanelMessageSummaryCacheEntry { message, summary };
        self.summary_indices.insert(message, index);
        self.next_replace_index =
            (self.next_replace_index + 1) % MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES;
        &self.summaries[index].summary
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.summaries.len()
    }

    #[cfg(test)]
    fn is_cached(&self, message: &str) -> bool {
        self.summary_indices.contains_key(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ops::Range, path::PathBuf};

    fn diagnostic(message: &str, line: usize, column: usize) -> Diagnostic {
        diagnostic_for_path(
            PathBuf::from("workspace/src/main.rs"),
            message,
            line,
            column,
            DiagnosticSeverity::Error,
        )
    }

    fn diagnostic_for_path(
        path: PathBuf,
        message: &str,
        line: usize,
        column: usize,
        severity: DiagnosticSeverity,
    ) -> Diagnostic {
        Diagnostic {
            path,
            line,
            column,
            char_range: Range { start: 0, end: 1 },
            severity,
            source: "rust-analyzer".to_owned(),
            message: message.to_owned(),
            unused: false,
            deprecated: false,
        }
    }

    fn diagnostic_set(diagnostics: Vec<Diagnostic>) -> DiagnosticSet {
        let mut set = DiagnosticSet::default();
        for diagnostic in diagnostics {
            set.replace(diagnostic.path.clone(), vec![diagnostic]);
        }
        set
    }

    fn owned_diagnostic_display_path_from_path(path: &Path) -> String {
        let compact = compact_path(path);
        let sanitized = sanitized_display_label_cow(&compact, MAX_DIAGNOSTIC_PATH_LABEL_CHARS, ".");
        assert!(
            matches!(&sanitized, Cow::Owned(_)),
            "expected owned sanitized label for {compact:?}"
        );
        let expected = sanitized.into_owned();
        assert_eq!(
            diagnostic_display_path_from_compact(compact),
            expected,
            "compact wrapper should match sanitizer output"
        );
        expected
    }

    #[test]
    fn diagnostic_display_path_reuses_clean_ascii_and_unicode_compact_labels() {
        let cases = [
            (
                PathBuf::from("workspace").join("src").join("main.rs"),
                "main.rs",
            ),
            (
                PathBuf::from("workspace")
                    .join("src")
                    .join("clean-\u{03bb}.rs"),
                "clean-\u{03bb}.rs",
            ),
        ];

        for (path, expected) in cases {
            let compact = compact_path(&path);
            assert!(matches!(
                sanitized_display_label_cow(
                    &compact,
                    MAX_DIAGNOSTIC_PATH_LABEL_CHARS,
                    "."
                ),
                Cow::Borrowed(label) if label == expected
            ));
            assert_eq!(diagnostic_display_path(&path), expected);
            assert!(matches!(
                diagnostic_display_path_cow(&path),
                Cow::Borrowed(label) if label == expected
            ));
            assert_eq!(diagnostic_display_path_from_compact(compact), expected);
        }
    }

    #[test]
    fn diagnostic_display_path_owns_dirty_truncated_and_fallback_labels() {
        let dirty_path = PathBuf::from("workspace")
            .join("src")
            .join("bad\nname\u{202e}.rs");
        let dirty_label = owned_diagnostic_display_path_from_path(&dirty_path);
        assert_eq!(dirty_label, "bad name.rs");
        assert!(!dirty_label.contains('\n'));
        assert!(!dirty_label.contains('\u{202e}'));

        let fallback_path = PathBuf::from("\n\u{202e}");
        assert_eq!(owned_diagnostic_display_path_from_path(&fallback_path), ".");

        let truncated_path = PathBuf::from("workspace").join("src").join(format!(
            "prefix-{}-suffix.rs",
            "x".repeat(MAX_DIAGNOSTIC_PATH_LABEL_CHARS * 2)
        ));
        let truncated_label = owned_diagnostic_display_path_from_path(&truncated_path);
        assert!(truncated_label.contains("..."));
        assert!(truncated_label.chars().count() <= MAX_DIAGNOSTIC_PATH_LABEL_CHARS);
        assert!(!truncated_label.chars().any(char::is_control));
    }

    #[test]
    fn diagnostic_path_label_cache_matches_display_path_for_sanitized_labels() {
        let paths = [
            PathBuf::from("workspace").join("src").join("main.rs"),
            PathBuf::from("workspace")
                .join("src")
                .join("clean-\u{03bb}.rs"),
            PathBuf::from("workspace")
                .join("src")
                .join("bad\nname\u{202e}.rs"),
            PathBuf::from("\n\u{202e}"),
            PathBuf::from("workspace").join("src").join(format!(
                "{}.rs",
                "x".repeat(MAX_DIAGNOSTIC_PATH_LABEL_CHARS * 2)
            )),
        ];
        let mut cache = DiagnosticPanelPathLabelCache::default();

        for path in &paths {
            assert_eq!(cache.label(path), diagnostic_display_path(path));
        }
        assert_eq!(cache.len(), paths.len());
        assert!(cache.label_is_borrowed(&paths[0]));
        assert!(cache.label_is_borrowed(&paths[1]));
        assert!(!cache.label_is_borrowed(&paths[2]));
    }

    #[test]
    fn diagnostic_path_label_cache_reuses_equal_paths() {
        let first = PathBuf::from("workspace/src/main.rs");
        let second = PathBuf::from("workspace/src/main.rs");
        let different = PathBuf::from("workspace/src/lib.rs");
        let mut cache = DiagnosticPanelPathLabelCache::default();

        assert_eq!(cache.label(&first), diagnostic_display_path(&first));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.label(&second), diagnostic_display_path(&second));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.label(&different), diagnostic_display_path(&different));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn diagnostic_path_label_cache_bounds_cached_entries_and_preserves_output() {
        let paths = (0..=MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES)
            .map(|idx| PathBuf::from(format!("workspace/src/file_{idx}.rs")))
            .collect::<Vec<_>>();
        let mut cache = DiagnosticPanelPathLabelCache::default();

        for path in paths.iter().take(MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES) {
            assert_eq!(cache.label(path), diagnostic_display_path(path));
            assert!(cache.is_cached(path));
        }
        assert_eq!(cache.len(), MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES);

        let overflow_path = paths.last().expect("paths include one overflow entry");
        assert_eq!(
            cache.label(overflow_path),
            diagnostic_display_path(overflow_path)
        );
        assert_eq!(cache.len(), MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES);
        assert!(cache.is_cached(overflow_path));
        let evicted_path = paths.first().expect("paths include cached entries");
        assert!(!cache.is_cached(evicted_path));

        let cached_path = paths
            .get(1)
            .expect("paths include remaining cached entries");
        assert_eq!(
            cache.label(cached_path),
            diagnostic_display_path(cached_path)
        );
        assert_eq!(cache.len(), MAX_DIAGNOSTIC_PATH_LABEL_CACHE_ENTRIES);
        assert!(cache.is_cached(cached_path));
        assert!(cache.is_cached(overflow_path));
    }

    #[test]
    fn diagnostic_message_summary_cache_reuses_equal_messages() {
        let first = "  unresolved\tname\u{7}\u{202e}\ntry importing it\u{2066}  ".to_owned();
        let second = first.clone();
        let mut cache = DiagnosticPanelMessageSummaryCache::default();

        assert_eq!(cache.summary(&first), "unresolved name try importing it");
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.summary(&second), "unresolved name try importing it");
        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache.summary("different diagnostic"),
            "different diagnostic"
        );
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn diagnostic_message_summary_cache_keeps_repeated_messages_bounded_to_one_entry() {
        let messages = (0..MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES * 2)
            .map(|_| "  unresolved\tname\ntry importing it  ".to_owned())
            .collect::<Vec<_>>();
        let mut cache = DiagnosticPanelMessageSummaryCache::default();

        for message in &messages {
            assert_eq!(cache.summary(message), "unresolved name try importing it");
        }

        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn diagnostic_message_summary_cache_bounds_cached_entries_and_preserves_output() {
        let messages = (0..=MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES)
            .map(|idx| format!("diagnostic {idx}\nsecond line"))
            .collect::<Vec<_>>();
        let mut cache = DiagnosticPanelMessageSummaryCache::default();

        for message in messages
            .iter()
            .take(MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES)
        {
            assert_eq!(cache.summary(message), diagnostic_message_summary(message));
            assert!(cache.is_cached(message));
        }
        assert_eq!(cache.len(), MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES);

        let overflow_message = messages
            .last()
            .expect("messages include one overflow entry");
        assert_eq!(
            cache.summary(overflow_message),
            diagnostic_message_summary(overflow_message)
        );
        assert_eq!(cache.len(), MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES);
        assert!(cache.is_cached(overflow_message));
        let evicted_message = messages.first().expect("messages include cached entries");
        assert!(!cache.is_cached(evicted_message));

        let cached_message = messages
            .get(1)
            .expect("messages include remaining cached entries");
        assert_eq!(
            cache.summary(cached_message),
            diagnostic_message_summary(cached_message)
        );
        assert_eq!(cache.len(), MAX_DIAGNOSTIC_MESSAGE_SUMMARY_CACHE_ENTRIES);
        assert!(cache.is_cached(cached_message));
        assert!(cache.is_cached(overflow_message));
    }

    #[test]
    fn diagnostic_panel_row_label_with_cached_message_matches_uncached_label() {
        let diagnostic = diagnostic("first line\nsecond line", 3, 5);
        let path = diagnostic_display_path(&diagnostic.path);
        let mut cache = DiagnosticPanelMessageSummaryCache::default();
        let message = cache.summary(&diagnostic.message);

        assert_eq!(
            diagnostic_panel_row_label_with_parts(&diagnostic, &path, message),
            diagnostic_panel_row_label(&diagnostic)
        );
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn diagnostic_panel_for_each_visible_row_streams_requested_range_in_order() {
        let diagnostics = diagnostic_set(vec![
            diagnostic_for_path(
                PathBuf::from("workspace/src/a.rs"),
                "first",
                1,
                1,
                DiagnosticSeverity::Error,
            ),
            diagnostic_for_path(
                PathBuf::from("workspace/src/b.rs"),
                "second\nwith detail",
                2,
                4,
                DiagnosticSeverity::Warning,
            ),
            diagnostic_for_path(
                PathBuf::from("workspace/src/c.rs"),
                "third",
                3,
                6,
                DiagnosticSeverity::Info,
            ),
        ]);
        let mut streamed_rows = Vec::new();

        diagnostic_panel_for_each_visible_row(&diagnostics, 1..3, |row| {
            streamed_rows.push((row.index, row.diagnostic.severity, row.label));
        });

        assert_eq!(streamed_rows.len(), 2);
        assert_eq!(streamed_rows[0].0, 1);
        assert_eq!(streamed_rows[0].1, DiagnosticSeverity::Warning);
        assert_eq!(
            streamed_rows[0].2,
            diagnostic_panel_row_label(diagnostics.get_sorted(1).expect("row 1 exists"))
        );
        assert_eq!(streamed_rows[1].0, 2);
        assert_eq!(streamed_rows[1].1, DiagnosticSeverity::Info);
        assert_eq!(
            streamed_rows[1].2,
            diagnostic_panel_row_label(diagnostics.get_sorted(2).expect("row 2 exists"))
        );
    }

    #[test]
    fn diagnostic_panel_prepare_visible_rows_prepares_only_requested_range() {
        let diagnostics = diagnostic_set(vec![
            diagnostic_for_path(
                PathBuf::from("workspace/src/a.rs"),
                "first",
                1,
                1,
                DiagnosticSeverity::Error,
            ),
            diagnostic_for_path(
                PathBuf::from("workspace/src/b.rs"),
                "second\nwith detail",
                2,
                4,
                DiagnosticSeverity::Warning,
            ),
            diagnostic_for_path(
                PathBuf::from("workspace/src/c.rs"),
                "third",
                3,
                6,
                DiagnosticSeverity::Info,
            ),
        ]);

        let rows = diagnostic_panel_prepare_visible_rows(&diagnostics, 1..3);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index, 1);
        assert_eq!(rows[0].severity, DiagnosticSeverity::Warning);
        assert_eq!(
            rows[0].label,
            diagnostic_panel_row_label(diagnostics.get_sorted(1).expect("row 1 exists"))
        );
        assert_eq!(rows[1].index, 2);
        assert_eq!(rows[1].severity, DiagnosticSeverity::Info);
        assert_eq!(
            rows[1].label,
            diagnostic_panel_row_label(diagnostics.get_sorted(2).expect("row 2 exists"))
        );
    }

    #[test]
    fn diagnostic_panel_prepare_visible_rows_clamps_extreme_range_before_reserving() {
        let diagnostics = diagnostic_set(vec![
            diagnostic_for_path(
                PathBuf::from("workspace/src/a.rs"),
                "first",
                1,
                1,
                DiagnosticSeverity::Error,
            ),
            diagnostic_for_path(
                PathBuf::from("workspace/src/b.rs"),
                "second",
                2,
                4,
                DiagnosticSeverity::Warning,
            ),
            diagnostic_for_path(
                PathBuf::from("workspace/src/c.rs"),
                "third",
                3,
                6,
                DiagnosticSeverity::Info,
            ),
        ]);

        let rows = diagnostic_panel_prepare_visible_rows(&diagnostics, 1..usize::MAX);

        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter().map(|row| row.index).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            rows[0].label,
            diagnostic_panel_row_label(diagnostics.get_sorted(1).expect("row 1 exists"))
        );
        assert_eq!(
            rows[1].label,
            diagnostic_panel_row_label(diagnostics.get_sorted(2).expect("row 2 exists"))
        );
    }

    #[test]
    fn diagnostic_panel_prepare_visible_rows_ignores_stale_reversed_range() {
        let diagnostics = diagnostic_set(vec![
            diagnostic_for_path(
                PathBuf::from("workspace/src/a.rs"),
                "first",
                1,
                1,
                DiagnosticSeverity::Error,
            ),
            diagnostic_for_path(
                PathBuf::from("workspace/src/b.rs"),
                "second",
                2,
                4,
                DiagnosticSeverity::Warning,
            ),
        ]);

        let start = 1;
        let end = 0;
        let rows = diagnostic_panel_prepare_visible_rows(&diagnostics, start..end);

        assert!(rows.is_empty());
    }

    #[test]
    fn diagnostic_panel_prepare_visible_rows_ignores_stale_range_past_current_count() {
        let diagnostics = diagnostic_set(vec![diagnostic_for_path(
            PathBuf::from("workspace/src/a.rs"),
            "first",
            1,
            1,
            DiagnosticSeverity::Error,
        )]);

        let rows = diagnostic_panel_prepare_visible_rows(&diagnostics, usize::MAX - 1..usize::MAX);

        assert!(rows.is_empty());
    }

    #[test]
    fn diagnostic_panel_prepared_rows_preserve_raw_targets_for_opening() {
        let raw_path = PathBuf::from(format!(
            "workspace/src/bad\nname\u{202e}{}.rs",
            "x".repeat(MAX_DIAGNOSTIC_PATH_LABEL_CHARS * 2)
        ));
        let raw_source = "rust-analyzer\nsource\u{202e}".to_owned();
        let raw_message = "bad\nmessage\u{202e}with detail".to_owned();
        let display_path = diagnostic_display_path(&raw_path);
        let diagnostics = diagnostic_set(vec![Diagnostic {
            path: raw_path.clone(),
            line: 7,
            column: 9,
            char_range: 12..34,
            severity: DiagnosticSeverity::Error,
            source: raw_source.clone(),
            message: raw_message.clone(),
            unused: true,
            deprecated: true,
        }]);

        let rows = diagnostic_panel_prepare_visible_rows(&diagnostics, 0..1);
        let row = rows.first().expect("prepared row exists");

        assert_eq!(row.index, 0);
        assert!(!row.label.contains('\n'));
        assert!(!row.label.contains('\u{202e}'));
        assert!(row.label.contains("..."));
        assert!(row.label.starts_with(&format!("{display_path}:7:9")));
        assert_ne!(display_path, raw_path.display().to_string());

        let raw = diagnostics
            .get_sorted(row.index)
            .expect("prepared row index resolves to raw diagnostic");
        assert_eq!(raw.path, raw_path);
        assert_eq!((raw.line, raw.column), (7, 9));
        assert_eq!(raw.char_range, 12..34);
        assert_eq!(raw.source, raw_source);
        assert_eq!(raw.message, raw_message);
        assert!(raw.unused);
        assert!(raw.deprecated);
        assert_eq!(diagnostic_panel_open_target(raw), (raw_path, 7, 9));
    }

    #[test]
    fn diagnostic_panel_prepared_row_index_resolves_current_diagnostic_data() {
        let path = PathBuf::from("workspace/src/main.rs");
        let mut diagnostics = DiagnosticSet::default();
        diagnostics.replace(
            path.clone(),
            vec![diagnostic_for_path(
                path.clone(),
                "original",
                2,
                4,
                DiagnosticSeverity::Error,
            )],
        );

        let rows = diagnostic_panel_prepare_visible_rows(&diagnostics, 0..1);
        let row = rows.first().expect("prepared row exists");
        assert_eq!(
            diagnostics
                .get_sorted(row.index)
                .expect("original diagnostic exists")
                .message,
            "original"
        );

        diagnostics.replace(
            path.clone(),
            vec![diagnostic_for_path(
                path.clone(),
                "replacement",
                2,
                4,
                DiagnosticSeverity::Error,
            )],
        );

        let current = diagnostics
            .get_sorted(row.index)
            .expect("replacement diagnostic exists");
        assert_eq!(current.message, "replacement");
        assert_eq!(diagnostic_panel_open_target(current), (path, 2, 4));
    }

    #[test]
    fn diagnostic_panel_normalized_selection_handles_extreme_counts() {
        assert_eq!(diagnostic_panel_normalized_selection(usize::MAX, 1), 0);
        assert_eq!(diagnostic_panel_normalized_selection(usize::MAX, 2), 1);
        assert_eq!(diagnostic_panel_normalized_selection(usize::MAX, 0), 0);
    }

    #[test]
    fn diagnostic_panel_keyboard_open_falls_back_only_when_selected_row_was_not_rendered() {
        assert!(diagnostic_panel_keyboard_open_matches_row(Some(4), 4));
        assert!(!diagnostic_panel_keyboard_open_matches_row(Some(4), 3));
        assert!(!diagnostic_panel_keyboard_open_matches_row(None, 4));
        assert!(diagnostic_panel_keyboard_open_can_use_rendered_row(
            false, false
        ));
        assert!(!diagnostic_panel_keyboard_open_can_use_rendered_row(
            true, false
        ));
        assert!(!diagnostic_panel_keyboard_open_can_use_rendered_row(
            false, true
        ));

        assert_eq!(
            diagnostic_panel_keyboard_open_fallback_index(Some(4), false, false, 8, 8),
            Some(4)
        );
        assert_eq!(
            diagnostic_panel_keyboard_open_fallback_index(Some(4), true, false, 8, 8),
            None
        );
        assert_eq!(
            diagnostic_panel_keyboard_open_fallback_index(Some(4), false, true, 8, 8),
            None
        );
        assert_eq!(
            diagnostic_panel_keyboard_open_fallback_index(Some(4), false, false, 8, 7),
            None
        );
        assert_eq!(
            diagnostic_panel_keyboard_open_fallback_index(None, false, false, 8, 8),
            None
        );
    }
}
