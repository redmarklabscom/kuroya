use crate::{
    KuroyaApp,
    path_display::{
        DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow,
        sanitized_display_label_cow,
    },
};
use kuroya_core::{BufferId, TextBuffer};
use std::borrow::Cow;

pub(crate) fn read_only_toggle_block_reason(
    lossy_preview: bool,
    binary_preview: bool,
    image_preview: bool,
    virtual_preview: bool,
) -> Option<&'static str> {
    if image_preview {
        Some("image previews must stay read-only")
    } else if binary_preview {
        Some("binary previews must stay read-only")
    } else if lossy_preview {
        Some("UTF-8 replacement previews must stay read-only")
    } else if virtual_preview {
        Some("generated preview buffers must stay read-only")
    } else {
        None
    }
}

pub(crate) fn read_only_status(buffer: &TextBuffer, read_only: bool) -> String {
    let name = buffer
        .path()
        .map(|path| display_path_label_cow(path.as_path()))
        .unwrap_or(Cow::Borrowed("Untitled"));
    if read_only {
        format!("{} is now read-only", name.as_ref())
    } else {
        format!("{} is now editable", name.as_ref())
    }
}

pub(crate) fn configured_read_only_reason(message: &str) -> String {
    configured_read_only_reason_cow(message).into_owned()
}

fn configured_read_only_reason_cow(message: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(
        message,
        DISPLAY_ERROR_LABEL_MAX_CHARS,
        "buffer is read-only",
    )
}

fn read_only_buffer_label(label: &str) -> String {
    read_only_buffer_label_cow(label).into_owned()
}

fn read_only_buffer_label_cow(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(label, DISPLAY_PATH_LABEL_MAX_CHARS, "Untitled")
}

impl KuroyaApp {
    fn read_only_toggle_blocked_by_global_setting(&self) -> bool {
        self.settings.read_only
    }

    pub(crate) fn sync_global_read_only_buffers(&mut self) {
        let global_read_only = self.settings.read_only;
        let protected_ids = self
            .lossy_decoded_buffers
            .iter()
            .copied()
            .chain(self.binary_preview_buffers.iter().copied())
            .chain(self.virtual_buffer_labels.keys().copied())
            .collect::<std::collections::HashSet<_>>();

        for buffer in &mut self.buffers {
            if !protected_ids.contains(&buffer.id()) {
                let read_only =
                    global_read_only || self.manual_read_only_buffers.contains(&buffer.id());
                buffer.set_read_only(read_only);
            }
        }
    }

    pub(crate) fn toggle_active_read_only(&mut self) {
        let Some(id) = self.active else {
            self.status = "No active buffer".to_owned();
            return;
        };
        self.toggle_buffer_read_only(id);
    }

    pub(crate) fn toggle_buffer_read_only(&mut self, id: BufferId) {
        if self.read_only_toggle_blocked_by_global_setting() {
            self.status =
                "Cannot change read-only mode while editor read-only is enabled".to_owned();
            return;
        }

        if let Some(reason) = read_only_toggle_block_reason(
            self.lossy_decoded_buffers.contains(&id),
            self.binary_preview_buffers.contains(&id),
            self.image_preview_buffers.contains_key(&id),
            self.virtual_buffer_labels.contains_key(&id),
        ) {
            let label = read_only_buffer_label(&self.buffer_label(id));
            self.status = format!("Cannot change read-only mode for {label}; {reason}");
            return;
        }

        let Some(status) = self.buffer_mut(id).map(|buffer| {
            let read_only = !buffer.is_read_only();
            buffer.set_read_only(read_only);
            read_only_status(buffer, read_only)
        }) else {
            return;
        };
        if self.buffer(id).is_some_and(TextBuffer::is_read_only) {
            self.manual_read_only_buffers.insert(id);
        } else {
            self.manual_read_only_buffers.remove(&id);
        }
        self.status = status;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        configured_read_only_reason, configured_read_only_reason_cow, read_only_buffer_label,
        read_only_buffer_label_cow, read_only_status, read_only_toggle_block_reason,
    };
    use crate::{
        KuroyaApp,
        app_startup_context::AppStartupContext,
        path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS},
        terminal::TerminalPane,
    };
    use kuroya_core::{EditorSettings, TextBuffer, Workspace};
    use std::{borrow::Cow, path::PathBuf, time::Instant};
    use tokio::runtime::Runtime;

    #[test]
    fn read_only_toggle_blocks_protected_previews() {
        assert_eq!(
            read_only_toggle_block_reason(false, true, false, false),
            Some("binary previews must stay read-only")
        );
        assert_eq!(
            read_only_toggle_block_reason(false, true, true, false),
            Some("image previews must stay read-only")
        );
        assert_eq!(
            read_only_toggle_block_reason(true, false, false, false),
            Some("UTF-8 replacement previews must stay read-only")
        );
        assert_eq!(
            read_only_toggle_block_reason(false, false, false, true),
            Some("generated preview buffers must stay read-only")
        );
        assert_eq!(
            read_only_toggle_block_reason(false, false, false, false),
            None
        );
    }

    #[test]
    fn read_only_status_names_file_or_untitled_buffer() {
        let named = TextBuffer::from_text(
            1,
            Some(PathBuf::from("workspace/src/main.rs")),
            "fn main() {}".to_owned(),
        );
        let untitled = TextBuffer::from_text(2, None, String::new());

        assert_eq!(read_only_status(&named, true), "main.rs is now read-only");
        assert_eq!(read_only_status(&named, false), "main.rs is now editable");
        assert_eq!(
            read_only_status(&untitled, true),
            "Untitled is now read-only"
        );
    }

    #[test]
    fn read_only_status_sanitizes_and_bounds_file_names() {
        let buffer = TextBuffer::from_text(
            1,
            Some(
                PathBuf::from("workspace/src")
                    .join(format!("bad\n{}\u{202e}tail.rs", "very-long-".repeat(24))),
            ),
            "fn main() {}".to_owned(),
        );

        let status = read_only_status(&buffer, true);

        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(
            status.trim_end_matches(" is now read-only").chars().count()
                <= DISPLAY_PATH_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn configured_read_only_reason_uses_message_when_present() {
        assert_eq!(
            configured_read_only_reason(" Generated file "),
            "Generated file"
        );
        assert_eq!(configured_read_only_reason("  "), "buffer is read-only");
    }

    #[test]
    fn configured_read_only_reason_cow_borrows_clean_ascii_and_unicode_reasons() {
        assert!(matches!(
            configured_read_only_reason_cow("Generated file"),
            Cow::Borrowed("Generated file")
        ));

        let unicode = "Generated \u{03bb} file";
        match configured_read_only_reason_cow(unicode) {
            Cow::Borrowed(reason) => assert_eq!(reason, unicode),
            Cow::Owned(reason) => panic!("expected borrowed reason, got {reason:?}"),
        }
    }

    #[test]
    fn configured_read_only_reason_cow_owns_dirty_truncated_and_fallback_reasons() {
        let long = format!(
            "Generated {} file",
            "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        );
        let reasons = [
            " Generated file ",
            "bad\nreason\u{202e}",
            long.as_str(),
            "\n\u{202e}",
        ];

        for reason in reasons {
            let label = configured_read_only_reason_cow(reason);

            assert_eq!(label.as_ref(), configured_read_only_reason(reason));
            assert!(
                matches!(&label, Cow::Owned(_)),
                "expected owned reason for {reason:?}"
            );
        }
    }

    #[test]
    fn configured_read_only_reason_sanitizes_and_bounds_message() {
        let reason = configured_read_only_reason(&format!(
            "first line\nsecond line \u{202e}{}",
            "x".repeat(DISPLAY_ERROR_LABEL_MAX_CHARS * 2)
        ));

        assert!(!reason.contains('\n'));
        assert!(!reason.contains('\u{202e}'));
        assert!(reason.contains("..."));
        assert!(reason.chars().count() <= DISPLAY_ERROR_LABEL_MAX_CHARS);
    }

    #[test]
    fn read_only_buffer_label_cow_borrows_clean_ascii_and_unicode_labels() {
        assert!(matches!(
            read_only_buffer_label_cow("main.rs"),
            Cow::Borrowed("main.rs")
        ));

        let unicode = "main-\u{03bb}.rs";
        match read_only_buffer_label_cow(unicode) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn read_only_buffer_label_cow_owns_dirty_truncated_and_fallback_labels() {
        let long = format!("main-{}.rs", "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2));
        let labels = [
            " main.rs ",
            "bad\nname\u{202e}",
            long.as_str(),
            "\n\u{202e}",
        ];

        for label in labels {
            let display_label = read_only_buffer_label_cow(label);

            assert_eq!(display_label.as_ref(), read_only_buffer_label(label));
            assert!(
                matches!(&display_label, Cow::Owned(_)),
                "expected owned label for {label:?}"
            );
        }
    }

    #[test]
    fn read_only_string_wrappers_match_cow_helpers() {
        let reasons = ["Generated file", "bad\nreason\u{202e}", "\n\u{202e}"];
        for reason in reasons {
            assert_eq!(
                configured_read_only_reason(reason),
                configured_read_only_reason_cow(reason).into_owned()
            );
        }

        let labels = ["main.rs", "bad\nname\u{202e}", "\n\u{202e}"];
        for label in labels {
            assert_eq!(
                read_only_buffer_label(label),
                read_only_buffer_label_cow(label).into_owned()
            );
        }
    }

    #[test]
    fn read_only_toggle_label_sanitizes_and_bounds_buffer_label() {
        let label = read_only_buffer_label(&format!(
            "buffer\nname \u{202e}{}",
            "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
        ));

        assert!(!label.contains('\n'));
        assert!(!label.contains('\u{202e}'));
        assert!(label.contains("..."));
        assert!(label.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn read_only_toggle_tracks_manual_read_only_state() {
        let root = PathBuf::from("workspace");
        let path = root.join("src/main.rs");
        let mut app = app_for_test(root);
        app.buffers
            .push(TextBuffer::from_text(1, Some(path), "text".to_owned()));

        app.toggle_buffer_read_only(1);

        assert!(app.buffer(1).is_some_and(TextBuffer::is_read_only));
        assert!(app.manual_read_only_buffers.contains(&1));

        app.toggle_buffer_read_only(1);

        assert!(!app.buffer(1).is_some_and(TextBuffer::is_read_only));
        assert!(!app.manual_read_only_buffers.contains(&1));
    }

    #[test]
    fn global_read_only_sync_preserves_manual_read_only_buffers_when_disabled() {
        let root = PathBuf::from("workspace");
        let manual_path = root.join("src/manual.rs");
        let global_path = root.join("src/global.rs");
        let mut app = app_for_test(root);
        app.buffers.push(TextBuffer::from_text(
            1,
            Some(manual_path),
            "manual".to_owned(),
        ));
        app.buffers.push(TextBuffer::from_text(
            2,
            Some(global_path),
            "global".to_owned(),
        ));
        app.toggle_buffer_read_only(1);
        app.settings.read_only = true;
        app.sync_global_read_only_buffers();

        assert!(app.buffer(1).is_some_and(TextBuffer::is_read_only));
        assert!(app.buffer(2).is_some_and(TextBuffer::is_read_only));

        app.settings.read_only = false;
        app.sync_global_read_only_buffers();

        assert!(app.buffer(1).is_some_and(TextBuffer::is_read_only));
        assert!(!app.buffer(2).is_some_and(TextBuffer::is_read_only));
        assert!(app.manual_read_only_buffers.contains(&1));
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
