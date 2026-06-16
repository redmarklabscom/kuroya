use crate::{
    font_candidates::{editor_font_candidates_for_family_stack, ui_font_candidates},
    font_loading::load_font_stack_bytes,
};
use egui::{self, Context, FontFamily, FontId, TextStyle};
use kuroya_core::EditorSettings;
use std::{path::Path, sync::Arc};

const MAX_EDITOR_FONT_FALLBACKS: usize = 6;
const MAX_UI_FONT_FALLBACKS: usize = 4;
const MIN_EDITOR_FONT_SIZE: f32 = 10.0;
const MAX_EDITOR_FONT_SIZE: f32 = 28.0;
const DEFAULT_EDITOR_FONT_SIZE: f32 = 13.0;
const MIN_UI_FONT_SIZE: f32 = 10.0;
const MAX_UI_FONT_SIZE: f32 = 24.0;
const DEFAULT_UI_FONT_SIZE: f32 = 13.0;

pub(crate) fn install_fonts(ctx: &Context, workspace_root: &Path, settings: &EditorSettings) {
    let mut fonts = egui::FontDefinitions::default();
    prepend_font_stack(
        &mut fonts,
        FontFamily::Monospace,
        load_font_stack_bytes(
            workspace_root,
            settings.editor_font_path.as_deref(),
            &editor_font_candidates_for_family_stack(&settings.font_family),
            MAX_EDITOR_FONT_FALLBACKS,
        ),
    );
    prepend_font_stack(
        &mut fonts,
        FontFamily::Proportional,
        load_font_stack_bytes(
            workspace_root,
            settings.ui_font_path.as_deref(),
            &ui_font_candidates(),
            MAX_UI_FONT_FALLBACKS,
        ),
    );
    ctx.set_fonts(fonts);
}

fn prepend_font_stack(
    fonts: &mut egui::FontDefinitions,
    family: FontFamily,
    loaded_fonts: Vec<(String, Vec<u8>)>,
) {
    let font_data = &mut fonts.font_data;
    let family_fonts = fonts.families.entry(family).or_default();
    for (index, (name, bytes)) in loaded_fonts.into_iter().enumerate() {
        font_data.insert(name.clone(), Arc::new(egui::FontData::from_owned(bytes)));
        family_fonts.insert(index, name);
    }
}

pub(crate) fn apply_typography(ctx: &Context, settings: &EditorSettings) {
    let ui_size = normalized_ui_font_size(settings.ui_font_size);
    let editor_size = normalized_editor_font_size(settings.font_size);
    ctx.style_mut(|style| {
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(ui_size + 5.0, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(ui_size, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(ui_size, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new((ui_size - 1.0).max(10.0), FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(editor_size, FontFamily::Monospace),
        );
    });
}

fn normalized_editor_font_size(size: f32) -> f32 {
    normalized_font_size(
        size,
        MIN_EDITOR_FONT_SIZE,
        MAX_EDITOR_FONT_SIZE,
        DEFAULT_EDITOR_FONT_SIZE,
    )
}

fn normalized_ui_font_size(size: f32) -> f32 {
    normalized_font_size(
        size,
        MIN_UI_FONT_SIZE,
        MAX_UI_FONT_SIZE,
        DEFAULT_UI_FONT_SIZE,
    )
}

fn normalized_font_size(size: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if size.is_finite() {
        size.clamp(min, max)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_typography, prepend_font_stack};
    use egui::{FontDefinitions, FontFamily, TextStyle};
    use kuroya_core::EditorSettings;

    #[test]
    fn prepend_font_stack_preserves_fallback_order_before_defaults() {
        let mut fonts = FontDefinitions::default();
        let original = fonts
            .families
            .get(&FontFamily::Monospace)
            .cloned()
            .unwrap_or_default();

        prepend_font_stack(
            &mut fonts,
            FontFamily::Monospace,
            vec![
                ("configured".to_owned(), Vec::new()),
                ("fallback".to_owned(), Vec::new()),
            ],
        );

        let family = fonts.families.get(&FontFamily::Monospace).unwrap();
        assert_eq!(family[0], "configured");
        assert_eq!(family[1], "fallback");
        assert_eq!(&family[2..], original.as_slice());
        assert!(fonts.font_data.contains_key("configured"));
        assert!(fonts.font_data.contains_key("fallback"));
    }

    #[test]
    fn apply_typography_replaces_non_finite_font_sizes() {
        let ctx = egui::Context::default();
        let settings = EditorSettings {
            font_size: f32::INFINITY,
            ui_font_size: f32::NAN,
            ..EditorSettings::default()
        };

        apply_typography(&ctx, &settings);

        let style = ctx.style();
        assert_eq!(style.text_styles[&TextStyle::Body].size, 13.0);
        assert_eq!(style.text_styles[&TextStyle::Monospace].size, 13.0);
        assert!(
            style
                .text_styles
                .values()
                .all(|font_id| font_id.size.is_finite())
        );
    }
}
