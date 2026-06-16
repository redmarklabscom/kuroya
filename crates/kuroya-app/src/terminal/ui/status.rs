use super::TerminalCommandStatus;
use egui::{Color32, Sense, vec2};
use std::{borrow::Cow, fmt::Write as _};

pub(super) struct TerminalSessionLabelContext<'a> {
    pub(super) display_shell_label: Option<Cow<'a, str>>,
    pub(super) uses_default_title_template: bool,
}

impl<'a> TerminalSessionLabelContext<'a> {
    pub(super) fn new(_shell_label: &'a str, tabs_title_template: &str) -> Self {
        Self {
            display_shell_label: Some(Cow::Borrowed(super::super::TERMINAL_DEFAULT_DISPLAY_LABEL)),
            uses_default_title_template: tabs_title_template.trim()
                == super::DEFAULT_TERMINAL_TABS_TITLE,
        }
    }

    pub(super) fn shell_display_label(&self) -> Option<Cow<'_, str>> {
        self.display_shell_label
            .as_ref()
            .map(|label| Cow::Borrowed(label.as_ref()))
    }

    pub(super) fn shell_tooltip_label(&self) -> Cow<'_, str> {
        self.shell_display_label()
            .unwrap_or(Cow::Borrowed(super::super::TERMINAL_DEFAULT_DISPLAY_LABEL))
    }
}

pub(super) fn terminal_command_status_dot(
    ui: &mut egui::Ui,
    status: TerminalCommandStatus,
    text_color: Color32,
) {
    let (rect, response) = ui.allocate_exact_size(vec2(12.0, 18.0), Sense::hover());
    if let Some(color) = terminal_command_status_color(status, text_color) {
        ui.painter().circle_filled(rect.center(), 3.5, color);
    }
    response.on_hover_text(terminal_command_status_tooltip(status));
}

fn terminal_command_status_color(
    status: TerminalCommandStatus,
    text_color: Color32,
) -> Option<Color32> {
    match status {
        TerminalCommandStatus::Unknown => None,
        TerminalCommandStatus::Prompt => Some(text_color.gamma_multiply(0.64)),
        TerminalCommandStatus::Running => Some(Color32::from_rgb(231, 185, 87)),
        TerminalCommandStatus::Succeeded => Some(Color32::from_rgb(72, 184, 112)),
        TerminalCommandStatus::Failed(_) => Some(Color32::from_rgb(218, 76, 76)),
        TerminalCommandStatus::TerminalError => Some(Color32::from_rgb(218, 76, 76)),
        TerminalCommandStatus::Finished => Some(text_color.gamma_multiply(0.55)),
        TerminalCommandStatus::Stopped => Some(text_color.gamma_multiply(0.42)),
    }
}

const TERMINAL_SESSION_TOOLTIP: &str = "Terminal session";
const TERMINAL_SESSION_TOOLTIP_PREFIX: &str = "Terminal session\n";
const TERMINAL_FAILED_COMMAND_TOOLTIP_PREFIX: &str = "Command exited with code ";
const TERMINAL_EXIT_CODE_MAX_CHARS: usize = "-2147483648".len();

pub(super) fn terminal_profile_tab_tooltip(status: TerminalCommandStatus) -> Cow<'static, str> {
    match terminal_profile_tab_static_tooltip(status) {
        Some(tooltip) => Cow::Borrowed(tooltip),
        None => match status {
            TerminalCommandStatus::Failed(exit_code) => {
                let mut tooltip = String::with_capacity(
                    TERMINAL_SESSION_TOOLTIP_PREFIX.len()
                        + TERMINAL_FAILED_COMMAND_TOOLTIP_PREFIX.len()
                        + TERMINAL_EXIT_CODE_MAX_CHARS,
                );
                tooltip.push_str(TERMINAL_SESSION_TOOLTIP_PREFIX);
                write_terminal_failed_command_tooltip(&mut tooltip, exit_code);
                Cow::Owned(tooltip)
            }
            _ => unreachable!("static terminal profile tooltip handled above"),
        },
    }
}

fn terminal_profile_tab_static_tooltip(status: TerminalCommandStatus) -> Option<&'static str> {
    match status {
        TerminalCommandStatus::Unknown => Some(TERMINAL_SESSION_TOOLTIP),
        TerminalCommandStatus::Prompt => Some("Terminal session\nShell prompt active"),
        TerminalCommandStatus::Running => Some("Terminal session\nShell command running"),
        TerminalCommandStatus::Succeeded => Some("Terminal session\nCommand succeeded"),
        TerminalCommandStatus::Failed(_) => None,
        TerminalCommandStatus::TerminalError => Some("Terminal session\nTerminal process failed"),
        TerminalCommandStatus::Finished => Some("Terminal session\nShell command finished"),
        TerminalCommandStatus::Stopped => Some("Terminal session\nTerminal stopped"),
    }
}

fn terminal_command_status_static_tooltip(status: TerminalCommandStatus) -> Option<&'static str> {
    match status {
        TerminalCommandStatus::Unknown => Some("No shell integration status"),
        TerminalCommandStatus::Prompt => Some("Shell prompt active"),
        TerminalCommandStatus::Running => Some("Shell command running"),
        TerminalCommandStatus::Succeeded => Some("Command succeeded"),
        TerminalCommandStatus::Failed(_) => None,
        TerminalCommandStatus::TerminalError => Some("Terminal process failed"),
        TerminalCommandStatus::Finished => Some("Shell command finished"),
        TerminalCommandStatus::Stopped => Some("Terminal stopped"),
    }
}

#[cfg(test)]
pub(super) fn terminal_command_status_tooltip(status: TerminalCommandStatus) -> Cow<'static, str> {
    terminal_command_status_tooltip_inner(status)
}

#[cfg(not(test))]
fn terminal_command_status_tooltip(status: TerminalCommandStatus) -> Cow<'static, str> {
    terminal_command_status_tooltip_inner(status)
}

fn terminal_command_status_tooltip_inner(status: TerminalCommandStatus) -> Cow<'static, str> {
    match terminal_command_status_static_tooltip(status) {
        Some(tooltip) => Cow::Borrowed(tooltip),
        None => match status {
            TerminalCommandStatus::Failed(exit_code) => {
                let mut tooltip = String::with_capacity(
                    TERMINAL_FAILED_COMMAND_TOOLTIP_PREFIX.len() + TERMINAL_EXIT_CODE_MAX_CHARS,
                );
                write_terminal_failed_command_tooltip(&mut tooltip, exit_code);
                Cow::Owned(tooltip)
            }
            _ => unreachable!("static terminal command status tooltip handled above"),
        },
    }
}

fn write_terminal_failed_command_tooltip(tooltip: &mut String, exit_code: i32) {
    tooltip.push_str(TERMINAL_FAILED_COMMAND_TOOLTIP_PREFIX);
    let _ = write!(tooltip, "{exit_code}");
}
