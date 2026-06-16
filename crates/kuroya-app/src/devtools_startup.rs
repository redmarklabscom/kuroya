use crate::{path_display::sanitized_display_label_cow, ui_text::count_label};
use eframe::egui::{self, RichText};
use std::{borrow::Cow, time::Duration, time::Instant};

pub(crate) const MAX_STARTUP_TIMING_ENTRIES: usize = 32;
pub(crate) const MAX_STARTUP_TIMING_LABEL_CHARS: usize = 80;
pub(crate) const MAX_STARTUP_TIMING_MS: f32 = 3_600_000.0;
const DEVTOOLS_MONOSPACE_ROW_HEIGHT: f32 = 18.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StartupTimingEntry {
    pub(crate) label: String,
    pub(crate) stage_ms: f32,
    pub(crate) total_ms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StartupTimingSummary<'a> {
    pub(crate) stage_count: usize,
    pub(crate) total_ms: f32,
    pub(crate) slowest_stage_label: &'a str,
    pub(crate) slowest_stage_ms: f32,
}

pub(crate) struct StartupProfiler {
    started_at: Instant,
    last_at: Instant,
    entries: Vec<StartupTimingEntry>,
}

impl StartupProfiler {
    pub(crate) fn start(now: Instant) -> Self {
        Self {
            started_at: now,
            last_at: now,
            entries: Vec::new(),
        }
    }

    pub(crate) fn record(&mut self, label: impl Into<String>) {
        let now = Instant::now();
        let entry = StartupTimingEntry {
            label: label.into(),
            stage_ms: duration_ms(now.saturating_duration_since(self.last_at)),
            total_ms: duration_ms(now.saturating_duration_since(self.started_at)),
        };
        record_startup_timing_entry(&mut self.entries, entry, MAX_STARTUP_TIMING_ENTRIES);
        self.last_at = now;
    }

    pub(crate) fn into_entries(self) -> Vec<StartupTimingEntry> {
        self.entries
    }
}

pub(crate) fn record_startup_timing_entry(
    entries: &mut Vec<StartupTimingEntry>,
    mut entry: StartupTimingEntry,
    max_entries: usize,
) {
    if max_entries == 0 {
        entries.clear();
        return;
    }
    normalize_startup_timing_entry(&mut entry);
    entries.push(entry);
    let overflow = entries.len().saturating_sub(max_entries);
    if overflow > 0 {
        entries.drain(0..overflow);
    }
}

pub(crate) fn startup_timing_summary(
    entries: &[StartupTimingEntry],
) -> Option<StartupTimingSummary<'_>> {
    let last = entries.last()?;
    let slowest = entries.iter().max_by(|a, b| {
        bounded_startup_timing_ms(a.stage_ms).total_cmp(&bounded_startup_timing_ms(b.stage_ms))
    })?;

    Some(StartupTimingSummary {
        stage_count: entries.len(),
        total_ms: bounded_startup_timing_ms(last.total_ms),
        slowest_stage_label: slowest.label.as_str(),
        slowest_stage_ms: bounded_startup_timing_ms(slowest.stage_ms),
    })
}

pub(crate) fn render_startup_timing_panel(ui: &mut egui::Ui, entries: &[StartupTimingEntry]) {
    ui.label(RichText::new("Startup Timing").strong());
    if entries.is_empty() {
        ui.label(RichText::new("No startup samples recorded").small());
        return;
    }
    if let Some(summary) = startup_timing_summary(entries) {
        ui.horizontal(|ui| {
            ui.label(count_label(summary.stage_count, "stage", "stages"));
            ui.label(format!("Total {:.1} ms", summary.total_ms));
            ui.label(format!(
                "Slowest {} {:.1} ms",
                summary.slowest_stage_label, summary.slowest_stage_ms
            ));
        });
    }

    let row_height = startup_timing_row_height(ui);
    egui::ScrollArea::vertical()
        .max_height(132.0)
        .auto_shrink([false, false])
        .show_rows(ui, row_height, entries.len(), |ui, rows| {
            for index in rows {
                if let Some(entry) = entries.get(index) {
                    ui.monospace(startup_timing_row_text(entry));
                }
            }
        });
}

fn startup_timing_row_height(ui: &egui::Ui) -> f32 {
    ui.spacing()
        .interact_size
        .y
        .max(DEVTOOLS_MONOSPACE_ROW_HEIGHT)
}

fn startup_timing_row_text(entry: &StartupTimingEntry) -> String {
    format!(
        "{:<24} +{:>6.1} ms  total {:>6.1} ms",
        entry.label,
        bounded_startup_timing_ms(entry.stage_ms),
        bounded_startup_timing_ms(entry.total_ms)
    )
}

fn duration_ms(duration: Duration) -> f32 {
    bounded_startup_timing_ms(duration.as_secs_f32() * 1000.0)
}

fn normalize_startup_timing_entry(entry: &mut StartupTimingEntry) {
    let normalized_label = match bounded_startup_timing_label_cow(&entry.label) {
        Cow::Borrowed(_) => None,
        Cow::Owned(label) => Some(label),
    };
    if let Some(label) = normalized_label {
        entry.label = label;
    }
    entry.stage_ms = bounded_startup_timing_ms(entry.stage_ms);
    entry.total_ms = bounded_startup_timing_ms(entry.total_ms);
}

#[cfg(test)]
fn bounded_startup_timing_label(value: &str) -> String {
    bounded_startup_timing_label_cow(value).into_owned()
}

fn bounded_startup_timing_label_cow(value: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(value, MAX_STARTUP_TIMING_LABEL_CHARS, "startup stage")
}

fn bounded_startup_timing_ms(metric_ms: f32) -> f32 {
    if metric_ms.is_nan() {
        0.0
    } else {
        metric_ms.clamp(0.0, MAX_STARTUP_TIMING_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_STARTUP_TIMING_ENTRIES, MAX_STARTUP_TIMING_LABEL_CHARS, MAX_STARTUP_TIMING_MS,
        StartupProfiler, StartupTimingEntry, StartupTimingSummary, bounded_startup_timing_label,
        bounded_startup_timing_label_cow, record_startup_timing_entry, startup_timing_row_text,
        startup_timing_summary,
    };
    use std::{borrow::Cow, time::Instant};

    #[test]
    fn startup_timing_entries_are_bounded() {
        let mut entries = Vec::new();
        for index in 0..(MAX_STARTUP_TIMING_ENTRIES + 2) {
            record_startup_timing_entry(
                &mut entries,
                StartupTimingEntry {
                    label: format!("stage-{index}"),
                    stage_ms: index as f32,
                    total_ms: index as f32,
                },
                MAX_STARTUP_TIMING_ENTRIES,
            );
        }

        assert_eq!(entries.len(), MAX_STARTUP_TIMING_ENTRIES);
        assert_eq!(entries.first().unwrap().label, "stage-2");

        record_startup_timing_entry(
            &mut entries,
            StartupTimingEntry {
                label: "clear".to_owned(),
                stage_ms: 0.0,
                total_ms: 0.0,
            },
            0,
        );
        assert!(entries.is_empty());
    }

    #[test]
    fn startup_timing_entries_normalize_label_and_metric_bounds() {
        let mut entries = Vec::new();
        record_startup_timing_entry(
            &mut entries,
            StartupTimingEntry {
                label: format!(
                    "Load\nSettings \u{202e}{}",
                    "stage-fragment-".repeat(MAX_STARTUP_TIMING_LABEL_CHARS)
                ),
                stage_ms: f32::NAN,
                total_ms: f32::INFINITY,
            },
            4,
        );

        let entry = entries.first().expect("startup timing should be stored");
        assert!(!entry.label.contains('\n'));
        assert!(!entry.label.contains('\u{202e}'));
        assert!(entry.label.contains("Load Settings"));
        assert!(entry.label.contains("..."));
        assert!(entry.label.chars().count() <= MAX_STARTUP_TIMING_LABEL_CHARS);
        assert_eq!(entry.stage_ms, 0.0);
        assert_eq!(entry.total_ms, MAX_STARTUP_TIMING_MS);
    }

    #[test]
    fn startup_timing_label_cow_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            bounded_startup_timing_label_cow("runtime"),
            Cow::Borrowed("runtime")
        ));

        let unicode = "runtime-\u{03bb}";
        match bounded_startup_timing_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed startup timing label, got {label:?}"),
        }
    }

    #[test]
    fn startup_timing_label_cow_owns_dirty_truncated_and_fallback_output() {
        let dirty = assert_owned_startup_timing_label("Load\nSettings \u{202e}done");
        assert_eq!(dirty, "Load Settings done");

        let truncated = assert_owned_startup_timing_label(&format!(
            "stage-{}",
            "fragment-".repeat(MAX_STARTUP_TIMING_LABEL_CHARS)
        ));
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= MAX_STARTUP_TIMING_LABEL_CHARS);

        let fallback = assert_owned_startup_timing_label("\n\u{202e}");
        assert_eq!(fallback, "startup stage");
    }

    #[test]
    fn startup_timing_label_string_wrapper_matches_cow_helper() {
        for value in [
            "runtime",
            "runtime-\u{03bb}",
            " Load Settings ",
            "Load\nSettings \u{202e}done",
            "\n\u{202e}",
        ] {
            assert_eq!(
                bounded_startup_timing_label(value),
                bounded_startup_timing_label_cow(value).into_owned()
            );
        }

        let long_label = format!(
            "stage-{}",
            "fragment-".repeat(MAX_STARTUP_TIMING_LABEL_CHARS)
        );
        assert_eq!(
            bounded_startup_timing_label(&long_label),
            bounded_startup_timing_label_cow(&long_label).into_owned()
        );
    }

    fn assert_owned_startup_timing_label(value: &str) -> String {
        match bounded_startup_timing_label_cow(value) {
            Cow::Owned(label) => label,
            Cow::Borrowed(label) => panic!("expected owned startup timing label, got {label:?}"),
        }
    }

    #[test]
    fn startup_profiler_records_ordered_stage_labels() {
        let mut profiler = StartupProfiler::start(Instant::now());
        profiler.record("runtime");
        profiler.record("settings");

        let entries = profiler.into_entries();
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["runtime", "settings"]
        );
        assert!(entries.iter().all(|entry| entry.stage_ms >= 0.0));
        assert!(
            entries
                .windows(2)
                .all(|pair| pair[1].total_ms >= pair[0].total_ms)
        );
    }

    #[test]
    fn startup_timing_row_text_bounds_metrics() {
        let row = startup_timing_row_text(&StartupTimingEntry {
            label: "runtime".to_owned(),
            stage_ms: f32::NAN,
            total_ms: f32::INFINITY,
        });

        assert!(row.starts_with("runtime"));
        assert!(row.contains("+   0.0 ms"));
        assert!(row.contains("total 3600000.0 ms"));
    }

    #[test]
    fn startup_timing_summary_reports_total_and_slowest_stage() {
        let entries = vec![
            StartupTimingEntry {
                label: "runtime".to_owned(),
                stage_ms: 2.0,
                total_ms: 2.0,
            },
            StartupTimingEntry {
                label: "settings".to_owned(),
                stage_ms: 8.5,
                total_ms: 10.5,
            },
            StartupTimingEntry {
                label: "watcher".to_owned(),
                stage_ms: 3.0,
                total_ms: 13.5,
            },
        ];

        assert_eq!(
            startup_timing_summary(&entries),
            Some(StartupTimingSummary {
                stage_count: 3,
                total_ms: 13.5,
                slowest_stage_label: "settings",
                slowest_stage_ms: 8.5,
            })
        );
    }

    #[test]
    fn startup_timing_summary_bounds_legacy_metric_values() {
        let entries = vec![
            StartupTimingEntry {
                label: "nan".to_owned(),
                stage_ms: f32::NAN,
                total_ms: f32::NAN,
            },
            StartupTimingEntry {
                label: "huge".to_owned(),
                stage_ms: f32::INFINITY,
                total_ms: f32::INFINITY,
            },
        ];

        assert_eq!(
            startup_timing_summary(&entries),
            Some(StartupTimingSummary {
                stage_count: 2,
                total_ms: MAX_STARTUP_TIMING_MS,
                slowest_stage_label: "huge",
                slowest_stage_ms: MAX_STARTUP_TIMING_MS,
            })
        );
    }

    #[test]
    fn startup_timing_summary_is_empty_without_entries() {
        assert_eq!(startup_timing_summary(&[]), None);
    }
}
