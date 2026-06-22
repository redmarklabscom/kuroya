use crate::{
    editor_pane_rows::EditorRowContext, editor_text_geometry::visual_column_for_char_offset,
    theme::bracket_depth_color,
};
use eframe::egui::{self, Color32, Pos2, pos2, vec2};
use kuroya_core::EditorBracketPairGuideMode;
use kuroya_core::buffer::BracketColor;
use std::ops::Range;

pub(crate) fn paint_bracket_pair_guides(
    painter: &egui::Painter,
    line_idx: usize,
    line_text: &str,
    text_pos: Pos2,
    rect: egui::Rect,
    row: &EditorRowContext<'_>,
) {
    let vertical_guides = row.bracket_pair_guides;
    let horizontal_guides = row.bracket_pair_guides_horizontal;
    if (!vertical_guides.enabled() && !horizontal_guides.enabled())
        || !bracket_overlay_geometry_is_valid(
            text_pos.x,
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom(),
            row.char_width,
            row.row_height,
            row.gutter_width,
        )
    {
        return;
    }

    for guide in row.bracket_pair_guide_ranges {
        let active = guide_is_active(
            guide.open_idx,
            guide.close_idx,
            row.active_bracket_pair_matches,
        );
        if !guide_mode_shows_any(vertical_guides, horizontal_guides, active) {
            continue;
        }

        let open_pos = row.buffer.char_position(guide.open_idx);
        let close_pos = row.buffer.char_position(guide.close_idx);
        let draw_vertical = guide_mode_shows(vertical_guides, active)
            && guide_visible_on_line(open_pos.line, close_pos.line, line_idx);
        let draw_horizontal = guide_mode_shows(horizontal_guides, active)
            && (line_idx == open_pos.line || line_idx == close_pos.line);
        if !draw_vertical && !draw_horizontal {
            continue;
        }

        let open_x = bracket_guide_x(
            row,
            text_pos,
            line_idx,
            line_text,
            open_pos.line,
            open_pos.column,
        );
        let stroke = bracket_pair_guide_stroke(
            guide.depth,
            active,
            row.highlight_active_bracket_pair,
            row.weak_text_color,
        );

        if draw_vertical {
            let top = if line_idx == open_pos.line {
                rect.top() + row.row_height * 0.58
            } else {
                rect.top() + 2.0
            };
            let bottom = if line_idx == close_pos.line {
                rect.top() + row.row_height * 0.42
            } else {
                rect.bottom() - 2.0
            };
            if bottom > top {
                painter.line_segment([pos2(open_x, top), pos2(open_x, bottom)], stroke);
            }
        }

        if draw_horizontal {
            let close_x = bracket_guide_x(
                row,
                text_pos,
                line_idx,
                line_text,
                close_pos.line,
                close_pos.column,
            );
            if open_pos.line == close_pos.line && line_idx == open_pos.line {
                paint_horizontal_bracket_pair_guide(painter, rect, row, open_x, close_x, stroke);
            } else {
                if line_idx == open_pos.line {
                    paint_horizontal_bracket_pair_guide(
                        painter,
                        rect,
                        row,
                        open_x,
                        open_x + row.char_width,
                        stroke,
                    );
                }
                if line_idx == close_pos.line {
                    paint_horizontal_bracket_pair_guide(
                        painter, rect, row, open_x, close_x, stroke,
                    );
                }
            }
        }
    }
}

pub(crate) fn paint_bracket_depth_markers(
    painter: &egui::Painter,
    snapshot_range: &Range<usize>,
    line_text: &str,
    text_pos: Pos2,
    rect: egui::Rect,
    bracket_colors: &[BracketColor],
    row: &EditorRowContext<'_>,
) {
    if !bracket_overlay_geometry_is_valid(
        text_pos.x,
        rect.left(),
        rect.top(),
        rect.right(),
        rect.bottom(),
        row.char_width,
        row.row_height,
        row.gutter_width,
    ) {
        return;
    }

    let start = bracket_colors.partition_point(|color| color.char_idx < snapshot_range.start);
    let end = start
        + bracket_colors[start..].partition_point(|color| color.char_idx < snapshot_range.end);
    for color in &bracket_colors[start..end] {
        let char_offset = color.char_idx.saturating_sub(snapshot_range.start);
        let col = visual_column_for_char_offset(line_text, char_offset, row.tab_width);
        let x = text_pos.x + col as f32 * row.char_width;
        let y = rect.top() + row.row_height - 4.0;
        painter.line_segment(
            [pos2(x, y), pos2(x + row.char_width.max(4.0), y)],
            egui::Stroke::new(1.4, bracket_depth_color(color.depth)),
        );
    }
}

pub(crate) fn paint_bracket_match_boxes(
    painter: &egui::Painter,
    snapshot_range: &Range<usize>,
    line_text: &str,
    text_pos: Pos2,
    rect: egui::Rect,
    row: &EditorRowContext<'_>,
) {
    if !bracket_overlay_geometry_is_valid(
        text_pos.x,
        rect.left(),
        rect.top(),
        rect.right(),
        rect.bottom(),
        row.char_width,
        row.row_height,
        row.gutter_width,
    ) {
        return;
    }

    for (a, b) in row.bracket_matches {
        for bracket in [a, b] {
            if snapshot_range.contains(bracket) {
                let char_offset = bracket.saturating_sub(snapshot_range.start);
                let col = visual_column_for_char_offset(line_text, char_offset, row.tab_width);
                painter.rect_stroke(
                    egui::Rect::from_min_size(
                        pos2(text_pos.x + (col as f32 * row.char_width), rect.top() + 2.0),
                        vec2(row.char_width, row.row_height - 3.0),
                    ),
                    2.0,
                    egui::Stroke::new(1.0, Color32::from_rgb(231, 185, 87)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
}

fn bracket_guide_x(
    row: &EditorRowContext<'_>,
    text_pos: Pos2,
    current_line: usize,
    current_line_text: &str,
    line: usize,
    column: usize,
) -> f32 {
    if line == current_line {
        let visual_col = visual_column_for_char_offset(current_line_text, column, row.tab_width);
        return text_pos.x + visual_col as f32 * row.char_width + row.char_width * 0.5;
    }

    let line_text = row.buffer.line(line).unwrap_or_default();
    let line_text = line_text.trim_end_matches(['\r', '\n']);
    let visual_col = visual_column_for_char_offset(line_text, column, row.tab_width);
    text_pos.x + visual_col as f32 * row.char_width + row.char_width * 0.5
}

fn paint_horizontal_bracket_pair_guide(
    painter: &egui::Painter,
    rect: egui::Rect,
    row: &EditorRowContext<'_>,
    start_x: f32,
    end_x: f32,
    stroke: egui::Stroke,
) {
    let left = start_x.min(end_x).max(rect.left() + row.gutter_width);
    let right = start_x
        .max(end_x)
        .max(left + row.char_width.min(8.0))
        .min(rect.right());
    if right <= left {
        return;
    }
    let y = rect.top() + row.row_height * 0.5;
    painter.line_segment([pos2(left, y), pos2(right, y)], stroke);
}

fn bracket_pair_guide_stroke(
    depth: usize,
    active: bool,
    highlight_active: bool,
    inactive_color: Color32,
) -> egui::Stroke {
    if active && highlight_active {
        egui::Stroke::new(1.5, bracket_depth_color(depth))
    } else {
        egui::Stroke::new(1.0, inactive_color)
    }
}

pub(crate) fn guide_mode_shows(mode: EditorBracketPairGuideMode, active: bool) -> bool {
    mode.enabled() && (!mode.active_only() || active)
}

fn guide_mode_shows_any(
    vertical: EditorBracketPairGuideMode,
    horizontal: EditorBracketPairGuideMode,
    active: bool,
) -> bool {
    guide_mode_shows(vertical, active) || guide_mode_shows(horizontal, active)
}

pub(crate) fn guide_visible_on_line(open_line: usize, close_line: usize, line_idx: usize) -> bool {
    let start = open_line.min(close_line);
    let end = open_line.max(close_line);
    start <= line_idx && line_idx <= end
}

pub(crate) fn guide_is_active(
    open_idx: usize,
    close_idx: usize,
    matches: &[(usize, usize)],
) -> bool {
    matches
        .iter()
        .any(|(left, right)| open_idx == (*left).min(*right) && close_idx == (*left).max(*right))
}

fn bracket_overlay_geometry_is_valid(
    text_x: f32,
    rect_left: f32,
    rect_top: f32,
    rect_right: f32,
    rect_bottom: f32,
    char_width: f32,
    row_height: f32,
    gutter_width: f32,
) -> bool {
    text_x.is_finite()
        && rect_left.is_finite()
        && rect_top.is_finite()
        && rect_right.is_finite()
        && rect_bottom.is_finite()
        && rect_right >= rect_left
        && rect_bottom >= rect_top
        && char_width.is_finite()
        && char_width > 0.0
        && row_height.is_finite()
        && row_height > 0.0
        && gutter_width.is_finite()
        && gutter_width >= 0.0
}

#[cfg(test)]
mod tests {
    use super::{
        bracket_overlay_geometry_is_valid, guide_is_active, guide_mode_shows, guide_mode_shows_any,
        guide_visible_on_line,
    };
    use kuroya_core::EditorBracketPairGuideMode;

    #[test]
    fn bracket_pair_guide_modes_follow_active_state() {
        assert!(!guide_mode_shows(EditorBracketPairGuideMode::Off, true));
        assert!(guide_mode_shows(EditorBracketPairGuideMode::On, false));
        assert!(!guide_mode_shows(EditorBracketPairGuideMode::Active, false));
        assert!(guide_mode_shows(EditorBracketPairGuideMode::Active, true));
        assert!(!guide_mode_shows_any(
            EditorBracketPairGuideMode::Active,
            EditorBracketPairGuideMode::Active,
            false,
        ));
        assert!(guide_mode_shows_any(
            EditorBracketPairGuideMode::Active,
            EditorBracketPairGuideMode::Off,
            true,
        ));
    }

    #[test]
    fn bracket_pair_guides_are_visible_between_pair_lines() {
        assert!(guide_visible_on_line(2, 5, 2));
        assert!(guide_visible_on_line(2, 5, 4));
        assert!(guide_visible_on_line(2, 5, 5));
        assert!(!guide_visible_on_line(2, 5, 1));
        assert!(!guide_visible_on_line(2, 5, 6));
    }

    #[test]
    fn bracket_pair_guide_active_match_accepts_reversed_pairs() {
        assert!(guide_is_active(4, 12, &[(12, 4)]));
        assert!(guide_is_active(4, 12, &[(4, 12)]));
        assert!(!guide_is_active(4, 12, &[(5, 12)]));
    }

    #[test]
    fn bracket_overlay_geometry_rejects_non_finite_or_collapsed_metrics() {
        assert!(bracket_overlay_geometry_is_valid(
            10.0, 0.0, 0.0, 120.0, 24.0, 8.0, 18.0, 48.0
        ));
        assert!(!bracket_overlay_geometry_is_valid(
            f32::NAN,
            0.0,
            0.0,
            120.0,
            24.0,
            8.0,
            18.0,
            48.0
        ));
        assert!(!bracket_overlay_geometry_is_valid(
            10.0, 0.0, 0.0, 120.0, 24.0, 0.0, 18.0, 48.0
        ));
        assert!(!bracket_overlay_geometry_is_valid(
            10.0,
            0.0,
            0.0,
            120.0,
            24.0,
            8.0,
            f32::INFINITY,
            48.0
        ));
        assert!(!bracket_overlay_geometry_is_valid(
            10.0, 120.0, 0.0, 0.0, 24.0, 8.0, 18.0, 48.0
        ));
        assert!(!bracket_overlay_geometry_is_valid(
            10.0, 0.0, 24.0, 120.0, 0.0, 8.0, 18.0, 48.0
        ));
        assert!(!bracket_overlay_geometry_is_valid(
            10.0, 0.0, 0.0, 120.0, 24.0, 8.0, 18.0, -1.0
        ));
    }
}
