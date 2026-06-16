use crate::{
    popup_buttons::{PopupButtonKind, popup_compact_button},
    preference_panels::sections::{
        SETTINGS_DISPLAY_TEXT_MAX_CHARS, SETTINGS_TARGET_TERMINAL_PROFILE, SettingsHighlightState,
        bounded_settings_display_text, bounded_settings_multiline_input,
        bounded_settings_singleline_input, settings_target_heading,
    },
    terminal_process::{TerminalShellProfile, default_shell_label, detected_shell_profiles},
};
use eframe::egui;
use kuroya_core::{EditorSettings, TerminalSplitCwd};

pub(super) fn render_profile_settings_with_highlight(
    ui: &mut egui::Ui,
    draft: &mut EditorSettings,
    highlight: &mut SettingsHighlightState<'_>,
) {
    settings_target_heading(ui, highlight, SETTINGS_TARGET_TERMINAL_PROFILE, "Profile");
    egui::Grid::new("settings_terminal_profile_grid")
        .num_columns(2)
        .spacing([18.0, 10.0])
        .show(ui, |ui| {
            ui.label("Detected profile");
            render_detected_profile_picker(ui, draft);
            ui.end_row();

            ui.label("Shell executable");
            render_shell_path_input(ui, draft);
            ui.end_row();

            ui.label("Shell arguments");
            render_shell_args_input(ui, draft);
            ui.end_row();

            ui.label("Start directory");
            render_terminal_cwd_input(ui, draft);
            ui.end_row();

            ui.label("Split start directory");
            terminal_split_cwd_combo(ui, "terminal_split_cwd", &mut draft.terminal_split_cwd);
            ui.end_row();
        });
}

fn render_detected_profile_picker(ui: &mut egui::Ui, draft: &mut EditorSettings) {
    let profiles = detected_shell_profiles();
    let selected = selected_profile_label(&profiles, draft);
    egui::ComboBox::from_id_salt("terminal_detected_profile")
        .width(bounded_setting_width(ui.available_width()))
        .selected_text(selected.as_str())
        .show_ui(ui, |ui| {
            for profile in profiles {
                let selected = profile_matches_settings(&profile, draft);
                let label = bounded_settings_display_text(
                    &profile.label,
                    SETTINGS_DISPLAY_TEXT_MAX_CHARS,
                    "Shell profile",
                );
                if ui.selectable_label(selected, label).clicked() {
                    apply_detected_profile(draft, profile);
                    ui.close();
                }
            }
        });
}

fn apply_detected_profile(draft: &mut EditorSettings, profile: TerminalShellProfile) {
    draft.terminal_shell_path = Some(profile.path);
    draft.terminal_shell_args = profile.args;
}

fn render_shell_path_input(ui: &mut egui::Ui, draft: &mut EditorSettings) {
    let mut value =
        bounded_settings_singleline_input(draft.terminal_shell_path.as_deref().unwrap_or_default());
    ui.horizontal(|ui| {
        let input_width = bounded_setting_width(ui.available_width() - 92.0);
        let response = ui.add_sized(
            [input_width, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut value).hint_text(default_shell_label()),
        );
        if response.changed() {
            draft.terminal_shell_path = non_empty_shell_setting(value.as_str());
        }
        if popup_compact_button(ui, "Default", PopupButtonKind::Secondary).clicked() {
            reset_shell_profile(draft);
        }
    });
}

fn reset_shell_profile(draft: &mut EditorSettings) {
    draft.terminal_shell_path = None;
    draft.terminal_shell_args.clear();
}

fn render_shell_args_input(ui: &mut egui::Ui, draft: &mut EditorSettings) {
    let mut value =
        bounded_settings_multiline_input(&terminal_shell_args_text(&draft.terminal_shell_args));
    let response = ui.add_sized(
        [bounded_setting_width(ui.available_width()), 72.0],
        egui::TextEdit::multiline(&mut value)
            .desired_rows(3)
            .hint_text("One argument per line"),
    );
    if response.changed() {
        draft.terminal_shell_args = non_empty_shell_settings(&value);
    }
}

fn terminal_shell_args_text(args: &[String]) -> String {
    let Some((first, rest)) = args.split_first() else {
        return String::new();
    };
    let mut value = String::with_capacity(
        args.iter()
            .map(String::len)
            .sum::<usize>()
            .saturating_add(args.len().saturating_sub(1)),
    );
    value.push_str(first);
    for arg in rest {
        value.push('\n');
        value.push_str(arg);
    }
    value
}

fn non_empty_shell_settings(value: &str) -> Vec<String> {
    let mut args = Vec::new();
    for line in value.lines() {
        if let Some(arg) = non_empty_shell_setting(line) {
            args.push(arg);
        }
    }
    args
}

fn render_terminal_cwd_input(ui: &mut egui::Ui, draft: &mut EditorSettings) {
    let mut value =
        bounded_settings_singleline_input(draft.terminal_cwd.as_deref().unwrap_or_default());
    ui.horizontal(|ui| {
        let input_width = bounded_setting_width(ui.available_width() - 92.0);
        let response = ui.add_sized(
            [input_width, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut value).hint_text("Workspace root"),
        );
        if response.changed() {
            draft.terminal_cwd = non_empty_setting(value.as_str());
        }
        if popup_compact_button(ui, "Default", PopupButtonKind::Secondary).clicked() {
            draft.terminal_cwd = None;
        }
    });
}

fn bounded_setting_width(width: f32) -> f32 {
    width.clamp(96.0, 280.0)
}

fn non_empty_setting(value: &str) -> Option<String> {
    sanitized_terminal_setting(value).map(str::to_owned)
}

fn non_empty_shell_setting(value: &str) -> Option<String> {
    sanitized_terminal_setting(value).map(str::to_owned)
}

fn sanitized_terminal_setting(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && !contains_terminal_setting_hidden_or_control(trimmed))
        .then_some(trimmed)
}

fn contains_terminal_setting_hidden_or_control(value: &str) -> bool {
    value.chars().any(|ch| {
        ch.is_control()
            || matches!(ch, '\u{2028}' | '\u{2029}')
            || is_hidden_terminal_setting_control(ch)
    })
}

fn is_hidden_terminal_setting_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

fn selected_profile_label(profiles: &[TerminalShellProfile], draft: &EditorSettings) -> String {
    profiles
        .iter()
        .find(|profile| profile_matches_settings(profile, draft))
        .map(|profile| {
            bounded_settings_display_text(
                &profile.label,
                SETTINGS_DISPLAY_TEXT_MAX_CHARS,
                "Shell profile",
            )
        })
        .or_else(|| {
            draft
                .terminal_shell_path
                .as_deref()
                .and_then(sanitized_terminal_setting)
                .map(|path| {
                    bounded_settings_display_text(
                        path,
                        SETTINGS_DISPLAY_TEXT_MAX_CHARS,
                        "Shell profile",
                    )
                })
        })
        .unwrap_or_else(default_shell_label)
}

fn profile_matches_settings(profile: &TerminalShellProfile, draft: &EditorSettings) -> bool {
    draft.terminal_shell_path.as_deref() == Some(profile.path.as_str())
        && draft.terminal_shell_args == profile.args
}

fn terminal_split_cwd_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut TerminalSplitCwd,
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(terminal_split_cwd_label(*value))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, TerminalSplitCwd::Inherited, "Inherited");
            ui.selectable_value(value, TerminalSplitCwd::Initial, "Initial");
            ui.selectable_value(value, TerminalSplitCwd::WorkspaceRoot, "Workspace root");
        });
}

fn terminal_split_cwd_label(split_cwd: TerminalSplitCwd) -> &'static str {
    match split_cwd {
        TerminalSplitCwd::Inherited => "Inherited",
        TerminalSplitCwd::Initial => "Initial",
        TerminalSplitCwd::WorkspaceRoot => "Workspace root",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_profile_settings_reject_control_characters() {
        assert_eq!(
            non_empty_shell_setting(" pwsh.exe "),
            Some("pwsh.exe".to_owned())
        );
        assert_eq!(non_empty_shell_setting(" pwsh.exe\n-NoProfile "), None);
        assert_eq!(non_empty_shell_setting(" pwsh\u{202e}.exe "), None);
        assert_eq!(non_empty_shell_setting(" \u{7} "), None);
        assert_eq!(non_empty_shell_setting(" "), None);
    }

    #[test]
    fn terminal_cwd_setting_rejects_control_characters() {
        assert_eq!(non_empty_setting(" tools "), Some("tools".to_owned()));
        assert_eq!(non_empty_setting(" tools\nbad "), None);
        assert_eq!(non_empty_setting(" tools\tbad "), None);
        assert_eq!(non_empty_setting(" tools\u{2028}bad "), None);
        assert_eq!(non_empty_setting(" tools\u{200b}bad "), None);
        assert_eq!(non_empty_setting(" \u{7} "), None);
        assert_eq!(non_empty_setting(" "), None);
    }

    #[test]
    fn selected_profile_label_ignores_unsafe_custom_shell_path() {
        let profiles = [TerminalShellProfile {
            label: "PowerShell".to_owned(),
            path: "pwsh.exe".to_owned(),
            args: Vec::new(),
        }];
        let mut draft = EditorSettings {
            terminal_shell_path: Some("pwsh.exe\n-NoProfile".to_owned()),
            ..EditorSettings::default()
        };

        assert_eq!(
            selected_profile_label(&profiles, &draft),
            default_shell_label().as_str()
        );

        draft.terminal_shell_path = Some(" custom-shell ".to_owned());

        assert_eq!(selected_profile_label(&profiles, &draft), "custom-shell");
    }

    #[test]
    fn detected_profile_selection_updates_executable_and_args() {
        let mut draft = EditorSettings::default();

        apply_detected_profile(
            &mut draft,
            TerminalShellProfile {
                label: "Command Prompt".to_owned(),
                path: "cmd.exe".to_owned(),
                args: vec!["/Q".to_owned()],
            },
        );

        assert_eq!(draft.terminal_shell_path.as_deref(), Some("cmd.exe"));
        assert_eq!(draft.terminal_shell_args, ["/Q".to_owned()]);
    }

    #[test]
    fn resetting_shell_profile_clears_stale_provider_args() {
        let mut draft = EditorSettings {
            terminal_shell_path: Some("pwsh.exe".to_owned()),
            terminal_shell_args: vec!["-NoLogo".to_owned(), "-NoProfile".to_owned()],
            ..EditorSettings::default()
        };

        reset_shell_profile(&mut draft);

        assert_eq!(draft.terminal_shell_path, None);
        assert!(draft.terminal_shell_args.is_empty());
    }
}
