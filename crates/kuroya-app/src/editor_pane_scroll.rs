use crate::{
    transient_state::{EditorInertialScroll, EditorMiddleClickScroll},
    ui_state::{clamp_scroll_target, next_smooth_scroll_offset, smooth_scroll_finished},
    workspace_state::PaneId,
};
use eframe::egui;
use kuroya_core::{BufferId, EditorMouseMiddleClickAction};
use std::collections::HashMap;

pub(crate) type EditorScrollKey = (PaneId, BufferId);

const EDITOR_INERTIAL_SCROLL_STOP_SPEED: f32 = 8.0;
const EDITOR_INERTIAL_SCROLL_FRICTION: f32 = 5200.0;
const EDITOR_INERTIAL_SCROLL_MAX_DT: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq)]
struct EditorScrollGeometry {
    row_height: f32,
    visible_rows: usize,
    max_offset: f32,
}

impl EditorScrollGeometry {
    fn new(line_total: usize, row_height: f32, viewport_height: f32) -> Option<Self> {
        if !row_height.is_finite()
            || row_height <= 0.0
            || !viewport_height.is_finite()
            || viewport_height <= 0.0
        {
            return None;
        }

        let content_height = line_total as f32 * row_height;
        if !content_height.is_finite() {
            return None;
        }

        let visible_rows = (viewport_height / row_height).floor().max(1.0) as usize;
        let max_offset = (content_height - viewport_height).max(0.0);
        Some(Self {
            row_height,
            visible_rows,
            max_offset,
        })
    }

    fn clamp_offset(self, offset: f32) -> f32 {
        if offset.is_finite() {
            offset.clamp(0.0, self.max_offset)
        } else {
            0.0
        }
    }

    fn offset_correction(self, offset: f32) -> Option<f32> {
        let clamped = self.clamp_offset(offset);
        (offset != clamped).then_some(clamped)
    }

    fn cursor_scroll_target(
        self,
        visible_row: usize,
        current_offset: f32,
        margin_rows: usize,
    ) -> Option<f32> {
        let margin = margin_rows.min(self.visible_rows.saturating_sub(1) / 2);
        let first_visible = (self.clamp_offset(current_offset) / self.row_height).floor() as usize;
        let last_visible_exclusive = first_visible.saturating_add(self.visible_rows);
        let upper_margin_row = first_visible.saturating_add(margin);
        let lower_margin_row = last_visible_exclusive.saturating_sub(margin);

        let target_first_row = if visible_row < upper_margin_row {
            visible_row.saturating_sub(margin)
        } else if visible_row >= lower_margin_row {
            visible_row
                .saturating_add(margin)
                .saturating_add(1)
                .saturating_sub(self.visible_rows)
        } else {
            return None;
        };

        Some(self.clamp_offset(target_first_row as f32 * self.row_height))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct EditorInertialScrollOffsets {
    pub(crate) vertical: Option<f32>,
    pub(crate) horizontal: Option<f32>,
    pub(crate) consumed_wheel: bool,
    pub(crate) active: bool,
}

pub(crate) fn clear_editor_scroll_state_for_buffer(
    scroll_offsets: &mut HashMap<EditorScrollKey, f32>,
    scroll_targets: &mut HashMap<EditorScrollKey, f32>,
    buffer_id: BufferId,
) {
    scroll_offsets.retain(|(_, id), _| *id != buffer_id);
    scroll_targets.retain(|(_, id), _| *id != buffer_id);
}

pub(crate) fn clear_editor_inertial_scrolls_for_buffer(
    scrolls: &mut HashMap<EditorScrollKey, EditorInertialScroll>,
    buffer_id: BufferId,
) {
    scrolls.retain(|(_, id), _| *id != buffer_id);
}

pub(crate) fn clear_editor_horizontal_scroll_offsets_for_buffer(
    scroll_offsets: &mut HashMap<EditorScrollKey, f32>,
    buffer_id: BufferId,
) {
    scroll_offsets.retain(|(_, id), _| *id != buffer_id);
}

pub(crate) fn clear_editor_middle_click_scroll_for_buffer(
    scroll: &mut Option<EditorMiddleClickScroll>,
    buffer_id: BufferId,
) {
    if scroll
        .as_ref()
        .is_some_and(|state| state.buffer_id == buffer_id)
    {
        *scroll = None;
    }
}

pub(crate) fn clear_editor_scroll_state_for_pane(
    scroll_offsets: &mut HashMap<EditorScrollKey, f32>,
    scroll_targets: &mut HashMap<EditorScrollKey, f32>,
    pane_id: PaneId,
) {
    scroll_offsets.retain(|(pane, _), _| *pane != pane_id);
    scroll_targets.retain(|(pane, _), _| *pane != pane_id);
}

pub(crate) fn clear_editor_inertial_scrolls_for_pane(
    scrolls: &mut HashMap<EditorScrollKey, EditorInertialScroll>,
    pane_id: PaneId,
) {
    scrolls.retain(|(pane, _), _| *pane != pane_id);
}

pub(crate) fn clear_editor_horizontal_scroll_offsets_for_pane(
    scroll_offsets: &mut HashMap<EditorScrollKey, f32>,
    pane_id: PaneId,
) {
    scroll_offsets.retain(|(pane, _), _| *pane != pane_id);
}

pub(crate) fn clear_editor_middle_click_scroll_for_pane(
    scroll: &mut Option<EditorMiddleClickScroll>,
    pane_id: PaneId,
) {
    if scroll
        .as_ref()
        .is_some_and(|state| state.pane_id == pane_id)
    {
        *scroll = None;
    }
}

pub(crate) fn resolve_editor_scroll_offset(
    scroll_to_row: Option<usize>,
    line_total: usize,
    row_height: f32,
    viewport_height: f32,
    surrounding_lines: usize,
    smooth_scrolling: bool,
    scroll_key: EditorScrollKey,
    scroll_offsets: &HashMap<EditorScrollKey, f32>,
    scroll_targets: &mut HashMap<EditorScrollKey, f32>,
) -> Option<f32> {
    let Some(geometry) = EditorScrollGeometry::new(line_total, row_height, viewport_height) else {
        scroll_targets.remove(&scroll_key);
        return None;
    };

    let mut forced_scroll_offset = None;
    if let Some(visible_row) = scroll_to_row {
        let current = scroll_offsets
            .get(&scroll_key)
            .copied()
            .map(|offset| geometry.clamp_offset(offset))
            .unwrap_or_default();
        if let Some(target) = geometry.cursor_scroll_target(visible_row, current, surrounding_lines)
        {
            if smooth_scrolling {
                scroll_targets.insert(scroll_key, target);
            } else {
                scroll_targets.remove(&scroll_key);
                forced_scroll_offset = Some(target);
            }
        } else {
            scroll_targets.remove(&scroll_key);
        }
    }

    if forced_scroll_offset.is_none()
        && let Some(target) = scroll_targets.get(&scroll_key).copied()
    {
        if target.is_finite() {
            let target = geometry.clamp_offset(target);
            scroll_targets.insert(scroll_key, target);
            let current = scroll_offsets
                .get(&scroll_key)
                .copied()
                .map(|offset| geometry.clamp_offset(offset))
                .unwrap_or(target);
            forced_scroll_offset =
                Some(geometry.clamp_offset(next_smooth_scroll_offset(current, target, row_height)));
        } else {
            scroll_targets.remove(&scroll_key);
        }
    }

    if forced_scroll_offset.is_none()
        && let Some(current) = scroll_offsets.get(&scroll_key).copied()
    {
        forced_scroll_offset = geometry.offset_correction(current);
    }

    forced_scroll_offset
}

#[cfg(test)]
pub(crate) fn cursor_scroll_target(
    visible_row: usize,
    current_offset: f32,
    line_total: usize,
    row_height: f32,
    viewport_height: f32,
    surrounding_lines: usize,
) -> Option<f32> {
    EditorScrollGeometry::new(line_total, row_height, viewport_height)?.cursor_scroll_target(
        visible_row,
        current_offset,
        surrounding_lines,
    )
}

pub(crate) fn record_editor_scroll_offset(
    scroll_offsets: &mut HashMap<EditorScrollKey, f32>,
    scroll_targets: &mut HashMap<EditorScrollKey, f32>,
    scroll_key: EditorScrollKey,
    scroll_offset: f32,
    row_height: f32,
) {
    scroll_offsets.insert(scroll_key, scroll_offset);
    if let Some(target) = scroll_targets.get(&scroll_key).copied()
        && smooth_scroll_finished(scroll_offset, target, row_height)
    {
        scroll_targets.remove(&scroll_key);
    }
}

pub(crate) fn record_editor_horizontal_scroll_offset(
    scroll_offsets: &mut HashMap<EditorScrollKey, f32>,
    scroll_key: EditorScrollKey,
    scroll_offset: f32,
) {
    if scroll_offset.is_finite() && scroll_offset > 0.0 {
        scroll_offsets.insert(scroll_key, scroll_offset);
    } else {
        scroll_offsets.remove(&scroll_key);
    }
}

pub(crate) fn editor_inertial_scroll_offsets(
    scrolls: &mut HashMap<EditorScrollKey, EditorInertialScroll>,
    scroll_key: EditorScrollKey,
    enabled: bool,
    hovered: bool,
    current_vertical_offset: f32,
    current_horizontal_offset: f32,
    line_total: usize,
    row_height: f32,
    viewport_height: f32,
    content_width: f32,
    viewport_width: f32,
    wheel_delta: egui::Vec2,
    wheel_multiplier: egui::Vec2,
    dt: f32,
) -> EditorInertialScrollOffsets {
    if !enabled {
        scrolls.remove(&scroll_key);
        return EditorInertialScrollOffsets::default();
    }

    let dt = editor_inertial_scroll_dt(dt);
    let max_vertical_offset =
        max_editor_vertical_scroll_offset(line_total, row_height, viewport_height);
    let max_horizontal_offset = max_editor_horizontal_scroll_offset(content_width, viewport_width);
    let current = egui::vec2(
        finite_non_negative(current_horizontal_offset),
        finite_non_negative(current_vertical_offset),
    );
    let max = egui::vec2(max_horizontal_offset, max_vertical_offset);
    let wheel_offset_delta = if hovered && finite_vec2(wheel_delta) && finite_vec2(wheel_multiplier)
    {
        egui::vec2(
            -wheel_delta.x * wheel_multiplier.x,
            -wheel_delta.y * wheel_multiplier.y,
        )
    } else {
        egui::Vec2::ZERO
    };

    if wheel_offset_delta != egui::Vec2::ZERO {
        let next = clamp_editor_scroll_offset(current + wheel_offset_delta, max);
        let applied = next - current;
        if applied == egui::Vec2::ZERO {
            scrolls.remove(&scroll_key);
            return EditorInertialScrollOffsets {
                consumed_wheel: true,
                ..Default::default()
            };
        }

        let velocity = applied / dt;
        scrolls.insert(
            scroll_key,
            EditorInertialScroll {
                velocity_x: velocity.x,
                velocity_y: velocity.y,
            },
        );
        return EditorInertialScrollOffsets {
            vertical: Some(next.y),
            horizontal: Some(next.x),
            consumed_wheel: true,
            active: true,
        };
    }

    let Some(scroll) = scrolls.get(&scroll_key).copied() else {
        return EditorInertialScrollOffsets::default();
    };

    let velocity = egui::vec2(scroll.velocity_x, scroll.velocity_y);
    if !finite_vec2(velocity) || velocity == egui::Vec2::ZERO {
        scrolls.remove(&scroll_key);
        return EditorInertialScrollOffsets::default();
    }

    let next = clamp_editor_scroll_offset(current + velocity * dt, max);
    let applied = next - current;
    let mut next_velocity = decay_editor_inertial_velocity(velocity, dt);
    if applied.x == 0.0 {
        next_velocity.x = 0.0;
    }
    if applied.y == 0.0 {
        next_velocity.y = 0.0;
    }

    let active = next_velocity != egui::Vec2::ZERO;
    if active {
        scrolls.insert(
            scroll_key,
            EditorInertialScroll {
                velocity_x: next_velocity.x,
                velocity_y: next_velocity.y,
            },
        );
    } else {
        scrolls.remove(&scroll_key);
    }

    EditorInertialScrollOffsets {
        vertical: (applied.y != 0.0).then_some(next.y),
        horizontal: (applied.x != 0.0).then_some(next.x),
        consumed_wheel: false,
        active,
    }
}

pub(crate) fn editor_middle_click_scroll_enabled(
    scroll_on_middle_click: bool,
    mouse_middle_click_action: EditorMouseMiddleClickAction,
) -> bool {
    scroll_on_middle_click
        && matches!(
            mouse_middle_click_action,
            EditorMouseMiddleClickAction::Default
        )
}

fn max_editor_vertical_scroll_offset(
    line_total: usize,
    row_height: f32,
    viewport_height: f32,
) -> f32 {
    EditorScrollGeometry::new(line_total, row_height, viewport_height)
        .map(|geometry| geometry.max_offset)
        .unwrap_or_default()
}

fn max_editor_horizontal_scroll_offset(content_width: f32, viewport_width: f32) -> f32 {
    if !content_width.is_finite() || !viewport_width.is_finite() || viewport_width <= 0.0 {
        return 0.0;
    }

    (content_width - viewport_width).max(0.0)
}

fn clamp_editor_scroll_offset(offset: egui::Vec2, max: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        offset.x.clamp(0.0, max.x.max(0.0)),
        offset.y.clamp(0.0, max.y.max(0.0)),
    )
}

fn decay_editor_inertial_velocity(velocity: egui::Vec2, dt: f32) -> egui::Vec2 {
    egui::vec2(
        decay_editor_inertial_velocity_component(velocity.x, dt),
        decay_editor_inertial_velocity_component(velocity.y, dt),
    )
}

fn decay_editor_inertial_velocity_component(velocity: f32, dt: f32) -> f32 {
    if velocity.abs() <= EDITOR_INERTIAL_SCROLL_STOP_SPEED {
        return 0.0;
    }

    let friction = EDITOR_INERTIAL_SCROLL_FRICTION * dt;
    if friction >= velocity.abs() {
        0.0
    } else {
        velocity - friction * velocity.signum()
    }
}

fn editor_inertial_scroll_dt(dt: f32) -> f32 {
    if dt.is_finite() && dt > 0.0 {
        dt.min(EDITOR_INERTIAL_SCROLL_MAX_DT)
    } else {
        1.0 / 60.0
    }
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn finite_vec2(value: egui::Vec2) -> bool {
    value.x.is_finite() && value.y.is_finite()
}

pub(crate) fn editor_middle_click_scroll_offset(
    scroll: Option<&EditorMiddleClickScroll>,
    pane_id: PaneId,
    buffer_id: BufferId,
    pointer_y: f32,
    current_offset: f32,
    line_total: usize,
    row_height: f32,
    viewport_height: f32,
) -> Option<f32> {
    let scroll = scroll?;
    if scroll.pane_id != pane_id
        || scroll.buffer_id != buffer_id
        || !pointer_y.is_finite()
        || !current_offset.is_finite()
    {
        return None;
    }

    let dead_zone = row_height.max(1.0) * 0.75;
    let distance = pointer_y - scroll.anchor_y;
    let scroll_distance = (distance.abs() - dead_zone).max(0.0);
    if scroll_distance <= f32::EPSILON {
        return Some(clamp_scroll_target(
            current_offset,
            line_total,
            row_height,
            viewport_height,
        ));
    }

    let step = distance.signum() * scroll_distance * 0.24;
    Some(clamp_scroll_target(
        current_offset + step,
        line_total,
        row_height,
        viewport_height,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        clear_editor_horizontal_scroll_offsets_for_buffer,
        clear_editor_horizontal_scroll_offsets_for_pane, clear_editor_inertial_scrolls_for_buffer,
        clear_editor_inertial_scrolls_for_pane, clear_editor_middle_click_scroll_for_buffer,
        clear_editor_middle_click_scroll_for_pane, clear_editor_scroll_state_for_buffer,
        clear_editor_scroll_state_for_pane, cursor_scroll_target, editor_inertial_scroll_offsets,
        editor_middle_click_scroll_enabled, editor_middle_click_scroll_offset,
        record_editor_horizontal_scroll_offset, resolve_editor_scroll_offset,
    };
    use crate::transient_state::{EditorInertialScroll, EditorMiddleClickScroll};
    use eframe::egui;
    use kuroya_core::EditorMouseMiddleClickAction;
    use std::collections::HashMap;

    #[test]
    fn cursor_scroll_target_keeps_cursor_inside_surrounding_margin() {
        assert_eq!(cursor_scroll_target(15, 100.0, 100, 10.0, 100.0, 2), None);
        assert_eq!(
            cursor_scroll_target(11, 100.0, 100, 10.0, 100.0, 2),
            Some(90.0)
        );
        assert_eq!(
            cursor_scroll_target(18, 100.0, 100, 10.0, 100.0, 2),
            Some(110.0)
        );
    }

    #[test]
    fn cursor_scroll_target_scrolls_minimally_without_margin() {
        assert_eq!(cursor_scroll_target(12, 100.0, 100, 10.0, 100.0, 0), None);
        assert_eq!(
            cursor_scroll_target(9, 100.0, 100, 10.0, 100.0, 0),
            Some(90.0)
        );
        assert_eq!(
            cursor_scroll_target(20, 100.0, 100, 10.0, 100.0, 0),
            Some(110.0)
        );
    }

    #[test]
    fn cursor_scroll_target_clamps_large_margin_to_visible_rows() {
        assert_eq!(
            cursor_scroll_target(18, 100.0, 100, 10.0, 100.0, 50),
            Some(130.0)
        );
    }

    #[test]
    fn cursor_scroll_target_treats_stale_current_offset_as_clamped() {
        assert_eq!(cursor_scroll_target(19, 1000.0, 20, 10.0, 50.0, 0), None);
    }

    #[test]
    fn resolve_editor_scroll_offset_clamps_stale_smooth_target_to_viewport() {
        let key = (1, 7);
        let offsets = HashMap::from([(key, 240.0)]);
        let mut targets = HashMap::from([(key, 240.0)]);

        assert_eq!(
            resolve_editor_scroll_offset(
                None,
                10,
                10.0,
                50.0,
                0,
                true,
                key,
                &offsets,
                &mut targets,
            ),
            Some(50.0)
        );
        assert_eq!(targets.get(&key), Some(&50.0));
    }

    #[test]
    fn resolve_editor_scroll_offset_corrects_stale_recorded_offset_without_target() {
        let key = (1, 7);
        let offsets = HashMap::from([(key, 240.0)]);
        let mut targets = HashMap::new();

        assert_eq!(
            resolve_editor_scroll_offset(
                None,
                10,
                10.0,
                50.0,
                0,
                true,
                key,
                &offsets,
                &mut targets,
            ),
            Some(50.0)
        );
        assert!(targets.is_empty());
    }

    #[test]
    fn clear_editor_scroll_state_for_buffer_removes_offsets_and_targets() {
        let mut offsets = HashMap::from([((1, 7), 10.0), ((2, 7), 20.0), ((1, 8), 30.0)]);
        let mut targets = HashMap::from([((1, 7), 40.0), ((2, 7), 50.0), ((1, 8), 60.0)]);

        clear_editor_scroll_state_for_buffer(&mut offsets, &mut targets, 7);

        assert_eq!(offsets, HashMap::from([((1, 8), 30.0)]));
        assert_eq!(targets, HashMap::from([((1, 8), 60.0)]));
    }

    #[test]
    fn clear_editor_scroll_state_for_pane_removes_offsets_and_targets() {
        let mut offsets = HashMap::from([((1, 7), 10.0), ((2, 7), 20.0), ((1, 8), 30.0)]);
        let mut targets = HashMap::from([((1, 7), 40.0), ((2, 7), 50.0), ((1, 8), 60.0)]);

        clear_editor_scroll_state_for_pane(&mut offsets, &mut targets, 1);

        assert_eq!(offsets, HashMap::from([((2, 7), 20.0)]));
        assert_eq!(targets, HashMap::from([((2, 7), 50.0)]));
    }

    #[test]
    fn clear_editor_horizontal_scroll_offsets_remove_matching_scope() {
        let mut offsets = HashMap::from([((1, 7), 10.0), ((2, 7), 20.0), ((1, 8), 30.0)]);

        clear_editor_horizontal_scroll_offsets_for_buffer(&mut offsets, 7);

        assert_eq!(offsets, HashMap::from([((1, 8), 30.0)]));

        clear_editor_horizontal_scroll_offsets_for_pane(&mut offsets, 1);

        assert!(offsets.is_empty());
    }

    #[test]
    fn clear_editor_inertial_scrolls_remove_matching_scope() {
        let scroll = EditorInertialScroll {
            velocity_x: 10.0,
            velocity_y: 20.0,
        };
        let mut scrolls = HashMap::from([((1, 7), scroll), ((2, 7), scroll), ((1, 8), scroll)]);

        clear_editor_inertial_scrolls_for_buffer(&mut scrolls, 7);

        assert_eq!(scrolls, HashMap::from([((1, 8), scroll)]));

        clear_editor_inertial_scrolls_for_pane(&mut scrolls, 1);

        assert!(scrolls.is_empty());
    }

    #[test]
    fn record_editor_horizontal_scroll_offset_keeps_only_positive_finite_offsets() {
        let mut offsets = HashMap::new();

        record_editor_horizontal_scroll_offset(&mut offsets, (1, 7), 42.0);
        assert_eq!(offsets.get(&(1, 7)), Some(&42.0));

        record_editor_horizontal_scroll_offset(&mut offsets, (1, 7), 0.0);
        assert!(!offsets.contains_key(&(1, 7)));

        record_editor_horizontal_scroll_offset(&mut offsets, (1, 7), f32::NAN);
        assert!(!offsets.contains_key(&(1, 7)));
    }

    #[test]
    fn editor_inertial_scroll_offsets_start_from_wheel_delta_and_decay() {
        let mut scrolls = HashMap::new();
        let first = editor_inertial_scroll_offsets(
            &mut scrolls,
            (1, 7),
            true,
            true,
            0.0,
            0.0,
            100,
            20.0,
            100.0,
            500.0,
            200.0,
            egui::vec2(0.0, -30.0),
            egui::Vec2::splat(1.0),
            1.0 / 60.0,
        );

        assert_eq!(first.vertical, Some(30.0));
        assert_eq!(first.horizontal, Some(0.0));
        assert!(first.consumed_wheel);
        assert!(first.active);

        let second = editor_inertial_scroll_offsets(
            &mut scrolls,
            (1, 7),
            true,
            true,
            30.0,
            0.0,
            100,
            20.0,
            100.0,
            500.0,
            200.0,
            egui::Vec2::ZERO,
            egui::Vec2::splat(1.0),
            1.0 / 60.0,
        );

        assert!(second.vertical.is_some_and(|offset| offset > 30.0));
        assert!(!second.consumed_wheel);
        assert!(second.active);
    }

    #[test]
    fn editor_inertial_scroll_offsets_stop_at_content_edges() {
        let mut scrolls = HashMap::new();
        let result = editor_inertial_scroll_offsets(
            &mut scrolls,
            (1, 7),
            true,
            true,
            0.0,
            0.0,
            1,
            20.0,
            100.0,
            120.0,
            200.0,
            egui::vec2(0.0, -30.0),
            egui::Vec2::splat(1.0),
            1.0 / 60.0,
        );

        assert_eq!(result.vertical, None);
        assert_eq!(result.horizontal, None);
        assert!(result.consumed_wheel);
        assert!(!result.active);
        assert!(scrolls.is_empty());
    }

    #[test]
    fn editor_inertial_scroll_offsets_clear_when_disabled() {
        let mut scrolls = HashMap::from([(
            (1, 7),
            EditorInertialScroll {
                velocity_x: 10.0,
                velocity_y: 20.0,
            },
        )]);

        let result = editor_inertial_scroll_offsets(
            &mut scrolls,
            (1, 7),
            false,
            true,
            0.0,
            0.0,
            100,
            20.0,
            100.0,
            500.0,
            200.0,
            egui::vec2(0.0, -30.0),
            egui::Vec2::splat(1.0),
            1.0 / 60.0,
        );

        assert_eq!(result, Default::default());
        assert!(scrolls.is_empty());
    }

    #[test]
    fn middle_click_scroll_enables_only_for_default_middle_click_action() {
        assert!(editor_middle_click_scroll_enabled(
            true,
            EditorMouseMiddleClickAction::Default
        ));
        assert!(!editor_middle_click_scroll_enabled(
            false,
            EditorMouseMiddleClickAction::Default
        ));
        assert!(!editor_middle_click_scroll_enabled(
            true,
            EditorMouseMiddleClickAction::OpenLink
        ));
        assert!(!editor_middle_click_scroll_enabled(
            true,
            EditorMouseMiddleClickAction::CtrlLeftClick
        ));
    }

    #[test]
    fn middle_click_scroll_offset_tracks_pointer_distance_and_clamps() {
        let scroll = EditorMiddleClickScroll {
            pane_id: 2,
            buffer_id: 7,
            anchor_y: 100.0,
        };

        assert_eq!(
            editor_middle_click_scroll_offset(Some(&scroll), 2, 7, 105.0, 40.0, 100, 20.0, 100.0),
            Some(40.0)
        );
        let offset =
            editor_middle_click_scroll_offset(Some(&scroll), 2, 7, 160.0, 40.0, 100, 20.0, 100.0)
                .unwrap();
        assert!((offset - 50.8).abs() < 0.01);
        assert_eq!(
            editor_middle_click_scroll_offset(Some(&scroll), 2, 7, 20.0, 4.0, 100, 20.0, 100.0),
            Some(0.0)
        );
        assert_eq!(
            editor_middle_click_scroll_offset(Some(&scroll), 3, 7, 160.0, 40.0, 100, 20.0, 100.0),
            None
        );
    }

    #[test]
    fn middle_click_scroll_state_clears_by_buffer_or_pane() {
        let mut scroll = Some(EditorMiddleClickScroll {
            pane_id: 2,
            buffer_id: 7,
            anchor_y: 100.0,
        });

        clear_editor_middle_click_scroll_for_buffer(&mut scroll, 8);
        assert!(scroll.is_some());
        clear_editor_middle_click_scroll_for_buffer(&mut scroll, 7);
        assert!(scroll.is_none());

        scroll = Some(EditorMiddleClickScroll {
            pane_id: 2,
            buffer_id: 7,
            anchor_y: 100.0,
        });
        clear_editor_middle_click_scroll_for_pane(&mut scroll, 1);
        assert!(scroll.is_some());
        clear_editor_middle_click_scroll_for_pane(&mut scroll, 2);
        assert!(scroll.is_none());
    }
}
