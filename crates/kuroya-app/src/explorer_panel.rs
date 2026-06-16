use crate::{
    KuroyaApp,
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_path_label_cow, sanitized_display_label_cow,
    },
    ui_icons::{IconKind, icon_button},
};
use eframe::egui::{self, Align, RichText};
use kuroya_core::Command;
use std::{borrow::Cow, path::Path};

impl KuroyaApp {
    pub(crate) fn render_explorer(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Explorer").strong());
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, IconKind::FolderOpen, "Open folder").clicked() {
                        self.command_bus.push(Command::OpenWorkspacePrompt);
                    }
                    if !self.workspace_placeholder {
                        if icon_button(ui, IconKind::Refresh, "Refresh workspace").clicked() {
                            self.command_bus.push(Command::RefreshWorkspace);
                        }
                        if icon_button(ui, IconKind::Plus, "New file in workspace").clicked() {
                            self.command_bus
                                .push(Command::CreateFileIn(self.workspace.root.clone()));
                        }
                    }
                });
            });
            let root_label = if self.workspace_placeholder {
                "No folder open".to_owned()
            } else {
                explorer_workspace_root_label(&self.workspace.root)
            };
            let root_response = ui.label(RichText::new(root_label).small());
            root_response.context_menu(|ui| {
                if ui.button("Open Folder").clicked() {
                    self.command_bus.push(Command::OpenWorkspacePrompt);
                    ui.close();
                }
                if !self.workspace_placeholder && ui.button("New File").clicked() {
                    self.command_bus
                        .push(Command::CreateFileIn(self.workspace.root.clone()));
                    ui.close();
                }
                if !self.workspace_placeholder && ui.button("New Folder").clicked() {
                    self.command_bus
                        .push(Command::CreateFolderIn(self.workspace.root.clone()));
                    ui.close();
                }
                if !self.workspace_placeholder && ui.button("Refresh").clicked() {
                    self.command_bus.push(Command::RefreshWorkspace);
                    ui.close();
                }
                if !self.workspace_placeholder && ui.button("Show Path").clicked() {
                    self.status = explorer_workspace_root_status(&self.workspace.root);
                    ui.close();
                }
            });
            ui.separator();
            self.render_explorer_tree(ui);
        });
    }
}

fn explorer_workspace_root_label(path: &Path) -> String {
    display_path_label_cow(path).into_owned()
}

fn explorer_workspace_root_status(path: &Path) -> String {
    explorer_workspace_root_status_text(path).into_owned()
}

fn explorer_workspace_root_status_text(path: &Path) -> Cow<'_, str> {
    if let Some(path) = path.to_str() {
        sanitized_display_label_cow(path, DISPLAY_PATH_LABEL_MAX_CHARS, ".")
    } else {
        let display = path.display().to_string();
        Cow::Owned(
            sanitized_display_label_cow(&display, DISPLAY_PATH_LABEL_MAX_CHARS, ".").into_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        explorer_workspace_root_label, explorer_workspace_root_status,
        explorer_workspace_root_status_text,
    };
    use crate::path_display::DISPLAY_PATH_LABEL_MAX_CHARS;
    use std::{
        borrow::Cow,
        path::{Path, PathBuf},
    };

    #[test]
    fn explorer_workspace_root_status_text_borrows_clean_ascii_and_unicode() {
        assert!(matches!(
            explorer_workspace_root_status_text(Path::new("workspace")),
            Cow::Borrowed("workspace")
        ));

        let unicode = "workspace-\u{03bb}";
        match explorer_workspace_root_status_text(Path::new(unicode)) {
            Cow::Borrowed(label) => assert_eq!(label, unicode),
            Cow::Owned(label) => panic!("expected borrowed label, got {label:?}"),
        }
    }

    #[test]
    fn explorer_workspace_root_status_text_owns_dirty_truncated_and_fallback_labels() {
        let long = format!("workspace-{}", "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2));
        let cases = [
            PathBuf::from("workspace\n\u{202e}root"),
            PathBuf::from(long),
            PathBuf::from("\n\u{202e}"),
        ];

        for root in cases {
            let status = explorer_workspace_root_status_text(&root);

            assert_eq!(status.as_ref(), explorer_workspace_root_status(&root));
            assert!(
                matches!(&status, Cow::Owned(_)),
                "expected owned status for {root:?}"
            );
        }

        let long_root = PathBuf::from(format!(
            "workspace-{}",
            "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2)
        ));
        let truncated = explorer_workspace_root_status_text(&long_root);
        assert!(truncated.contains("..."));
        assert!(truncated.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);

        let fallback_root = PathBuf::from("\n\u{202e}");
        assert_eq!(
            explorer_workspace_root_status_text(&fallback_root).as_ref(),
            "."
        );
    }

    #[test]
    fn explorer_workspace_root_status_wrapper_matches_cow_helper() {
        let long = format!("workspace-{}", "x".repeat(DISPLAY_PATH_LABEL_MAX_CHARS * 2));
        let roots = [
            PathBuf::from("workspace"),
            PathBuf::from("workspace-\u{03bb}"),
            PathBuf::from("workspace\n\u{202e}root"),
            PathBuf::from(long),
            PathBuf::from("\n\u{202e}"),
        ];

        for root in roots {
            assert_eq!(
                explorer_workspace_root_status(&root),
                explorer_workspace_root_status_text(&root).into_owned()
            );
        }
    }

    #[test]
    fn explorer_workspace_root_labels_are_display_safe_and_bounded() {
        let root = PathBuf::from("workspace").join(format!(
            "bad\n\u{202e}root{}",
            "-segment".repeat(DISPLAY_PATH_LABEL_MAX_CHARS)
        ));

        let label = explorer_workspace_root_label(&root);
        let status = explorer_workspace_root_status(&root);

        for value in [label, status] {
            assert!(!value.contains('\n'));
            assert!(!value.contains('\u{202e}'));
            assert!(value.contains("..."));
            assert!(value.chars().count() <= DISPLAY_PATH_LABEL_MAX_CHARS);
        }
    }

    #[test]
    fn explorer_workspace_root_status_preserves_raw_pathbuf() {
        let root = PathBuf::from("workspace").join("raw\n\u{202e}root");
        let original = root.clone();

        let status = explorer_workspace_root_status(&root);

        assert_eq!(root, original);
        assert!(root.as_os_str().to_string_lossy().contains('\n'));
        assert!(root.as_os_str().to_string_lossy().contains('\u{202e}'));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
    }
}
