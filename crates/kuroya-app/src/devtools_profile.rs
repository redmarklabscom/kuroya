use crate::{
    KuroyaApp,
    devtools_trace_id::next_devtools_trace_id,
    ui_text::{count_label, truncate_middle},
};
use eframe::egui::{self, RichText};
use std::{collections::VecDeque, time::Duration, time::Instant};

pub(crate) const MAX_PROFILE_SAMPLES: usize = 240;
pub(crate) const SLOW_PROFILE_SAMPLE_MS: f32 = 16.7;
const DEVTOOLS_MONOSPACE_ROW_HEIGHT: f32 = 18.0;
const PROFILE_CATEGORY_MAX_CHARS: usize = 24;
const PROFILE_NAME_MAX_CHARS: usize = 96;
const PROFILE_DURATION_MAX_MS: f32 = 60_000.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProfileSample {
    pub(crate) id: u64,
    pub(crate) category: String,
    pub(crate) name: String,
    pub(crate) duration_ms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ProfileSampleStats {
    pub(crate) sample_count: usize,
    pub(crate) average_ms: f32,
    pub(crate) max_ms: f32,
    pub(crate) slow_sample_count: usize,
}

impl KuroyaApp {
    pub(crate) fn profiling_enabled(&self) -> bool {
        cfg!(debug_assertions) && self.settings.devtools_profiling_enabled
    }

    pub(crate) fn record_profile_sample(
        &mut self,
        category: impl Into<String>,
        name: impl Into<String>,
        duration: Duration,
    ) {
        if !self.profiling_enabled() {
            return;
        }
        self.record_profile_sample_enabled(category, name, duration);
    }

    fn record_profile_sample_enabled(
        &mut self,
        category: impl Into<String>,
        name: impl Into<String>,
        duration: Duration,
    ) {
        let id = next_devtools_trace_id(&mut self.next_profile_sample_id);
        record_profile_sample_entry(
            &mut self.profile_samples,
            ProfileSample {
                id,
                category: category.into(),
                name: name.into(),
                duration_ms: duration_ms(duration),
            },
            MAX_PROFILE_SAMPLES,
        );
    }

    pub(crate) fn record_profile_mark(
        &mut self,
        enabled: bool,
        marker: &mut Instant,
        category: &'static str,
        name: &'static str,
    ) {
        if !enabled {
            return;
        }
        let now = Instant::now();
        let duration = now.saturating_duration_since(*marker);
        *marker = now;
        self.record_profile_sample_enabled(category, name, duration);
    }
}

pub(crate) fn record_profile_sample_entry(
    entries: &mut VecDeque<ProfileSample>,
    mut entry: ProfileSample,
    max_entries: usize,
) {
    if max_entries == 0 {
        entries.clear();
        return;
    }
    normalize_profile_sample_entry(&mut entry);
    entries.push_back(entry);
    while entries.len() > max_entries {
        entries.pop_front();
    }
}

pub(crate) fn profile_sample_stats(
    entries: &VecDeque<ProfileSample>,
) -> Option<ProfileSampleStats> {
    if entries.is_empty() {
        return None;
    }

    let mut total_ms = 0.0;
    let mut max_ms = 0.0_f32;
    let mut slow_sample_count = 0usize;
    for entry in entries {
        let duration_ms = normalized_profile_duration_ms(entry.duration_ms);
        total_ms += duration_ms;
        max_ms = max_ms.max(duration_ms);
        if duration_ms > SLOW_PROFILE_SAMPLE_MS {
            slow_sample_count += 1;
        }
    }

    Some(ProfileSampleStats {
        sample_count: entries.len(),
        average_ms: total_ms / entries.len() as f32,
        max_ms,
        slow_sample_count,
    })
}

pub(crate) fn render_profile_panel(
    ui: &mut egui::Ui,
    enabled: bool,
    entries: &VecDeque<ProfileSample>,
) {
    ui.label(RichText::new("Profiling").strong());
    if !cfg!(debug_assertions) {
        ui.label(RichText::new("Unavailable in release builds").small());
        return;
    }
    if !enabled {
        ui.label(RichText::new("Disabled").small());
        return;
    }
    if entries.is_empty() {
        ui.label(RichText::new("No profile samples recorded yet").small());
        return;
    }
    if let Some(stats) = profile_sample_stats(entries) {
        ui.horizontal(|ui| {
            ui.label(count_label(stats.sample_count, "sample", "samples"));
            ui.label(format!("Avg {:.2} ms", stats.average_ms));
            ui.label(format!("Max {:.2} ms", stats.max_ms));
            ui.label(format!(
                "{} slow",
                count_label(stats.slow_sample_count, "sample", "samples")
            ));
        });
    }

    let row_height = devtools_row_height(ui);
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .auto_shrink([false, false])
        .show_rows(ui, row_height, entries.len(), |ui, rows| {
            let len = entries.len();
            for display_index in rows {
                if let Some(entry) = reversed_vec_deque_item(entries, len, display_index) {
                    ui.monospace(profile_sample_row_text(entry));
                }
            }
        });
}

fn devtools_row_height(ui: &egui::Ui) -> f32 {
    ui.spacing()
        .interact_size
        .y
        .max(DEVTOOLS_MONOSPACE_ROW_HEIGHT)
}

fn reversed_vec_deque_item<T>(items: &VecDeque<T>, len: usize, display_index: usize) -> Option<&T> {
    len.checked_sub(display_index + 1)
        .and_then(|index| items.get(index))
}

fn profile_sample_row_text(entry: &ProfileSample) -> String {
    let category = profile_display_label(&entry.category, PROFILE_CATEGORY_MAX_CHARS);
    let name = profile_display_label(&entry.name, PROFILE_NAME_MAX_CHARS);
    format!(
        "#{:04} [{:<8}] {:<16} {:>6.2} ms",
        entry.id,
        category,
        name,
        normalized_profile_duration_ms(entry.duration_ms)
    )
}

fn normalize_profile_sample_entry(entry: &mut ProfileSample) {
    entry.category = profile_display_label(&entry.category, PROFILE_CATEGORY_MAX_CHARS);
    entry.name = profile_display_label(&entry.name, PROFILE_NAME_MAX_CHARS);
    entry.duration_ms = normalized_profile_duration_ms(entry.duration_ms);
}

fn profile_display_label(raw: &str, max_chars: usize) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;
    for ch in raw.chars().take(max_chars.saturating_mul(4).max(1)) {
        if is_hidden_format_control(ch) {
            continue;
        }
        if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
            if !previous_was_space {
                normalized.push(' ');
                previous_was_space = true;
            }
            continue;
        }
        normalized.push(ch);
        previous_was_space = ch.is_whitespace();
    }
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return "profile".to_owned();
    }
    truncate_middle(trimmed, max_chars)
}

fn is_hidden_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn normalized_profile_duration_ms(duration_ms: f32) -> f32 {
    if duration_ms.is_finite() {
        duration_ms.clamp(0.0, PROFILE_DURATION_MAX_MS)
    } else {
        0.0
    }
}

fn duration_ms(duration: Duration) -> f32 {
    duration.as_secs_f32() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::{
        PROFILE_CATEGORY_MAX_CHARS, PROFILE_DURATION_MAX_MS, PROFILE_NAME_MAX_CHARS, ProfileSample,
        ProfileSampleStats, is_hidden_format_control, profile_sample_row_text,
        profile_sample_stats, record_profile_sample_entry,
    };
    use std::collections::VecDeque;

    #[test]
    fn profile_samples_are_bounded() {
        let mut entries = VecDeque::new();
        record_profile_sample_entry(
            &mut entries,
            ProfileSample {
                id: 1,
                category: "frame".to_owned(),
                name: "setup".to_owned(),
                duration_ms: 1.0,
            },
            2,
        );
        record_profile_sample_entry(
            &mut entries,
            ProfileSample {
                id: 2,
                category: "command".to_owned(),
                name: "Open File".to_owned(),
                duration_ms: 2.0,
            },
            2,
        );
        record_profile_sample_entry(
            &mut entries,
            ProfileSample {
                id: 3,
                category: "async".to_owned(),
                name: "Index Workspace".to_owned(),
                duration_ms: 3.0,
            },
            2,
        );

        assert_eq!(
            entries
                .iter()
                .map(|entry| (entry.id, entry.category.as_str(), entry.name.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "command", "Open File"), (3, "async", "Index Workspace")]
        );

        record_profile_sample_entry(
            &mut entries,
            ProfileSample {
                id: 4,
                category: "frame".to_owned(),
                name: "clear".to_owned(),
                duration_ms: 0.0,
            },
            0,
        );
        assert!(entries.is_empty());
    }

    #[test]
    fn profile_sample_stats_summarize_recent_samples() {
        let entries = VecDeque::from([
            ProfileSample {
                id: 1,
                category: "frame".to_owned(),
                name: "setup".to_owned(),
                duration_ms: 4.0,
            },
            ProfileSample {
                id: 2,
                category: "frame".to_owned(),
                name: "render".to_owned(),
                duration_ms: 22.0,
            },
            ProfileSample {
                id: 3,
                category: "async".to_owned(),
                name: "Index Workspace".to_owned(),
                duration_ms: 10.0,
            },
        ]);

        assert_eq!(
            profile_sample_stats(&entries),
            Some(ProfileSampleStats {
                sample_count: 3,
                average_ms: 12.0,
                max_ms: 22.0,
                slow_sample_count: 1,
            })
        );
    }

    #[test]
    fn profile_sample_stats_are_empty_without_samples() {
        assert_eq!(profile_sample_stats(&VecDeque::new()), None);
    }

    #[test]
    fn profile_samples_sanitize_and_bound_display_metadata() {
        let mut entries = VecDeque::new();
        record_profile_sample_entry(
            &mut entries,
            ProfileSample {
                id: 1,
                category: format!("frame\u{202e}\u{061c}\n{}", "category".repeat(16)),
                name: format!(
                    "render\u{200b}\u{2060}\u{feff}\u{2028}\t{}",
                    "name".repeat(64)
                ),
                duration_ms: f32::INFINITY,
            },
            4,
        );

        let entry = entries.front().expect("sample should be recorded");
        assert!(entry.category.chars().count() <= PROFILE_CATEGORY_MAX_CHARS);
        assert!(entry.name.chars().count() <= PROFILE_NAME_MAX_CHARS);
        assert!(!entry.category.contains('\n'));
        assert!(!entry.name.contains('\t'));
        assert!(!entry.category.contains('\u{202e}'));
        assert!(!entry.category.contains('\u{061c}'));
        assert!(!entry.name.contains('\u{200b}'));
        assert!(!entry.name.contains('\u{2060}'));
        assert!(!entry.name.contains('\u{feff}'));
        assert!(!entry.name.contains('\u{2028}'));
        assert_eq!(entry.duration_ms, 0.0);
    }

    #[test]
    fn profile_sample_row_text_bounds_legacy_display_values() {
        let raw_category = "\n\u{202e}".to_owned();
        let raw_name = format!(
            "render\u{200b}\nsetup\t{}",
            "name-fragment-".repeat(PROFILE_NAME_MAX_CHARS)
        );
        let entry = ProfileSample {
            id: 7,
            category: raw_category.clone(),
            name: raw_name.clone(),
            duration_ms: PROFILE_DURATION_MAX_MS * 4.0,
        };

        let row = profile_sample_row_text(&entry);

        assert!(row.starts_with("#0007 [profile "));
        assert!(row.contains("render setup"));
        assert!(row.contains("..."));
        assert!(row.contains("60000.00 ms"));
        assert!(
            !row.chars()
                .any(|ch| ch.is_control() || is_hidden_format_control(ch)),
            "profile row should not include control or hidden format characters: {row:?}"
        );
        assert_eq!(entry.category, raw_category);
        assert_eq!(entry.name, raw_name);
    }

    #[test]
    fn profile_sample_stats_clamp_non_finite_and_negative_durations() {
        let entries = VecDeque::from([
            ProfileSample {
                id: 1,
                category: "frame".to_owned(),
                name: "nan".to_owned(),
                duration_ms: f32::NAN,
            },
            ProfileSample {
                id: 2,
                category: "frame".to_owned(),
                name: "negative".to_owned(),
                duration_ms: -100.0,
            },
            ProfileSample {
                id: 3,
                category: "frame".to_owned(),
                name: "huge".to_owned(),
                duration_ms: PROFILE_DURATION_MAX_MS * 4.0,
            },
        ]);

        let stats = profile_sample_stats(&entries).expect("samples should produce stats");

        assert_eq!(stats.sample_count, 3);
        assert_eq!(stats.max_ms, PROFILE_DURATION_MAX_MS);
        assert_eq!(stats.slow_sample_count, 1);
    }
}
