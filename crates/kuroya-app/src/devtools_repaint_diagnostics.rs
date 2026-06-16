use crate::{KuroyaApp, devtools_trace_id::next_devtools_trace_id};
use eframe::egui::{self, RichText};
use std::{
    collections::VecDeque,
    fmt::Write as _,
    time::{Duration, Instant},
};

pub(crate) const MAX_REPAINT_DIAGNOSTIC_SAMPLES: usize = 180;
pub(crate) const FRAME_BUDGET_MS: f32 = 16.7;
pub(crate) const MAX_REPAINT_DIAGNOSTIC_MS: f32 = 3_600_000.0;

const REPAINT_DIAGNOSTIC_VISIBLE_ROWS: usize = 40;
const REPAINT_DIAGNOSTIC_ROW_CAPACITY: usize = 128;

const REPAINT_LABEL_UI_EVENTS: &str = "events";
const REPAINT_LABEL_UI_EVENT_BACKPRESSURE: &str = "backpressure";
const REPAINT_LABEL_TERMINAL_OUTPUT: &str = "terminal";
const REPAINT_LABEL_FILESYSTEM_CHANGES: &str = "fs";
const REPAINT_LABEL_COMMANDS: &str = "commands";
const REPAINT_LABEL_LANGUAGE_SYNC: &str = "sync";
const REPAINT_LABEL_LSP_RESTART: &str = "restart";
const REPAINT_LABEL_LSP_DIAGNOSTICS: &str = "lspdiag";
const REPAINT_LABEL_WORKSPACE_REFRESH: &str = "refresh";
const REPAINT_LABEL_PLUGIN_RELOAD: &str = "plugin";
const REPAINT_LABEL_AUTOSAVE: &str = "autosave";
const REPAINT_LABEL_SESSION_SAVE: &str = "session";
const REPAINT_LABEL_STARTUP_WARMUP: &str = "startup";
const REPAINT_LABEL_SCHEDULED: &str = "scheduled";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepaintCause {
    UiEvents,
    UiEventBackpressure,
    TerminalOutput,
    FilesystemChanges,
    Commands,
    LanguageSync,
    LspRestart,
    LspDiagnostics,
    WorkspaceRefresh,
    PluginReload,
    Autosave,
    SessionSave,
    StartupWarmup,
    Scheduled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct RepaintFrameActivity {
    pub(crate) ui_events: usize,
    pub(crate) dropped_ui_events: usize,
    pub(crate) terminal_events: usize,
    pub(crate) filesystem_changes: usize,
    pub(crate) commands: usize,
    pub(crate) language_syncs: usize,
    pub(crate) lsp_restarts: usize,
    pub(crate) lsp_diagnostics: usize,
    pub(crate) workspace_refreshes: usize,
    pub(crate) plugin_reloads: usize,
    pub(crate) autosaves: usize,
    pub(crate) session_save_requested: bool,
    pub(crate) startup_warmup: bool,
}

impl RepaintFrameActivity {
    pub(crate) fn has_runtime_activity(self) -> bool {
        self.ui_events > 0
            || self.dropped_ui_events > 0
            || self.terminal_events > 0
            || self.filesystem_changes > 0
            || self.commands > 0
            || self.language_syncs > 0
            || self.lsp_restarts > 0
            || self.lsp_diagnostics > 0
            || self.workspace_refreshes > 0
            || self.plugin_reloads > 0
            || self.autosaves > 0
            || self.session_save_requested
    }

    pub(crate) fn keeps_frame_active(self) -> bool {
        self.startup_warmup || self.has_runtime_activity()
    }

    pub(crate) fn needs_immediate_repaint(self) -> bool {
        self.dropped_ui_events > 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RepaintDiagnosticSample {
    pub(crate) id: u64,
    pub(crate) cause: RepaintCause,
    pub(crate) frame_interval_ms: Option<f32>,
    pub(crate) update_ms: f32,
    pub(crate) repaint_after_ms: f32,
    pub(crate) activity: RepaintFrameActivity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RepaintDiagnosticStats {
    pub(crate) sample_count: usize,
    pub(crate) average_update_ms: f32,
    pub(crate) max_update_ms: f32,
    pub(crate) average_frame_interval_ms: Option<f32>,
    pub(crate) max_frame_interval_ms: Option<f32>,
    pub(crate) slow_frame_count: usize,
}

impl KuroyaApp {
    pub(crate) fn record_repaint_diagnostics(
        &mut self,
        activity: RepaintFrameActivity,
        update_duration: Duration,
        repaint_after: Duration,
    ) {
        let now = Instant::now();
        let frame_interval_ms = self
            .last_repaint_diagnostic_at
            .map(|last| duration_ms(now.saturating_duration_since(last)));
        self.last_repaint_diagnostic_at = Some(now);
        let id = next_devtools_trace_id(&mut self.next_repaint_diagnostic_id);
        record_repaint_diagnostic_sample(
            &mut self.repaint_diagnostics,
            RepaintDiagnosticSample {
                id,
                cause: repaint_cause(activity),
                frame_interval_ms,
                update_ms: duration_ms(update_duration),
                repaint_after_ms: duration_ms(repaint_after),
                activity,
            },
            MAX_REPAINT_DIAGNOSTIC_SAMPLES,
        );
    }
}

pub(crate) fn repaint_cause(activity: RepaintFrameActivity) -> RepaintCause {
    if activity.ui_events > 0 {
        RepaintCause::UiEvents
    } else if activity.dropped_ui_events > 0 {
        RepaintCause::UiEventBackpressure
    } else if activity.terminal_events > 0 {
        RepaintCause::TerminalOutput
    } else if activity.filesystem_changes > 0 {
        RepaintCause::FilesystemChanges
    } else if activity.commands > 0 {
        RepaintCause::Commands
    } else if activity.language_syncs > 0 {
        RepaintCause::LanguageSync
    } else if activity.lsp_restarts > 0 {
        RepaintCause::LspRestart
    } else if activity.lsp_diagnostics > 0 {
        RepaintCause::LspDiagnostics
    } else if activity.workspace_refreshes > 0 {
        RepaintCause::WorkspaceRefresh
    } else if activity.plugin_reloads > 0 {
        RepaintCause::PluginReload
    } else if activity.autosaves > 0 {
        RepaintCause::Autosave
    } else if activity.session_save_requested {
        RepaintCause::SessionSave
    } else if activity.startup_warmup {
        RepaintCause::StartupWarmup
    } else {
        RepaintCause::Scheduled
    }
}

pub(crate) fn record_repaint_diagnostic_sample(
    samples: &mut VecDeque<RepaintDiagnosticSample>,
    mut sample: RepaintDiagnosticSample,
    max_samples: usize,
) {
    if max_samples == 0 {
        samples.clear();
        return;
    }
    sample.frame_interval_ms = sample.frame_interval_ms.map(bounded_repaint_metric_ms);
    sample.update_ms = bounded_repaint_metric_ms(sample.update_ms);
    sample.repaint_after_ms = bounded_repaint_metric_ms(sample.repaint_after_ms);
    samples.push_back(sample);
    while samples.len() > max_samples {
        samples.pop_front();
    }
}

pub(crate) fn repaint_diagnostic_stats(
    samples: &VecDeque<RepaintDiagnosticSample>,
) -> Option<RepaintDiagnosticStats> {
    if samples.is_empty() {
        return None;
    }

    let mut update_sum = 0.0;
    let mut max_update_ms = 0.0_f32;
    let mut interval_sum = 0.0;
    let mut interval_count = 0usize;
    let mut max_frame_interval_ms = 0.0_f32;
    let mut slow_frame_count = 0usize;

    for sample in samples {
        let update_ms = bounded_repaint_metric_ms(sample.update_ms);
        update_sum += update_ms;
        max_update_ms = max_update_ms.max(update_ms);
        if let Some(interval_ms) = sample.frame_interval_ms.map(bounded_repaint_metric_ms) {
            interval_sum += interval_ms;
            interval_count += 1;
            max_frame_interval_ms = max_frame_interval_ms.max(interval_ms);
            if interval_ms > FRAME_BUDGET_MS {
                slow_frame_count += 1;
            }
        }
    }

    Some(RepaintDiagnosticStats {
        sample_count: samples.len(),
        average_update_ms: update_sum / samples.len() as f32,
        max_update_ms,
        average_frame_interval_ms: (interval_count > 0)
            .then_some(interval_sum / interval_count as f32),
        max_frame_interval_ms: (interval_count > 0).then_some(max_frame_interval_ms),
        slow_frame_count,
    })
}

pub(crate) fn render_repaint_diagnostics_panel(
    ui: &mut egui::Ui,
    samples: &VecDeque<RepaintDiagnosticSample>,
) {
    ui.label(RichText::new("Repaint Diagnostics").strong());
    let Some(latest) = samples.back() else {
        ui.label(RichText::new("No repaint samples yet").small());
        return;
    };

    ui.horizontal(|ui| {
        ui.label(format!("Cause {}", latest.cause.label()));
        ui.label(format!(
            "Update {:.1} ms",
            bounded_repaint_metric_ms(latest.update_ms)
        ));
        ui.label(format!(
            "Next {:.0} ms",
            bounded_repaint_metric_ms(latest.repaint_after_ms)
        ));
    });
    if let Some(interval) = latest.frame_interval_ms.map(bounded_repaint_metric_ms) {
        ui.label(format!("Frame interval {:.1} ms", interval));
    }
    if let Some(stats) = repaint_diagnostic_stats(samples) {
        ui.horizontal(|ui| {
            ui.label(format!("Avg update {:.1} ms", stats.average_update_ms));
            ui.label(format!("Max update {:.1} ms", stats.max_update_ms));
            if let Some(interval) = stats.average_frame_interval_ms {
                ui.label(format!("Avg frame {:.1} ms", interval));
            }
            ui.label(format!("Slow frames {}", stats.slow_frame_count));
        });
    }

    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
    egui::ScrollArea::vertical()
        .max_height(160.0)
        .auto_shrink([false, false])
        .show_rows(
            ui,
            row_height,
            samples.len().min(REPAINT_DIAGNOSTIC_VISIBLE_ROWS),
            |ui, row_range| {
                for row_index in row_range {
                    let Some(sample) = repaint_diagnostic_visible_sample(samples, row_index) else {
                        continue;
                    };
                    let mut row = String::with_capacity(REPAINT_DIAGNOSTIC_ROW_CAPACITY);
                    append_repaint_diagnostic_row(&mut row, sample);
                    ui.monospace(row);
                }
            },
        );
}

#[cfg(test)]
fn activity_label(activity: RepaintFrameActivity) -> String {
    let mut label = String::new();
    append_activity_label(&mut label, activity);
    label
}

fn repaint_diagnostic_visible_sample(
    samples: &VecDeque<RepaintDiagnosticSample>,
    row_index: usize,
) -> Option<&RepaintDiagnosticSample> {
    if row_index >= REPAINT_DIAGNOSTIC_VISIBLE_ROWS {
        return None;
    }
    let sample_index = samples.len().checked_sub(row_index + 1)?;
    samples.get(sample_index)
}

#[cfg(test)]
fn repaint_diagnostic_row(sample: &RepaintDiagnosticSample) -> String {
    let mut row = String::with_capacity(REPAINT_DIAGNOSTIC_ROW_CAPACITY);
    append_repaint_diagnostic_row(&mut row, sample);
    row
}

fn append_repaint_diagnostic_row(output: &mut String, sample: &RepaintDiagnosticSample) {
    let _ = write!(
        output,
        "#{:04} {:<10} update {:.1} ms interval ",
        sample.id,
        sample.cause.label(),
        bounded_repaint_metric_ms(sample.update_ms)
    );
    if let Some(interval) = sample.frame_interval_ms.map(bounded_repaint_metric_ms) {
        let _ = write!(output, "{interval:.1} ms");
    } else {
        output.push_str("first");
    }
    output.push(' ');
    append_activity_label(output, sample.activity);
}

fn append_activity_label(output: &mut String, activity: RepaintFrameActivity) {
    let mut has_part = false;
    push_count(output, &mut has_part, activity.ui_events, "event");
    push_count(output, &mut has_part, activity.dropped_ui_events, "dropped");
    push_count(output, &mut has_part, activity.terminal_events, "terminal");
    push_count(output, &mut has_part, activity.filesystem_changes, "fs");
    push_count(output, &mut has_part, activity.commands, "cmd");
    push_count(
        output,
        &mut has_part,
        activity.language_syncs,
        REPAINT_LABEL_LANGUAGE_SYNC,
    );
    push_count(
        output,
        &mut has_part,
        activity.lsp_restarts,
        REPAINT_LABEL_LSP_RESTART,
    );
    push_count(
        output,
        &mut has_part,
        activity.lsp_diagnostics,
        REPAINT_LABEL_LSP_DIAGNOSTICS,
    );
    push_count(
        output,
        &mut has_part,
        activity.workspace_refreshes,
        REPAINT_LABEL_WORKSPACE_REFRESH,
    );
    push_count(
        output,
        &mut has_part,
        activity.plugin_reloads,
        REPAINT_LABEL_PLUGIN_RELOAD,
    );
    push_count(output, &mut has_part, activity.autosaves, "save");
    if activity.session_save_requested {
        push_part(output, &mut has_part, REPAINT_LABEL_SESSION_SAVE);
    }
    if activity.startup_warmup {
        push_part(output, &mut has_part, REPAINT_LABEL_STARTUP_WARMUP);
    }
    if !has_part {
        output.push_str("idle");
    }
}

fn push_part(output: &mut String, has_part: &mut bool, part: &str) {
    if *has_part {
        output.push_str(", ");
    }
    output.push_str(part);
    *has_part = true;
}

fn push_count(output: &mut String, has_part: &mut bool, count: usize, label: &str) {
    if count > 0 {
        if *has_part {
            output.push_str(", ");
        }
        let _ = write!(output, "{count} {label}");
        *has_part = true;
    }
}

fn duration_ms(duration: Duration) -> f32 {
    bounded_repaint_metric_ms(duration.as_secs_f32() * 1000.0)
}

fn bounded_repaint_metric_ms(metric_ms: f32) -> f32 {
    if metric_ms.is_nan() {
        0.0
    } else {
        metric_ms.clamp(0.0, MAX_REPAINT_DIAGNOSTIC_MS)
    }
}

impl RepaintCause {
    fn label(self) -> &'static str {
        match self {
            Self::UiEvents => REPAINT_LABEL_UI_EVENTS,
            Self::UiEventBackpressure => REPAINT_LABEL_UI_EVENT_BACKPRESSURE,
            Self::TerminalOutput => REPAINT_LABEL_TERMINAL_OUTPUT,
            Self::FilesystemChanges => REPAINT_LABEL_FILESYSTEM_CHANGES,
            Self::Commands => REPAINT_LABEL_COMMANDS,
            Self::LanguageSync => REPAINT_LABEL_LANGUAGE_SYNC,
            Self::LspRestart => REPAINT_LABEL_LSP_RESTART,
            Self::LspDiagnostics => REPAINT_LABEL_LSP_DIAGNOSTICS,
            Self::WorkspaceRefresh => REPAINT_LABEL_WORKSPACE_REFRESH,
            Self::PluginReload => REPAINT_LABEL_PLUGIN_RELOAD,
            Self::Autosave => REPAINT_LABEL_AUTOSAVE,
            Self::SessionSave => REPAINT_LABEL_SESSION_SAVE,
            Self::StartupWarmup => REPAINT_LABEL_STARTUP_WARMUP,
            Self::Scheduled => REPAINT_LABEL_SCHEDULED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_REPAINT_DIAGNOSTIC_MS, RepaintCause, RepaintDiagnosticSample, RepaintFrameActivity,
        activity_label, record_repaint_diagnostic_sample, repaint_cause, repaint_diagnostic_row,
        repaint_diagnostic_stats, repaint_diagnostic_visible_sample,
    };
    use std::collections::VecDeque;

    #[test]
    fn repaint_cause_prioritizes_visible_activity() {
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                ui_events: 1,
                dropped_ui_events: 1,
                terminal_events: 1,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::UiEvents
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                dropped_ui_events: 1,
                terminal_events: 1,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::UiEventBackpressure
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                terminal_events: 1,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::TerminalOutput
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                lsp_diagnostics: 2,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::LspDiagnostics
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                lsp_restarts: 1,
                lsp_diagnostics: 2,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::LspRestart
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                plugin_reloads: 1,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::PluginReload
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                workspace_refreshes: 1,
                plugin_reloads: 1,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::WorkspaceRefresh
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                session_save_requested: true,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::SessionSave
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity {
                startup_warmup: true,
                ..RepaintFrameActivity::default()
            }),
            RepaintCause::StartupWarmup
        );
        assert_eq!(
            repaint_cause(RepaintFrameActivity::default()),
            RepaintCause::Scheduled
        );
    }

    #[test]
    fn repaint_diagnostic_samples_are_bounded() {
        let mut samples = VecDeque::new();
        for id in 1..=3 {
            record_repaint_diagnostic_sample(
                &mut samples,
                RepaintDiagnosticSample {
                    id,
                    cause: RepaintCause::Scheduled,
                    frame_interval_ms: None,
                    update_ms: id as f32,
                    repaint_after_ms: 80.0,
                    activity: RepaintFrameActivity::default(),
                },
                2,
            );
        }

        assert_eq!(
            samples
                .iter()
                .map(|sample| (sample.id, sample.update_ms))
                .collect::<Vec<_>>(),
            vec![(2, 2.0), (3, 3.0)]
        );
    }

    #[test]
    fn repaint_diagnostic_samples_normalize_metric_bounds() {
        let mut samples = VecDeque::new();

        record_repaint_diagnostic_sample(
            &mut samples,
            RepaintDiagnosticSample {
                id: 1,
                cause: RepaintCause::Scheduled,
                frame_interval_ms: Some(-12.0),
                update_ms: f32::NAN,
                repaint_after_ms: f32::INFINITY,
                activity: RepaintFrameActivity::default(),
            },
            8,
        );

        let sample = samples.front().expect("sample should be recorded");
        assert_eq!(sample.frame_interval_ms, Some(0.0));
        assert_eq!(sample.update_ms, 0.0);
        assert_eq!(sample.repaint_after_ms, MAX_REPAINT_DIAGNOSTIC_MS);
    }

    #[test]
    fn repaint_diagnostic_stats_summarize_recent_frame_health() {
        let samples = VecDeque::from([
            RepaintDiagnosticSample {
                id: 1,
                cause: RepaintCause::Scheduled,
                frame_interval_ms: None,
                update_ms: 3.0,
                repaint_after_ms: 80.0,
                activity: RepaintFrameActivity::default(),
            },
            RepaintDiagnosticSample {
                id: 2,
                cause: RepaintCause::Commands,
                frame_interval_ms: Some(12.0),
                update_ms: 6.0,
                repaint_after_ms: 16.0,
                activity: RepaintFrameActivity {
                    commands: 1,
                    ..RepaintFrameActivity::default()
                },
            },
            RepaintDiagnosticSample {
                id: 3,
                cause: RepaintCause::TerminalOutput,
                frame_interval_ms: Some(33.4),
                update_ms: 12.0,
                repaint_after_ms: 0.0,
                activity: RepaintFrameActivity {
                    terminal_events: 4,
                    ..RepaintFrameActivity::default()
                },
            },
        ]);

        let stats = repaint_diagnostic_stats(&samples).expect("samples should produce stats");

        assert_eq!(stats.sample_count, 3);
        assert_close(stats.average_update_ms, 7.0);
        assert_close(stats.max_update_ms, 12.0);
        assert_close(stats.average_frame_interval_ms.unwrap(), 22.7);
        assert_close(stats.max_frame_interval_ms.unwrap(), 33.4);
        assert_eq!(stats.slow_frame_count, 1);
    }

    #[test]
    fn repaint_diagnostic_stats_bound_legacy_metric_values() {
        let samples = VecDeque::from([
            RepaintDiagnosticSample {
                id: 1,
                cause: RepaintCause::Scheduled,
                frame_interval_ms: Some(f32::NAN),
                update_ms: f32::INFINITY,
                repaint_after_ms: 80.0,
                activity: RepaintFrameActivity::default(),
            },
            RepaintDiagnosticSample {
                id: 2,
                cause: RepaintCause::Commands,
                frame_interval_ms: Some(MAX_REPAINT_DIAGNOSTIC_MS + 1_000.0),
                update_ms: -40.0,
                repaint_after_ms: 16.0,
                activity: RepaintFrameActivity {
                    commands: 1,
                    ..RepaintFrameActivity::default()
                },
            },
        ]);

        let stats = repaint_diagnostic_stats(&samples).expect("samples should produce stats");

        assert_eq!(stats.sample_count, 2);
        assert_close(stats.average_update_ms, MAX_REPAINT_DIAGNOSTIC_MS / 2.0);
        assert_close(stats.max_update_ms, MAX_REPAINT_DIAGNOSTIC_MS);
        assert_close(
            stats.average_frame_interval_ms.unwrap(),
            MAX_REPAINT_DIAGNOSTIC_MS / 2.0,
        );
        assert_close(
            stats.max_frame_interval_ms.unwrap(),
            MAX_REPAINT_DIAGNOSTIC_MS,
        );
        assert_eq!(stats.slow_frame_count, 1);
    }

    #[test]
    fn repaint_diagnostic_stats_are_empty_without_samples() {
        assert_eq!(repaint_diagnostic_stats(&VecDeque::new()), None);
    }

    #[test]
    fn repaint_diagnostic_visible_sample_maps_newest_rows_first() {
        let samples = VecDeque::from([
            RepaintDiagnosticSample {
                id: 1,
                cause: RepaintCause::Scheduled,
                frame_interval_ms: None,
                update_ms: 1.0,
                repaint_after_ms: 80.0,
                activity: RepaintFrameActivity::default(),
            },
            RepaintDiagnosticSample {
                id: 2,
                cause: RepaintCause::Commands,
                frame_interval_ms: Some(16.0),
                update_ms: 2.0,
                repaint_after_ms: 16.0,
                activity: RepaintFrameActivity {
                    commands: 1,
                    ..RepaintFrameActivity::default()
                },
            },
            RepaintDiagnosticSample {
                id: 3,
                cause: RepaintCause::TerminalOutput,
                frame_interval_ms: Some(32.0),
                update_ms: 3.0,
                repaint_after_ms: 0.0,
                activity: RepaintFrameActivity {
                    terminal_events: 1,
                    ..RepaintFrameActivity::default()
                },
            },
        ]);

        assert_eq!(
            repaint_diagnostic_visible_sample(&samples, 0).map(|sample| sample.id),
            Some(3)
        );
        assert_eq!(
            repaint_diagnostic_visible_sample(&samples, 1).map(|sample| sample.id),
            Some(2)
        );
        assert_eq!(
            repaint_diagnostic_visible_sample(&samples, 2).map(|sample| sample.id),
            Some(1)
        );
        assert_eq!(repaint_diagnostic_visible_sample(&samples, 3), None);
    }

    #[test]
    fn repaint_diagnostic_visible_sample_preserves_display_cap() {
        let samples = (1..=45)
            .map(|id| RepaintDiagnosticSample {
                id,
                cause: RepaintCause::Scheduled,
                frame_interval_ms: Some(id as f32),
                update_ms: id as f32,
                repaint_after_ms: 16.0,
                activity: RepaintFrameActivity::default(),
            })
            .collect::<VecDeque<_>>();

        assert_eq!(
            repaint_diagnostic_visible_sample(&samples, 39).map(|sample| sample.id),
            Some(6)
        );
        assert_eq!(repaint_diagnostic_visible_sample(&samples, 40), None);
    }

    #[test]
    fn repaint_diagnostic_row_bounds_legacy_metrics() {
        let row = repaint_diagnostic_row(&RepaintDiagnosticSample {
            id: 7,
            cause: RepaintCause::TerminalOutput,
            frame_interval_ms: Some(f32::NAN),
            update_ms: f32::INFINITY,
            repaint_after_ms: 0.0,
            activity: RepaintFrameActivity {
                dropped_ui_events: 1,
                terminal_events: 2,
                ..RepaintFrameActivity::default()
            },
        });

        assert_eq!(
            row,
            "#0007 terminal   update 3600000.0 ms interval 0.0 ms 1 dropped, 2 terminal"
        );
    }

    #[test]
    fn repaint_diagnostic_row_reports_first_idle_sample() {
        let row = repaint_diagnostic_row(&RepaintDiagnosticSample {
            id: 42,
            cause: RepaintCause::Scheduled,
            frame_interval_ms: None,
            update_ms: -1.0,
            repaint_after_ms: 0.0,
            activity: RepaintFrameActivity::default(),
        });

        assert_eq!(row, "#0042 scheduled  update 0.0 ms interval first idle");
    }

    #[test]
    fn repaint_activity_label_reports_dropped_ui_events() {
        assert_eq!(
            activity_label(RepaintFrameActivity {
                dropped_ui_events: 3,
                ..RepaintFrameActivity::default()
            }),
            "3 dropped"
        );
    }

    #[test]
    fn repaint_activity_label_reports_startup_warmup() {
        assert_eq!(
            activity_label(RepaintFrameActivity {
                startup_warmup: true,
                ..RepaintFrameActivity::default()
            }),
            "startup"
        );
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {actual} to be close to {expected}"
        );
    }
}
