use eframe::egui::{self, Color32, FontFamily, FontId};
use kuroya_core::settings::clamp_editor_font_size;

pub(crate) fn measured_monospace_char_width(ui: &mut egui::Ui, font_size: f32) -> f32 {
    let font_size = safe_editor_font_size(font_size);
    let font_id = FontId::new(font_size, FontFamily::Monospace);
    ui.fonts_mut(|fonts| {
        let measured = fonts
            .layout_no_wrap("m".to_owned(), font_id, Color32::WHITE)
            .rect
            .width();
        if measured.is_finite() && measured > 0.0 {
            measured.max(font_size * 0.5)
        } else {
            font_size * 0.5
        }
    })
}

pub(crate) fn visual_column_for_char_offset(
    text: &str,
    char_offset: usize,
    tab_width: usize,
) -> usize {
    if char_offset == 0 {
        return 0;
    }
    if let Some(prefix_len) = plain_ascii_prefix_len_without_tabs(text, char_offset) {
        return prefix_len;
    }

    let tab_width = tab_width.max(1);
    let mut visual_column = 0usize;
    for (index, ch) in text.chars().enumerate() {
        if index >= char_offset {
            break;
        }
        visual_column =
            visual_column.saturating_add(visual_width_for_char(ch, visual_column, tab_width));
    }
    visual_column
}

pub(crate) fn visual_width(text: &str, tab_width: usize) -> usize {
    if let Some(len) = plain_ascii_len_without_tabs(text) {
        return len;
    }

    let tab_width = tab_width.max(1);
    let mut visual_column = 0usize;
    for ch in text.chars() {
        visual_column =
            visual_column.saturating_add(visual_width_for_char(ch, visual_column, tab_width));
    }
    visual_column
}

pub(crate) fn visual_x_for_char_idx(
    text_pos_x: f32,
    line_text: &str,
    char_idx: usize,
    snapshot_start: usize,
    tab_width: usize,
    char_width: f32,
) -> f32 {
    let text_pos_x = finite_or_zero(text_pos_x);
    if !char_width.is_finite() || char_width <= 0.0 {
        return text_pos_x;
    }

    let char_offset = char_idx.saturating_sub(snapshot_start);
    let visual_col = visual_column_for_char_offset(line_text, char_offset, tab_width);
    saturated_f32_from_f64(text_pos_x as f64 + visual_col as f64 * char_width as f64)
}

pub(crate) fn char_offset_for_visual_column(
    text: &str,
    visual_column: usize,
    tab_width: usize,
) -> usize {
    if visual_column == 0 {
        return 0;
    }
    if let Some(len) = plain_ascii_len_without_tabs(text) {
        return visual_column.min(len);
    }

    let tab_width = tab_width.max(1);
    let mut current_column = 0usize;
    let mut char_count = 0usize;
    for (index, ch) in text.chars().enumerate() {
        char_count = index + 1;
        let width = visual_width_for_char(ch, current_column, tab_width);
        if width == 0 {
            continue;
        }
        let midpoint = current_column.saturating_add(width / 2);
        if visual_column <= midpoint {
            return index;
        }
        if visual_column < current_column.saturating_add(width) {
            return index + 1;
        }
        current_column = current_column.saturating_add(width);
    }
    char_count
}

pub(crate) fn visual_width_for_char(ch: char, visual_column: usize, tab_width: usize) -> usize {
    if ch == '\t' {
        let tab_width = tab_width.max(1);
        let remainder = visual_column % tab_width;
        if remainder == 0 {
            tab_width
        } else {
            tab_width - remainder
        }
    } else if is_zero_width_codepoint(ch) {
        0
    } else {
        1
    }
}

fn plain_ascii_prefix_len_without_tabs(text: &str, char_offset: usize) -> Option<usize> {
    let prefix_len = char_offset.min(text.len());
    text.as_bytes()
        .get(..prefix_len)
        .filter(|prefix| prefix.iter().all(|byte| byte.is_ascii() && *byte != b'\t'))
        .map(|prefix| prefix.len())
}

fn plain_ascii_len_without_tabs(text: &str) -> Option<usize> {
    text.as_bytes()
        .iter()
        .all(|byte| byte.is_ascii() && *byte != b'\t')
        .then_some(text.len())
}

fn is_zero_width_codepoint(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{E0100}'..='\u{E01EF}'
            | '\u{200C}'
            | '\u{200D}'
            | '\u{2060}'
            | '\u{FEFF}'
    )
}

fn safe_editor_font_size(font_size: f32) -> f32 {
    clamp_editor_font_size(font_size, 13.0)
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn saturated_f32_from_f64(value: f64) -> f32 {
    if !value.is_finite() {
        if value.is_sign_negative() {
            -f32::MAX
        } else {
            f32::MAX
        }
    } else if value > f32::MAX as f64 {
        f32::MAX
    } else if value < -(f32::MAX as f64) {
        -f32::MAX
    } else {
        value as f32
    }
}

#[cfg(test)]
mod tests {
    use super::{
        char_offset_for_visual_column, safe_editor_font_size, visual_column_for_char_offset,
        visual_width, visual_width_for_char, visual_x_for_char_idx,
    };

    #[test]
    fn tabs_expand_to_the_next_tab_stop() {
        assert_eq!(visual_width_for_char('\t', 0, 4), 4);
        assert_eq!(visual_width_for_char('\t', 2, 4), 2);
        assert_eq!(visual_width("a\tb", 4), 5);
    }

    #[test]
    fn char_offsets_map_to_visual_columns() {
        assert_eq!(visual_column_for_char_offset("\tab", 0, 4), 0);
        assert_eq!(visual_column_for_char_offset("\tab", 1, 4), 4);
        assert_eq!(visual_column_for_char_offset("\tab", 3, 4), 6);
    }

    #[test]
    fn plain_ascii_geometry_uses_direct_columns() {
        let line = "let value = 123;";

        assert_eq!(visual_width(line, 4), line.len());
        assert_eq!(visual_column_for_char_offset(line, 3, 4), 3);
        assert_eq!(
            visual_column_for_char_offset(line, usize::MAX, 4),
            line.len()
        );
        assert_eq!(char_offset_for_visual_column(line, 5, 4), 5);
        assert_eq!(
            char_offset_for_visual_column(line, usize::MAX, 4),
            line.len()
        );
    }

    #[test]
    fn plain_ascii_prefix_fast_path_stops_before_complex_text() {
        assert_eq!(visual_column_for_char_offset("abc\tdef", 3, 4), 3);
        assert_eq!(visual_column_for_char_offset("abc\u{0301}def", 3, 4), 3);
        assert_eq!(visual_column_for_char_offset("abc\u{0301}def", 4, 4), 3);
    }

    #[test]
    fn visual_columns_map_back_to_nearest_char_offsets() {
        assert_eq!(char_offset_for_visual_column("\tab", 0, 4), 0);
        assert_eq!(char_offset_for_visual_column("\tab", 3, 4), 1);
        assert_eq!(char_offset_for_visual_column("\tab", 4, 4), 1);
        assert_eq!(char_offset_for_visual_column("\tab", 6, 4), 3);
    }

    #[test]
    fn zero_width_unicode_marks_do_not_advance_visual_columns() {
        let composed = "e\u{0301}x";

        assert_eq!(visual_width_for_char('\u{0301}', 1, 4), 0);
        assert_eq!(visual_width_for_char('\u{FE0F}', 1, 4), 0);
        assert_eq!(visual_width(composed, 4), 2);
        assert_eq!(visual_column_for_char_offset(composed, 1, 4), 1);
        assert_eq!(visual_column_for_char_offset(composed, 2, 4), 1);
        assert_eq!(char_offset_for_visual_column(composed, 1, 4), 2);
    }

    #[test]
    fn visual_x_for_char_idx_uses_snapshot_relative_visual_columns() {
        assert_eq!(visual_x_for_char_idx(10.0, "\tab", 2, 0, 4, 8.0), 50.0);
        assert_eq!(
            visual_x_for_char_idx(10.0, "e\u{0301}x", 2, 0, 4, 8.0),
            18.0
        );
        assert_eq!(visual_x_for_char_idx(10.0, "abcdef", 6, 3, 4, 8.0), 34.0);
        assert_eq!(visual_x_for_char_idx(10.0, "abcdef", 20, 0, 4, 8.0), 58.0);
    }

    #[test]
    fn visual_x_for_char_idx_rejects_non_finite_geometry() {
        assert_eq!(visual_x_for_char_idx(f32::NAN, "abc", 3, 0, 4, 8.0), 24.0);
        assert_eq!(visual_x_for_char_idx(10.0, "abc", 3, 0, 4, f32::NAN), 10.0);
        assert_eq!(visual_x_for_char_idx(10.0, "abc", 3, 0, 4, 0.0), 10.0);
    }

    #[test]
    fn visual_x_for_char_idx_saturates_extreme_columns() {
        assert_eq!(
            visual_x_for_char_idx(10.0, "\t\t", 2, 0, usize::MAX, f32::MAX),
            f32::MAX
        );
        assert_eq!(
            visual_x_for_char_idx(-f32::MAX, "", 0, 0, 4, 8.0),
            -f32::MAX
        );
    }

    #[test]
    fn editor_font_size_uses_finite_clamped_values() {
        use kuroya_core::settings::MAX_EDITOR_FONT_SIZE;

        assert_eq!(safe_editor_font_size(f32::NAN), 13.0);
        assert_eq!(safe_editor_font_size(0.0), 13.0);
        assert_eq!(safe_editor_font_size(f32::MAX), MAX_EDITOR_FONT_SIZE);
    }

    #[test]
    fn visual_columns_saturate_for_extreme_tab_widths() {
        assert_eq!(visual_width("\t\t", usize::MAX), usize::MAX);
        assert_eq!(
            visual_column_for_char_offset("\t\t", 2, usize::MAX),
            usize::MAX
        );
        assert_eq!(
            char_offset_for_visual_column("\t\t", usize::MAX, usize::MAX),
            1
        );
    }
}
