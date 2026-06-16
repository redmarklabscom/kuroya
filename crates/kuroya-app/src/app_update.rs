mod panels;

use crate::{
    KuroyaApp, devtools_repaint_diagnostics::RepaintFrameActivity, fonts, persistence, theme,
    ui_event_channel,
};
use eframe::egui::Context;
use kuroya_core::window_zoom_factor;
use std::time::Instant;

impl KuroyaApp {
    pub(crate) fn drain_terminal_output_for_frame(&mut self) -> (usize, bool) {
        let terminal_events = self.terminal.drain_output();
        if !self.running_workspace_tasks.is_empty() {
            self.prune_finished_workspace_tasks();
        }
        let terminal_output_pending = self.terminal.has_pending_output();
        (terminal_events, terminal_output_pending)
    }
}

impl eframe::App for KuroyaApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        let profiling = self.profiling_enabled();
        let mut profile_mark = frame_start;
        let zoom_factor = window_zoom_factor(self.settings.window_zoom_level);
        if (ctx.zoom_factor() - zoom_factor).abs() > f32::EPSILON {
            ctx.set_zoom_factor(zoom_factor);
        }
        self.handle_close_request(ctx);
        if self.fonts_dirty {
            fonts::install_fonts(ctx, &self.workspace.root, &self.settings);
            fonts::apply_typography(ctx, &self.settings);
            self.fonts_dirty = false;
        }
        if self.theme_dirty {
            theme::apply_theme(ctx, &self.settings.theme);
            self.theme_dirty = false;
        }
        self.terminal.set_repaint_context(ctx.clone());
        self.record_profile_mark(profiling, &mut profile_mark, "frame", "setup");
        let ui_events = self.handle_events_with_context(ctx);
        if self.exit_confirmed {
            self.exit_process();
        }
        self.dispatch_shortcuts(ctx);
        self.handle_editor_file_drops(ctx);
        self.record_profile_mark(profiling, &mut profile_mark, "frame", "events");
        let (terminal_events, terminal_output_pending) = self.drain_terminal_output_for_frame();
        let filesystem_changes = self.drain_file_watcher();
        let workspace_refreshes = self.flush_pending_workspace_refresh();
        let plugin_reloads = self.flush_pending_workspace_plugin_reload();
        let window_focused = ctx.input(|input| input.focused);
        let autosaves = self.autosave_if_needed(window_focused);
        let session_save_requested = self.persist_session_if_needed();
        let language_syncs = self.flush_pending_language_sync();
        let completion_requests = self.flush_pending_completion_requests();
        let signature_help_requests = self.flush_pending_signature_help_requests();
        let format_on_type_requests = self.flush_pending_format_on_type_requests();
        let format_on_save_timeouts = self.flush_timed_out_format_on_save_requests();
        let lsp_restarts = self.flush_pending_lsp_restarts();
        let lsp_diagnostics = self.flush_pending_lsp_diagnostics();
        let lsp_symbol_refreshes = self.flush_pending_lsp_symbol_refreshes();
        self.record_profile_mark(profiling, &mut profile_mark, "frame", "runtime");
        let commands = self.drain_commands(ctx);
        self.record_profile_mark(profiling, &mut profile_mark, "frame", "commands");
        let dropped_ui_events = ui_event_channel::take_dropped_ui_event_count();

        self.render_main_panels(ctx);
        self.render_active_overlays(ctx);
        self.record_profile_mark(profiling, &mut profile_mark, "frame", "render");
        let update_duration = frame_start.elapsed();
        if profiling {
            self.record_profile_sample("frame", "total", update_duration);
        }
        let startup_warmup = self.startup_repaint_warmup_active();
        self.record_frame_timing(update_duration);
        self.maybe_show_gpu_acceleration_prompt();
        let activity = RepaintFrameActivity {
            ui_events,
            dropped_ui_events,
            terminal_events,
            filesystem_changes,
            commands,
            language_syncs: language_syncs
                .saturating_add(completion_requests)
                .saturating_add(signature_help_requests)
                .saturating_add(format_on_type_requests)
                .saturating_add(format_on_save_timeouts)
                .saturating_add(lsp_symbol_refreshes),
            lsp_restarts,
            lsp_diagnostics,
            workspace_refreshes,
            plugin_reloads,
            autosaves,
            session_save_requested,
            startup_warmup,
        };
        let repaint_after = self.next_frame_repaint_after(
            Instant::now(),
            activity,
            terminal_output_pending,
            profiling,
        );
        self.record_repaint_diagnostics(activity, update_duration, repaint_after);
        if repaint_after.is_zero() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(repaint_after);
        }
    }

    fn on_exit(&mut self) {
        self.prepare_shutdown();
    }
}

impl KuroyaApp {
    fn exit_process(&mut self) -> ! {
        self.prepare_shutdown();
        std::process::exit(0);
    }

    fn prepare_shutdown(&mut self) {
        if self.shutdown_prepared {
            return;
        }
        self.shutdown_prepared = true;
        self.notify_lsp_close_all();
        for client in self.lsp_clients.values() {
            client.shutdown();
        }
        let _ = self.save_app_state();
        let _ = self.terminal.drain_output_for_shutdown();
        if !self.workspace_placeholder {
            let _ = persistence::save_session(&self.workspace.root, &self.build_session());
        }
        self.terminal.close_all_sessions_for_shutdown();
    }
}
