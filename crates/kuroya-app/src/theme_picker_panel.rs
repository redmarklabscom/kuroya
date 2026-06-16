use crate::{
    KuroyaApp,
    path_display::{
        DISPLAY_PATH_LABEL_MAX_CHARS, display_error_label_cow, display_path_label_cow,
        sanitized_display_label_cow,
    },
    theme::{
        THEME_DISPLAY_LABEL_MAX_CHARS, built_in_themes, next_built_in_theme_after,
        plugin_theme_display_label, plugin_theme_display_label_bounded,
        plugin_theme_reference_label, rgb, selected_theme_index_with_plugins, theme_display_label,
        theme_palette,
    },
    ui_state::{handle_list_navigation_keys, selected_row_scroll_offset, selection_page_step},
    workspace_state::settings_path,
};
use eframe::egui::{self, Context, FontFamily, FontId, Key, ScrollArea, Sense, pos2, vec2};
use kuroya_core::{
    PluginThemeRegistration, PluginThemeRegistry, ThemeSettings, load_plugin_theme_settings,
};
use std::borrow::Cow;

const THEME_PICKER_ROW_HEIGHT: f32 = 48.0;
const THEME_PICKER_DEFAULT_FONT_SIZE: f32 = 13.0;

#[derive(Clone)]
enum ThemePickerApply {
    BuiltIn(usize),
    Plugin(PluginThemeRegistration),
}

struct PluginThemePickerRow<'a> {
    label: Cow<'a, str>,
    reference: String,
}

impl KuroyaApp {
    pub(crate) fn cycle_theme(&mut self) {
        let theme = next_built_in_theme_after(&self.settings.theme.name);
        self.apply_theme_preset(theme, false);
    }

    fn apply_theme_preset(&mut self, theme: ThemeSettings, keep_picker_open: bool) {
        self.settings.theme = theme;
        self.theme_picker_selected =
            selected_theme_index_with_plugins(&self.settings.theme, &self.plugin_themes);
        self.theme_dirty = true;
        self.theme_picker_open = keep_picker_open;

        match self.settings.save(&settings_path(&self.workspace.root)) {
            Ok(()) => {
                self.status = theme_applied_status(&self.settings.theme.name);
            }
            Err(error) => {
                self.status = theme_save_failed_status(error);
            }
        }
    }

    fn apply_plugin_theme_preset(
        &mut self,
        registration: PluginThemeRegistration,
        keep_picker_open: bool,
    ) {
        match load_plugin_theme_settings(&registration) {
            Ok(mut theme) => {
                theme.name = plugin_theme_display_label_bounded(&registration);
                self.apply_theme_preset(theme, keep_picker_open);
            }
            Err(error) => {
                self.theme_picker_open = keep_picker_open;
                self.status = plugin_theme_load_failed_status(&registration, error);
            }
        }
    }

    pub(crate) fn render_theme_picker(&mut self, ctx: &Context) {
        let themes = built_in_themes();
        let plugin_themes = self.plugin_themes.themes();
        let theme_count = themes.len().saturating_add(plugin_themes.len());
        let mut selected_theme = self.theme_picker_selected;
        normalize_theme_picker_selection(
            &mut selected_theme,
            &self.settings.theme,
            &self.plugin_themes,
            theme_count,
        );
        let current_palette = theme_palette(&self.settings.theme);
        let label_font = theme_picker_font(self.settings.ui_font_size, 0.0);
        let detail_font = theme_picker_font(self.settings.ui_font_size, -2.0);
        let mut apply = None;
        let mut close = false;

        egui::Window::new("Themes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 72.0])
            .fixed_size([420.0, 300.0])
            .show(ctx, |ui| {
                if ui.input(|input| input.key_pressed(Key::Escape)) {
                    close = true;
                }
                let viewport_height = ui.available_height();
                let selection_changed = ui.input(|input| {
                    handle_list_navigation_keys(
                        input,
                        &mut selected_theme,
                        theme_count,
                        selection_page_step(THEME_PICKER_ROW_HEIGHT, viewport_height),
                    )
                });
                if ui.input(|input| input.key_pressed(Key::Enter)) {
                    apply = theme_picker_apply(themes, plugin_themes, selected_theme);
                }

                let mut scroll_area = ScrollArea::vertical();
                if selection_changed {
                    scroll_area = scroll_area.vertical_scroll_offset(selected_row_scroll_offset(
                        selected_theme,
                        theme_count,
                        THEME_PICKER_ROW_HEIGHT,
                        viewport_height,
                    ));
                }
                scroll_area.show_rows(ui, THEME_PICKER_ROW_HEIGHT, theme_count, |ui, rows| {
                    for idx in rows {
                        if let Some(theme) = themes.get(idx) {
                            let selected = idx == selected_theme;
                            let palette = theme_palette(theme);
                            let (rect, response) = ui.allocate_exact_size(
                                vec2(ui.available_width(), THEME_PICKER_ROW_HEIGHT),
                                Sense::click(),
                            );
                            if response.clicked() {
                                selected_theme = idx;
                                apply = Some(ThemePickerApply::BuiltIn(idx));
                            }

                            let painter = ui.painter();
                            if selected {
                                painter.rect_filled(rect, 4.0, palette.panel_alt);
                            }
                            painter.text(
                                pos2(rect.left() + 10.0, rect.top() + 12.0),
                                egui::Align2::LEFT_TOP,
                                theme_display_label(&theme.name),
                                label_font.clone(),
                                palette.text,
                            );
                            let swatch_x = rect.right() - 116.0;
                            for (swatch_idx, color) in
                                [theme.background, theme.panel, theme.accent, theme.error]
                                    .into_iter()
                                    .enumerate()
                            {
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        pos2(
                                            swatch_x + swatch_idx as f32 * 26.0,
                                            rect.top() + 12.0,
                                        ),
                                        vec2(18.0, 18.0),
                                    ),
                                    3.0,
                                    rgb(color),
                                );
                            }
                            continue;
                        }

                        let Some(theme) =
                            plugin_theme_for_picker_index(plugin_themes, themes.len(), idx)
                        else {
                            continue;
                        };
                        let selected = idx == selected_theme;
                        let row = prepare_plugin_theme_picker_row(theme);
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(ui.available_width(), THEME_PICKER_ROW_HEIGHT),
                            Sense::click(),
                        );
                        if response.clicked() {
                            selected_theme = idx;
                            apply = Some(ThemePickerApply::Plugin(theme.clone()));
                        }

                        let painter = ui.painter();
                        if selected {
                            painter.rect_filled(rect, 4.0, current_palette.panel_alt);
                        }
                        painter.text(
                            pos2(rect.left() + 10.0, rect.top() + 8.0),
                            egui::Align2::LEFT_TOP,
                            row.label.as_ref(),
                            label_font.clone(),
                            current_palette.text,
                        );
                        painter.text(
                            pos2(rect.left() + 10.0, rect.top() + 28.0),
                            egui::Align2::LEFT_TOP,
                            row.reference.as_str(),
                            detail_font.clone(),
                            current_palette.muted,
                        );
                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                pos2(rect.right() - 38.0, rect.top() + 16.0),
                                vec2(18.0, 18.0),
                            ),
                            3.0,
                            current_palette.accent,
                        );
                    }
                });
            });

        self.theme_picker_selected = selected_theme;
        if close {
            self.theme_picker_open = false;
            self.status = "Closed theme picker".to_owned();
        } else if let Some(theme) = apply {
            match theme {
                ThemePickerApply::BuiltIn(index) => {
                    if let Some(theme) = themes.get(index).cloned() {
                        self.apply_theme_preset(theme, true);
                    }
                }
                ThemePickerApply::Plugin(registration) => {
                    self.apply_plugin_theme_preset(registration, true);
                }
            }
        }
    }
}

fn prepare_plugin_theme_picker_row(
    registration: &PluginThemeRegistration,
) -> PluginThemePickerRow<'_> {
    PluginThemePickerRow {
        label: plugin_theme_picker_display_label(registration),
        reference: plugin_theme_reference_label(registration),
    }
}

fn plugin_theme_picker_display_label(registration: &PluginThemeRegistration) -> Cow<'_, str> {
    sanitized_display_label_cow(
        plugin_theme_display_label(registration),
        THEME_DISPLAY_LABEL_MAX_CHARS,
        "Plugin theme",
    )
}

fn plugin_theme_for_picker_index(
    plugin_themes: &[PluginThemeRegistration],
    built_in_count: usize,
    selected: usize,
) -> Option<&PluginThemeRegistration> {
    let plugin_idx = selected.checked_sub(built_in_count)?;
    plugin_themes.get(plugin_idx)
}

fn normalize_theme_picker_selection(
    selected: &mut usize,
    theme: &ThemeSettings,
    plugin_themes: &PluginThemeRegistry,
    theme_count: usize,
) {
    if theme_count == 0 {
        *selected = 0;
        return;
    }
    if *selected >= theme_count {
        *selected = selected_theme_index_with_plugins(theme, plugin_themes).min(theme_count - 1);
    }
}

fn theme_picker_font(ui_font_size: f32, delta: f32) -> FontId {
    let size = if ui_font_size.is_finite() {
        (ui_font_size + delta).clamp(10.0, 24.0)
    } else {
        THEME_PICKER_DEFAULT_FONT_SIZE
    };
    FontId::new(size, FontFamily::Proportional)
}

fn theme_picker_apply(
    built_in_themes: &[ThemeSettings],
    plugin_themes: &[PluginThemeRegistration],
    selected: usize,
) -> Option<ThemePickerApply> {
    if selected >= built_in_themes.len().saturating_add(plugin_themes.len()) {
        return None;
    }

    if selected < built_in_themes.len() {
        Some(ThemePickerApply::BuiltIn(selected))
    } else {
        plugin_theme_for_picker_index(plugin_themes, built_in_themes.len(), selected)
            .cloned()
            .map(ThemePickerApply::Plugin)
    }
}

fn theme_status_label(label: &str) -> Cow<'_, str> {
    sanitized_display_label_cow(label, DISPLAY_PATH_LABEL_MAX_CHARS, "Unnamed theme")
}

fn theme_applied_status(theme_name: &str) -> String {
    let label = theme_status_label(theme_name);
    let mut status = String::with_capacity("Theme: ".len() + label.len());
    status.push_str("Theme: ");
    status.push_str(label.as_ref());
    status
}

fn theme_save_failed_status(error: impl std::fmt::Display) -> String {
    let error = error.to_string();
    let error = display_error_label_cow(&error);
    let prefix = "Theme changed, but settings save failed: ";
    let mut status = String::with_capacity(prefix.len() + error.len());
    status.push_str(prefix);
    status.push_str(error.as_ref());
    status
}

fn plugin_theme_load_failed_status(
    registration: &PluginThemeRegistration,
    error: impl std::fmt::Display,
) -> String {
    let error = error.to_string();
    let label = theme_status_label(plugin_theme_display_label(registration));
    let path = display_path_label_cow(&registration.path);
    let error = display_error_label_cow(&error);
    let mut status = String::with_capacity(
        "Could not load plugin theme  from : ".len() + label.len() + path.len() + error.len(),
    );
    status.push_str("Could not load plugin theme ");
    status.push_str(label.as_ref());
    status.push_str(" from ");
    status.push_str(path.as_ref());
    status.push_str(": ");
    status.push_str(error.as_ref());
    status
}

#[cfg(test)]
mod tests {
    use super::{
        ThemePickerApply, normalize_theme_picker_selection, plugin_theme_load_failed_status,
        prepare_plugin_theme_picker_row, theme_applied_status, theme_picker_apply,
        theme_picker_font, theme_save_failed_status,
    };
    use crate::path_display::{DISPLAY_ERROR_LABEL_MAX_CHARS, DISPLAY_PATH_LABEL_MAX_CHARS};
    use crate::theme::{THEME_DISPLAY_LABEL_MAX_CHARS, THEME_REFERENCE_LABEL_MAX_CHARS};
    use egui::FontFamily;
    use kuroya_core::{
        PLUGIN_API_VERSION, PluginCapabilities, PluginContributions, PluginDescriptor,
        PluginManifest, PluginThemeContribution, PluginThemeRegistration, PluginThemeRegistry,
        ThemeSettings,
    };
    use std::{borrow::Cow, path::PathBuf};

    #[test]
    fn normalize_theme_picker_selection_resets_invalid_state_to_active_theme() {
        let registry = PluginThemeRegistry::default();
        let theme = ThemeSettings {
            name: "Graphite".to_owned(),
            ..ThemeSettings::default()
        };
        let mut selected = usize::MAX;

        normalize_theme_picker_selection(
            &mut selected,
            &theme,
            &registry,
            ThemeSettings::built_in_presets().len(),
        );

        assert_eq!(ThemeSettings::built_in_presets()[selected].name, "Graphite");
    }

    #[test]
    fn normalize_theme_picker_selection_clamps_empty_lists_to_zero() {
        let mut selected = 99;

        normalize_theme_picker_selection(
            &mut selected,
            &ThemeSettings::default(),
            &PluginThemeRegistry::default(),
            0,
        );

        assert_eq!(selected, 0);
    }

    #[test]
    fn theme_picker_apply_handles_builtins_plugins_and_invalid_indices() {
        let builtins = ThemeSettings::built_in_presets();
        let plugin = PluginThemeRegistration {
            plugin_id: "solar.plugin".to_owned(),
            theme_id: "solar-dark".to_owned(),
            label: "Solar Dark".to_owned(),
            path: PathBuf::from("themes/dark.toml"),
        };
        let plugins = vec![plugin.clone()];

        assert!(matches!(
            theme_picker_apply(&builtins, &plugins, 0),
            Some(ThemePickerApply::BuiltIn(_))
        ));
        assert!(matches!(
            theme_picker_apply(&builtins, &plugins, builtins.len()),
            Some(ThemePickerApply::Plugin(registration)) if registration == plugin
        ));
        assert!(theme_picker_apply(&builtins, &[], builtins.len()).is_none());
        assert!(theme_picker_apply(&builtins, &plugins, usize::MAX).is_none());
    }

    #[test]
    fn prepare_plugin_theme_picker_row_prepares_labels_without_mutating_registration() {
        let plugin = PluginThemeRegistration {
            plugin_id: " solar.plugin ".to_owned(),
            theme_id: " solar-dark ".to_owned(),
            label: "  ".to_owned(),
            path: PathBuf::from("themes/dark.toml"),
        };

        let row = prepare_plugin_theme_picker_row(&plugin);

        assert_eq!(row.label.as_ref(), "solar-dark");
        assert_eq!(row.reference.as_str(), "solar.plugin:solar-dark");
        assert_eq!(plugin.label, "  ");
    }

    #[test]
    fn prepare_plugin_theme_picker_row_borrows_clean_display_label() {
        let plugin = PluginThemeRegistration {
            plugin_id: "solar.plugin".to_owned(),
            theme_id: "solar-dark".to_owned(),
            label: "Solar Dark".to_owned(),
            path: PathBuf::from("themes/dark.toml"),
        };

        let row = prepare_plugin_theme_picker_row(&plugin);

        assert_eq!(row.reference.as_str(), "solar.plugin:solar-dark");
        assert!(matches!(row.label, Cow::Borrowed(label) if label == "Solar Dark"));
    }

    #[test]
    fn prepare_plugin_theme_picker_row_bounds_display_labels() {
        let plugin = PluginThemeRegistration {
            plugin_id: format!("plugin\n{}\u{2066}tail", "very-long-plugin-id-".repeat(16)),
            theme_id: format!("theme\n{}\u{2067}tail", "very-long-theme-id-".repeat(16)),
            label: format!("Theme\n{}\u{202e}tail", "very-long-theme-label-".repeat(16)),
            path: PathBuf::from("themes/dark.toml"),
        };

        let row = prepare_plugin_theme_picker_row(&plugin);

        assert!(!row.label.contains('\n'));
        assert!(!row.label.contains('\u{202e}'));
        assert!(row.label.contains("..."));
        assert!(row.label.chars().count() <= THEME_DISPLAY_LABEL_MAX_CHARS);
        assert!(matches!(row.label, Cow::Owned(_)));
        assert!(!row.reference.contains('\n'));
        assert!(!row.reference.contains('\u{2066}'));
        assert!(!row.reference.contains('\u{2067}'));
        assert!(row.reference.contains("..."));
        assert!(row.reference.chars().count() <= THEME_REFERENCE_LABEL_MAX_CHARS);
    }

    #[test]
    fn theme_picker_font_uses_safe_bounds() {
        assert_eq!(theme_picker_font(f32::NAN, 0.0).size, 13.0);
        assert_eq!(theme_picker_font(2.0, 0.0).size, 10.0);
        assert_eq!(theme_picker_font(42.0, 0.0).size, 24.0);
        assert_eq!(
            theme_picker_font(14.0, 0.0).family,
            FontFamily::Proportional
        );
    }

    #[test]
    fn normalize_theme_picker_selection_can_restore_plugin_theme_index() {
        let registry = PluginThemeRegistry::from_plugins(&[PluginDescriptor {
            root: PathBuf::from("workspace/.kuroya/plugins/solar"),
            manifest: PluginManifest {
                api_version: PLUGIN_API_VERSION.to_owned(),
                id: "solar.plugin".to_owned(),
                name: "Solar".to_owned(),
                version: "0.1.0".to_owned(),
                entry: None,
                activation_events: Vec::new(),
                capabilities: PluginCapabilities {
                    themes: true,
                    ..PluginCapabilities::default()
                },
                contributes: PluginContributions {
                    themes: vec![PluginThemeContribution {
                        id: "solar-dark".to_owned(),
                        label: "Solar Dark".to_owned(),
                        path: PathBuf::from("workspace/.kuroya/plugins/solar/themes/dark.toml"),
                    }],
                    ..PluginContributions::default()
                },
            },
        }]);
        let mut selected = usize::MAX;

        normalize_theme_picker_selection(
            &mut selected,
            &ThemeSettings {
                name: "solar.plugin:solar-dark".to_owned(),
                ..ThemeSettings::default()
            },
            &registry,
            ThemeSettings::built_in_presets().len() + registry.len(),
        );

        assert_eq!(selected, ThemeSettings::built_in_presets().len());
    }

    #[test]
    fn theme_applied_status_sanitizes_and_bounds_theme_name() {
        let status = theme_applied_status(&format!(
            "theme\n{}\u{202e}tail",
            "very-long-theme-name-".repeat(16)
        ));

        assert!(status.starts_with("Theme: theme "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(status.contains("..."));
        assert!(status.chars().count() <= "Theme: ".chars().count() + DISPLAY_PATH_LABEL_MAX_CHARS);
    }

    #[test]
    fn theme_applied_status_falls_back_for_blank_control_theme_name() {
        assert_eq!(
            theme_applied_status("\n\u{202e}\u{0007}"),
            "Theme: Unnamed theme"
        );
    }

    #[test]
    fn theme_save_failed_status_sanitizes_and_bounds_error() {
        let status = theme_save_failed_status(format!(
            "first line\n{}\u{2066}tail",
            "settings-save-error-".repeat(16)
        ));

        assert!(status.starts_with("Theme changed, but settings save failed: first line "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{2066}'));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Theme changed, but settings save failed: ".chars().count()
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }

    #[test]
    fn plugin_theme_load_failed_status_sanitizes_label_path_and_error() {
        let registration = PluginThemeRegistration {
            plugin_id: "plugin.id".to_owned(),
            theme_id: "theme.id".to_owned(),
            label: format!(
                "plugin\n{}\u{202e}tail",
                "very-long-plugin-theme-label-".repeat(10)
            ),
            path: PathBuf::from("plugins").join(format!(
                "bad\n{}\u{2067}.toml",
                "very-long-theme-path-".repeat(16)
            )),
        };
        let status = plugin_theme_load_failed_status(
            &registration,
            format!(
                "load failed\n{}\u{200f}tail",
                "theme-load-error-".repeat(16)
            ),
        );

        assert!(status.starts_with("Could not load plugin theme plugin "));
        assert!(!status.contains('\n'));
        assert!(!status.contains('\u{202e}'));
        assert!(!status.contains('\u{2067}'));
        assert!(!status.contains('\u{200f}'));
        assert!(status.contains("bad "));
        assert!(status.contains("load failed "));
        assert!(status.contains("..."));
        assert!(
            status.chars().count()
                <= "Could not load plugin theme  from : ".chars().count()
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + DISPLAY_PATH_LABEL_MAX_CHARS
                    + DISPLAY_ERROR_LABEL_MAX_CHARS
        );
    }
}
