use crate::{KuroyaApp, session_state::EditorPane};
use kuroya_core::BufferId;
use std::{collections::HashMap, path::PathBuf};

impl KuroyaApp {
    pub(super) fn restore_session_panes(
        &mut self,
        pane_paths: Vec<Option<PathBuf>>,
        pane_weights: &[f32],
        active_pane_index: Option<usize>,
        restored_by_path: &HashMap<PathBuf, BufferId>,
    ) -> Vec<Option<crate::workspace_state::PaneId>> {
        let pane_paths = restorable_pane_paths(pane_paths);
        self.clear_pane_restore_runtime_state();
        self.panes.clear();
        self.focused_pane = None;
        self.last_autosave_focused_pane = None;
        let mut pane_ids_by_index = Vec::with_capacity(pane_paths.len());
        for (pane_index, pane_path) in pane_paths.into_iter().enumerate() {
            let id = self.next_pane_id;
            self.next_pane_id += 1;
            pane_ids_by_index.push(Some(id));
            let active = pane_path
                .as_ref()
                .and_then(|path| restored_by_path.get(path).copied());
            if active.is_none()
                && let Some(path) = pane_path
            {
                self.pending_pane_paths.insert(id, path);
            }
            let weight = pane_weights.get(pane_index).copied().unwrap_or(1.0);
            self.panes.push(EditorPane { id, active, weight });
        }

        if self.panes.is_empty() {
            self.panes.push(EditorPane {
                id: 1,
                active: None,
                weight: 1.0,
            });
            self.active_pane = 1;
            self.next_pane_id = self.next_pane_id.max(2);
        } else {
            self.active_pane = active_pane_index
                .and_then(|index| self.panes.get(index))
                .map(|pane| pane.id)
                .unwrap_or(self.panes[0].id);
        }
        self.normalize_pane_weights();
        pane_ids_by_index
    }

    fn clear_pane_restore_runtime_state(&mut self) {
        self.pending_pane_paths.clear();
        self.pending_pane_view_states.clear();
        self.pending_pane_scroll_lines.clear();
        self.pending_pane_horizontal_scroll_offsets.clear();
        self.editor_scroll_offsets.clear();
        self.editor_horizontal_scroll_offsets.clear();
        self.editor_scroll_targets.clear();
        self.editor_inertial_scrolls.clear();
        self.editor_middle_click_scroll = None;
    }
}

fn restorable_pane_paths(pane_paths: Vec<Option<PathBuf>>) -> Vec<Option<PathBuf>> {
    if pane_paths.iter().all(Option::is_none) {
        Vec::new()
    } else {
        pane_paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_startup_context::AppStartupContext, persistence::PaneBufferViewState,
        terminal::TerminalPane, transient_state::EditorInertialScroll,
        transient_state::EditorMiddleClickScroll,
    };
    use kuroya_core::{EditorSettings, Workspace};
    use std::{path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn restore_session_panes_clears_stale_pane_runtime_state() {
        let root = PathBuf::from("workspace");
        let old_path = root.join("old.rs");
        let restored_path = root.join("src/main.rs");
        let mut app = app_for_test(root.clone());
        let stale_pane = app.active_pane;
        let stale_buffer = 7;

        app.pending_pane_paths.insert(stale_pane, old_path.clone());
        app.pending_pane_view_states.insert(
            stale_pane,
            PaneBufferViewState {
                pane_index: 0,
                path: old_path,
                scroll_line: 12,
                horizontal_scroll_offset: 32.0,
            },
        );
        app.pending_pane_scroll_lines
            .insert((stale_pane, stale_buffer), 12);
        app.pending_pane_horizontal_scroll_offsets
            .insert((stale_pane, stale_buffer), 32.0);
        app.editor_scroll_offsets
            .insert((stale_pane, stale_buffer), 120.0);
        app.editor_horizontal_scroll_offsets
            .insert((stale_pane, stale_buffer), 48.0);
        app.editor_scroll_targets
            .insert((stale_pane, stale_buffer), 180.0);
        app.editor_inertial_scrolls.insert(
            (stale_pane, stale_buffer),
            EditorInertialScroll {
                velocity_x: 4.0,
                velocity_y: 8.0,
            },
        );
        app.editor_middle_click_scroll = Some(EditorMiddleClickScroll {
            pane_id: stale_pane,
            buffer_id: stale_buffer,
            anchor_y: 80.0,
        });

        let pane_ids = app.restore_session_panes(
            vec![Some(restored_path.clone())],
            &[0.5],
            Some(0),
            &HashMap::new(),
        );
        let restored_pane = app.panes[0].id;

        assert_eq!(pane_ids, vec![Some(restored_pane)]);
        assert_eq!(app.panes.len(), 1);
        assert_eq!(app.active_pane, restored_pane);
        assert_eq!(
            app.pending_pane_paths.get(&restored_pane),
            Some(&restored_path)
        );
        assert!(app.pending_pane_view_states.is_empty());
        assert!(app.pending_pane_scroll_lines.is_empty());
        assert!(app.pending_pane_horizontal_scroll_offsets.is_empty());
        assert!(app.editor_scroll_offsets.is_empty());
        assert!(app.editor_horizontal_scroll_offsets.is_empty());
        assert!(app.editor_scroll_targets.is_empty());
        assert!(app.editor_inertial_scrolls.is_empty());
        assert!(app.editor_middle_click_scroll.is_none());
    }

    #[test]
    fn restore_session_panes_collapses_all_empty_persisted_panes() {
        let root = PathBuf::from("workspace");
        let mut app = app_for_test(root);

        let pane_ids = app.restore_session_panes(
            vec![None, None, None],
            &[f32::NAN],
            Some(2),
            &HashMap::new(),
        );

        assert!(pane_ids.is_empty());
        assert_eq!(app.panes.len(), 1);
        assert_eq!(app.panes[0].id, 1);
        assert_eq!(app.panes[0].active, None);
        assert_eq!(app.panes[0].weight, 1.0);
        assert_eq!(app.active_pane, 1);
        assert_eq!(app.next_pane_id, 2);
        assert!(app.pending_pane_paths.is_empty());
    }

    fn app_for_test(root: PathBuf) -> KuroyaApp {
        let (tx, rx) = crate::ui_event_channel::ui_event_channel();
        let settings = EditorSettings::default();
        KuroyaApp::from_startup_context(AppStartupContext {
            runtime: Runtime::new().expect("test runtime"),
            tx,
            rx,
            workspace: Workspace::new(root.clone()),
            settings: settings.clone(),
            settings_panel_draft: settings,
            settings_editor_font_path: String::new(),
            settings_ui_font_path: String::new(),
            theme_picker_selected: 0,
            saved_session: None,
            terminal: TerminalPane::new(root.clone(), 100, 12.0, 1.2),
            watcher: None,
            recent_projects: Vec::new(),
            trusted_workspaces: vec![root],
            now: Instant::now(),
            startup_timings: Vec::new(),
        })
    }
}
