use egui::Color32;
use kuroya_core::DiagnosticSeverity;

pub(crate) fn rgb(color: [u8; 3]) -> Color32 {
    Color32::from_rgb(color[0], color[1], color[2])
}

pub(super) fn contrast_ratio(first: Color32, second: Color32) -> f32 {
    let bright = relative_luminance(first).max(relative_luminance(second));
    let dark = relative_luminance(first).min(relative_luminance(second));
    (bright + 0.05) / (dark + 0.05)
}

pub(super) fn color_distance(first: Color32, second: Color32) -> f32 {
    let red = first.r() as f32 - second.r() as f32;
    let green = first.g() as f32 - second.g() as f32;
    let blue = first.b() as f32 - second.b() as f32;
    (red * red + green * green + blue * blue).sqrt()
}

pub(super) fn higher_contrast_color(
    background: Color32,
    first: Color32,
    second: Color32,
) -> Color32 {
    if contrast_ratio(first, background) >= contrast_ratio(second, background) {
        first
    } else {
        second
    }
}

pub(super) fn readable_color_or(
    preferred: Color32,
    background: Color32,
    fallback: Color32,
    min_contrast: f32,
) -> Color32 {
    let background = opaque_color_or(background, Color32::BLACK);
    let fallback = opaque_color_or(
        fallback,
        higher_contrast_color(background, Color32::WHITE, Color32::BLACK),
    );
    let preferred = opaque_color_or(preferred, fallback);
    let min_contrast = if min_contrast.is_finite() {
        min_contrast.max(0.0)
    } else {
        0.0
    };

    if contrast_ratio(preferred, background) >= min_contrast {
        preferred
    } else {
        fallback
    }
}

pub(super) fn blend_color(base: Color32, overlay: Color32, amount: f32) -> Color32 {
    let amount = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let base = opaque_color_or(base, Color32::BLACK);
    let overlay = opaque_color_or(overlay, base);
    let mix = |a: u8, b: u8| a as f32 + ((b as f32 - a as f32) * amount);
    Color32::from_rgb(
        mix(base.r(), overlay.r()).round() as u8,
        mix(base.g(), overlay.g()).round() as u8,
        mix(base.b(), overlay.b()).round() as u8,
    )
}

fn opaque_color_or(color: Color32, fallback: Color32) -> Color32 {
    let color = if color.a() == 0 { fallback } else { color };
    Color32::from_rgb(color.r(), color.g(), color.b())
}

pub(crate) fn bracket_depth_color(depth: usize) -> Color32 {
    const COLORS: [Color32; 4] = [
        Color32::from_rgb(91, 141, 239),
        Color32::from_rgb(231, 185, 87),
        Color32::from_rgb(116, 199, 154),
        Color32::from_rgb(201, 133, 232),
    ];
    COLORS[depth % COLORS.len()]
}

pub(crate) fn diagnostic_color(severity: DiagnosticSeverity) -> Color32 {
    match severity {
        DiagnosticSeverity::Error => Color32::from_rgb(232, 98, 98),
        DiagnosticSeverity::Warning => Color32::from_rgb(231, 185, 87),
        DiagnosticSeverity::Info => Color32::from_rgb(91, 141, 239),
        DiagnosticSeverity::Hint => Color32::from_rgb(126, 136, 150),
    }
}

pub(crate) fn document_highlight_color(kind: Option<u8>) -> Color32 {
    match kind {
        Some(3) => Color32::from_rgb(82, 62, 49),
        Some(2) => Color32::from_rgb(48, 61, 54),
        _ => Color32::from_rgb(48, 56, 72),
    }
}

pub(crate) fn semantic_token_color(token_type: &str, modifiers: &[String]) -> Color32 {
    let modifiers = SemanticTokenModifierFlags::from_modifiers(modifiers);
    if modifiers.deprecated {
        return Color32::from_rgba_unmultiplied(126, 136, 150, 34);
    }
    if modifiers.readonly {
        return Color32::from_rgba_unmultiplied(91, 141, 239, 44);
    }
    if modifiers.static_modifier {
        return Color32::from_rgba_unmultiplied(201, 133, 232, 38);
    }
    if modifiers.declaration_or_definition {
        return Color32::from_rgba_unmultiplied(231, 185, 87, 34);
    }

    semantic_token_type_color(token_type)
}

#[derive(Default)]
struct SemanticTokenModifierFlags {
    deprecated: bool,
    readonly: bool,
    static_modifier: bool,
    declaration_or_definition: bool,
}

impl SemanticTokenModifierFlags {
    fn from_modifiers(modifiers: &[String]) -> Self {
        let mut flags = Self::default();
        for modifier in modifiers {
            match modifier.as_str() {
                "deprecated" => flags.deprecated = true,
                "readonly" => flags.readonly = true,
                "static" => flags.static_modifier = true,
                "declaration" | "definition" => flags.declaration_or_definition = true,
                _ => {}
            }
        }
        flags
    }
}

fn semantic_token_type_color(token_type: &str) -> Color32 {
    match token_type {
        "function" | "method" | "macro" => Color32::from_rgba_unmultiplied(75, 137, 220, 42),
        "class" | "enum" | "interface" | "struct" | "type" | "typeParameter" => {
            Color32::from_rgba_unmultiplied(174, 124, 215, 42)
        }
        "parameter" | "property" | "variable" | "enumMember" => {
            Color32::from_rgba_unmultiplied(76, 176, 135, 36)
        }
        "keyword" | "modifier" | "operator" => Color32::from_rgba_unmultiplied(214, 169, 83, 34),
        "string" | "number" | "regexp" => Color32::from_rgba_unmultiplied(196, 141, 82, 32),
        "comment" => Color32::from_rgba_unmultiplied(116, 127, 142, 30),
        _ => Color32::from_rgba_unmultiplied(120, 146, 185, 28),
    }
}

fn relative_luminance(color: Color32) -> f32 {
    let red = srgb_channel_to_linear(color.r());
    let green = srgb_channel_to_linear(color.g());
    let blue = srgb_channel_to_linear(color.b());
    (0.2126 * red) + (0.7152 * green) + (0.0722 * blue)
}

fn srgb_channel_to_linear(value: u8) -> f32 {
    let channel = value as f32 / 255.0;
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        blend_color, color_distance, contrast_ratio, readable_color_or, semantic_token_color,
    };
    use egui::Color32;

    #[test]
    fn contrast_ratio_uses_wcag_luminance_range() {
        assert!((contrast_ratio(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
        assert_eq!(contrast_ratio(Color32::BLACK, Color32::BLACK), 1.0);
    }

    #[test]
    fn blend_color_clamps_blend_amount() {
        let base = Color32::from_rgb(10, 20, 30);
        let overlay = Color32::from_rgb(110, 120, 130);

        assert_eq!(blend_color(base, overlay, f32::NAN), base);
        assert_eq!(blend_color(base, overlay, f32::INFINITY), base);
        assert_eq!(blend_color(base, overlay, -1.0), base);
        assert_eq!(blend_color(base, overlay, 2.0), overlay);
        assert_eq!(
            blend_color(base, overlay, 0.5),
            Color32::from_rgb(60, 70, 80)
        );
    }

    #[test]
    fn blend_color_uses_opaque_fallbacks_for_transparent_colors() {
        let base = Color32::from_rgb(10, 20, 30);

        assert_eq!(blend_color(base, Color32::TRANSPARENT, 0.8), base);
        assert_eq!(
            blend_color(Color32::TRANSPARENT, Color32::from_rgb(100, 120, 140), 0.5),
            Color32::from_rgb(50, 60, 70)
        );
    }

    #[test]
    fn readable_color_falls_back_from_transparent_colors() {
        let background = Color32::from_rgb(16, 16, 16);
        let fallback = Color32::from_rgb(240, 240, 240);

        assert_eq!(
            readable_color_or(Color32::TRANSPARENT, background, fallback, 4.5),
            fallback
        );
        assert_eq!(
            readable_color_or(
                Color32::TRANSPARENT,
                Color32::TRANSPARENT,
                fallback,
                f32::NAN
            ),
            fallback
        );
    }

    #[test]
    fn color_distance_is_zero_for_identical_colors() {
        assert_eq!(
            color_distance(Color32::from_rgb(12, 34, 56), Color32::from_rgb(12, 34, 56)),
            0.0
        );
    }

    #[test]
    fn semantic_token_color_uses_modifiers_before_token_type() {
        assert_eq!(
            semantic_token_color("function", &["deprecated".to_owned()]),
            Color32::from_rgba_unmultiplied(126, 136, 150, 34)
        );
        assert_eq!(
            semantic_token_color("variable", &["readonly".to_owned()]),
            Color32::from_rgba_unmultiplied(91, 141, 239, 44)
        );
        assert_eq!(
            semantic_token_color("class", &["static".to_owned()]),
            Color32::from_rgba_unmultiplied(201, 133, 232, 38)
        );
        assert_eq!(
            semantic_token_color("method", &["definition".to_owned()]),
            Color32::from_rgba_unmultiplied(231, 185, 87, 34)
        );
    }
}
