use super::{
    DIFF_PATCH_OVERVIEW_MAX_SCAN_BYTES, DIFF_PATCH_OVERVIEW_MAX_SCAN_LINES,
    layout::{editor_rect_finite, one_based_buffer_line, saturated_f32_from_f64},
};
use eframe::egui::{self, Color32, Rect, pos2, vec2};
use kuroya_core::{GitLineChangeKind, TextBuffer, buffer::CursorPosition};
use std::collections::BTreeMap;

pub(crate) fn diff_patch_overview_lines(buffer: &TextBuffer) -> BTreeMap<usize, GitLineChangeKind> {
    if diff_patch_overview_scan_exceeds_budget(buffer) {
        return BTreeMap::new();
    }

    (0..buffer.len_lines())
        .filter_map(|line_idx| {
            let kind = diff_patch_overview_line_kind_for_buffer(buffer, line_idx)?;
            Some((line_idx + 1, kind))
        })
        .collect()
}

fn diff_patch_overview_scan_exceeds_budget(buffer: &TextBuffer) -> bool {
    buffer.len_bytes() > DIFF_PATCH_OVERVIEW_MAX_SCAN_BYTES
        || buffer.len_lines() > DIFF_PATCH_OVERVIEW_MAX_SCAN_LINES
}

fn diff_patch_overview_line_kind_for_buffer(
    buffer: &TextBuffer,
    line_idx: usize,
) -> Option<GitLineChangeKind> {
    if buffer.line_starts_with(line_idx, "+") && !buffer.line_starts_with(line_idx, "+++") {
        Some(GitLineChangeKind::Added)
    } else if buffer.line_starts_with(line_idx, "-") && !buffer.line_starts_with(line_idx, "---") {
        Some(GitLineChangeKind::Deleted)
    } else {
        None
    }
}

pub(super) fn paint_scm_diff_overview_ruler(
    ui: &egui::Ui,
    scroll_rect: Rect,
    line_count: usize,
    diff_lines: &BTreeMap<usize, GitLineChangeKind>,
    cursor_lines: &[usize],
    overview_ruler_border: bool,
    overview_ruler_lanes: usize,
) {
    let overview_ruler_lanes = overview_ruler_lanes.min(3);
    if !editor_rect_finite(scroll_rect)
        || scroll_rect.width() <= 0.0
        || scroll_rect.height() <= 0.0
        || overview_ruler_lanes == 0
    {
        return;
    }

    let painter = ui.painter();
    if overview_ruler_border {
        painter.rect_filled(
            overview_ruler_border_rect(scroll_rect, overview_ruler_lanes),
            0.0,
            Color32::from_rgb(47, 47, 47),
        );
    }
    if diff_lines.is_empty() && cursor_lines.is_empty() {
        return;
    }

    for (line_number, kind) in diff_lines {
        if *line_number == 0 {
            continue;
        }
        painter.rect_filled(
            scm_diff_overview_marker_rect(
                scroll_rect,
                line_count,
                *line_number,
                overview_ruler_lanes,
            ),
            0.0,
            scm_diff_overview_marker_color(*kind),
        );
    }
    for line_number in cursor_lines {
        if *line_number == 0 {
            continue;
        }
        painter.rect_filled(
            overview_ruler_cursor_marker_rect(
                scroll_rect,
                line_count,
                *line_number,
                overview_ruler_lanes,
            ),
            0.0,
            Color32::from_rgb(191, 191, 191),
        );
    }
}

pub(super) fn overview_ruler_border_rect(scroll_rect: Rect, overview_ruler_lanes: usize) -> Rect {
    if !editor_rect_finite(scroll_rect) {
        return Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0));
    }

    let lanes_width = overview_ruler_lanes.clamp(1, 3) as f32 * 3.0;
    Rect::from_min_max(
        pos2(scroll_rect.right() - lanes_width - 1.0, scroll_rect.top()),
        pos2(scroll_rect.right() - lanes_width, scroll_rect.bottom()),
    )
}

pub(super) fn overview_ruler_cursor_lines(
    cursor_positions: &[CursorPosition],
    hide_cursor_in_overview_ruler: bool,
    line_count: usize,
) -> Vec<usize> {
    if hide_cursor_in_overview_ruler || cursor_positions.is_empty() {
        return Vec::new();
    }

    let mut cursor_lines = Vec::with_capacity(cursor_positions.len());
    cursor_lines.extend(
        cursor_positions
            .iter()
            .filter_map(|cursor| one_based_buffer_line(cursor.line, line_count)),
    );
    cursor_lines.sort_unstable();
    cursor_lines.dedup();
    cursor_lines
}

pub(super) fn overview_ruler_cursor_marker_rect(
    scroll_rect: Rect,
    line_count: usize,
    line_number: usize,
    overview_ruler_lanes: usize,
) -> Rect {
    if !editor_rect_finite(scroll_rect) {
        return Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0));
    }

    let x = overview_ruler_lane_left(scroll_rect, overview_ruler_lanes, 1);
    let y = overview_ruler_line_y(scroll_rect, line_count, line_number);
    Rect::from_min_size(
        pos2(x + 0.5, (y - 1.0).max(scroll_rect.top())),
        vec2(2.0, 2.0),
    )
}

pub(super) fn scm_diff_overview_marker_rect(
    scroll_rect: Rect,
    line_count: usize,
    line_number: usize,
    overview_ruler_lanes: usize,
) -> Rect {
    if !editor_rect_finite(scroll_rect) {
        return Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0));
    }

    let x = overview_ruler_lane_left(scroll_rect, overview_ruler_lanes, 0);
    let y = overview_ruler_line_y(scroll_rect, line_count, line_number);
    Rect::from_min_size(pos2(x, (y - 1.0).max(scroll_rect.top())), vec2(3.0, 2.0))
}

fn overview_ruler_line_y(scroll_rect: Rect, line_count: usize, line_number: usize) -> f32 {
    let line_count = line_count.max(1);
    let line_idx = line_number
        .saturating_sub(1)
        .min(line_count.saturating_sub(1));
    let ratio = if line_count <= 1 {
        0.0
    } else {
        line_idx as f64 / line_count.saturating_sub(1) as f64
    };
    saturated_f32_from_f64(scroll_rect.top() as f64 + ratio * scroll_rect.height() as f64)
}

fn overview_ruler_lane_left(
    scroll_rect: Rect,
    overview_ruler_lanes: usize,
    preferred_lane_from_right: usize,
) -> f32 {
    let lanes = overview_ruler_lanes.clamp(1, 3);
    let lane = preferred_lane_from_right.min(lanes - 1);
    scroll_rect.right() - ((lane + 1) as f32 * 3.0)
}

fn scm_diff_overview_marker_color(kind: GitLineChangeKind) -> Color32 {
    match kind {
        GitLineChangeKind::Added => Color32::from_rgb(76, 175, 80),
        GitLineChangeKind::Modified => Color32::from_rgb(91, 141, 239),
        GitLineChangeKind::Deleted => Color32::from_rgb(232, 98, 98),
    }
}
