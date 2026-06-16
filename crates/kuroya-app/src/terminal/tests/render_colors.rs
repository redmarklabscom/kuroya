use super::*;

#[test]
fn terminal_bold_ansi_colors_can_use_bright_variant() {
    let palette = terminal_ansi_palette_from_colors(
        egui::Color32::from_rgb(18, 20, 24),
        egui::Color32::from_rgb(222, 226, 233),
        egui::Color32::from_rgb(126, 136, 150),
        egui::Color32::from_rgb(91, 141, 239),
        egui::Color32::from_rgb(231, 185, 87),
        egui::Color32::from_rgb(197, 15, 31),
    );
    let red = terminal_foreground_color(vt100::Color::Idx(1), egui::Color32::WHITE, &palette);
    let bright_red =
        terminal_foreground_color(vt100::Color::Idx(9), egui::Color32::WHITE, &palette);

    assert_eq!(
        terminal_bold_foreground_color(vt100::Color::Idx(1), red, true, &palette),
        bright_red
    );

    assert_eq!(
        terminal_bold_foreground_color(vt100::Color::Idx(1), red, false, &palette),
        red
    );
}

#[test]
fn terminal_dim_ansi_text_color_blends_after_bold_resolution() {
    let background = egui::Color32::from_rgb(18, 20, 24);
    let text = egui::Color32::from_rgb(222, 226, 233);
    let palette = terminal_ansi_palette_from_colors(
        background,
        text,
        egui::Color32::from_rgb(126, 136, 150),
        egui::Color32::from_rgb(91, 141, 239),
        egui::Color32::from_rgb(231, 185, 87),
        egui::Color32::from_rgb(197, 15, 31),
    );
    let red = terminal_foreground_color(vt100::Color::Idx(1), text, &palette);
    let bright_red = terminal_bold_foreground_color(vt100::Color::Idx(1), red, true, &palette);

    let rendered = terminal_rendered_text_color(
        vt100::Color::Idx(1),
        red,
        background,
        true,
        true,
        true,
        1.0,
        &palette,
    );

    assert_ne!(rendered, bright_red);
    assert!(rendered.r() < bright_red.r());
    assert!(rendered.g() > red.g());
    assert!(rendered.b() > red.b());
}

#[test]
fn terminal_dim_ansi_text_color_runs_before_minimum_contrast_adjustment() {
    let background = egui::Color32::BLACK;
    let text = egui::Color32::from_rgb(120, 120, 120);
    let palette = terminal_ansi_palette_from_colors(
        background,
        text,
        egui::Color32::from_rgb(80, 80, 80),
        egui::Color32::from_rgb(91, 141, 239),
        egui::Color32::from_rgb(231, 185, 87),
        egui::Color32::from_rgb(197, 15, 31),
    );

    let dim_without_contrast = terminal_rendered_text_color(
        vt100::Color::Default,
        text,
        background,
        false,
        true,
        true,
        1.0,
        &palette,
    );
    let adjusted = terminal_rendered_text_color(
        vt100::Color::Default,
        text,
        background,
        false,
        true,
        true,
        4.5,
        &palette,
    );

    assert!(dim_without_contrast.r() < text.r());
    assert!(adjusted.r() > dim_without_contrast.r());
}

#[test]
fn terminal_ansi_palette_uses_theme_colors_for_basic_roles() {
    let background = egui::Color32::from_rgb(18, 20, 24);
    let text = egui::Color32::from_rgb(222, 226, 233);
    let muted = egui::Color32::from_rgb(126, 136, 150);
    let accent = egui::Color32::from_rgb(91, 141, 239);
    let warning = egui::Color32::from_rgb(231, 185, 87);
    let error = egui::Color32::from_rgb(232, 98, 98);
    let palette =
        terminal_ansi_palette_from_colors(background, text, muted, accent, warning, error);

    assert_eq!(
        terminal_foreground_color(vt100::Color::Idx(1), text, &palette),
        error
    );
    assert_eq!(
        terminal_foreground_color(vt100::Color::Idx(3), text, &palette),
        warning
    );
    assert_eq!(
        terminal_foreground_color(vt100::Color::Idx(4), text, &palette),
        accent
    );
    assert_eq!(
        terminal_foreground_color(vt100::Color::Idx(8), text, &palette),
        muted
    );
    assert_eq!(
        terminal_foreground_color(vt100::Color::Rgb(1, 2, 3), text, &palette),
        egui::Color32::from_rgb(1, 2, 3)
    );
}

#[test]
fn terminal_minimum_contrast_adjusts_low_contrast_text() {
    let background = egui::Color32::from_rgb(12, 12, 12);
    let foreground = egui::Color32::from_rgb(20, 20, 20);

    let adjusted = terminal_contrast_color(foreground, background, 4.5);

    assert_ne!(adjusted, foreground);
    assert_eq!(
        terminal_contrast_color(foreground, background, 1.0),
        foreground
    );
}
