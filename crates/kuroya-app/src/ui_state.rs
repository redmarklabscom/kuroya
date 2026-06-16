use eframe::egui::{InputState, Key};

pub(crate) fn clamp_selection(selection: &mut usize, len: usize) {
    *selection = (*selection).min(len.saturating_sub(1));
}

pub(crate) fn plain_key_pressed(input: &InputState, key: Key) -> bool {
    plain_key_modifiers_active(input) && input.key_pressed(key)
}

fn plain_key_modifiers_active(input: &InputState) -> bool {
    !input.modifiers.alt
        && !input.modifiers.ctrl
        && !input.modifiers.shift
        && !input.modifiers.mac_cmd
        && !input.modifiers.command
}

pub(crate) fn move_selection(selection: &mut usize, len: usize, delta: isize) {
    if len == 0 {
        *selection = 0;
        return;
    }

    *selection = wrapped_index(*selection, len, delta);
}

pub(crate) fn handle_list_navigation_keys(
    input: &InputState,
    selection: &mut usize,
    len: usize,
    page_rows: usize,
) -> bool {
    let before = *selection;
    if !plain_key_modifiers_active(input) {
        return false;
    }

    if input.key_pressed(Key::ArrowDown) {
        move_selection(selection, len, 1);
    }
    if input.key_pressed(Key::ArrowUp) {
        move_selection(selection, len, -1);
    }
    if input.key_pressed(Key::PageDown) {
        move_selection_by_page(selection, len, page_rows, 1);
    }
    if input.key_pressed(Key::PageUp) {
        move_selection_by_page(selection, len, page_rows, -1);
    }
    if input.key_pressed(Key::Home) {
        move_selection_to_start(selection);
    }
    if input.key_pressed(Key::End) {
        move_selection_to_end(selection, len);
    }

    before != *selection
}

pub(crate) fn move_selection_by_page(
    selection: &mut usize,
    len: usize,
    page_rows: usize,
    direction: isize,
) {
    if len == 0 {
        *selection = 0;
        return;
    }

    let step = page_rows.max(1);
    if direction < 0 {
        *selection = selection.saturating_sub(step);
    } else if direction > 0 {
        *selection = selection.saturating_add(step).min(len - 1);
    }
}

pub(crate) fn move_selection_to_end(selection: &mut usize, len: usize) {
    *selection = len.saturating_sub(1);
}

pub(crate) fn move_selection_to_start(selection: &mut usize) {
    *selection = 0;
}

pub(crate) fn selection_page_step(row_height: f32, viewport_height: f32) -> usize {
    if !row_height.is_finite()
        || !viewport_height.is_finite()
        || row_height <= 0.0
        || viewport_height <= 0.0
    {
        return 1;
    }

    (viewport_height / row_height).floor().max(1.0) as usize
}

pub(crate) fn clamp_scroll_target(
    target: f32,
    line_total: usize,
    row_height: f32,
    viewport_height: f32,
) -> f32 {
    if !target.is_finite() {
        return 0.0;
    }

    let row_height = finite_positive_or_one(row_height);
    let viewport_height = finite_positive_or_one(viewport_height);
    let content_height = line_total.max(1) as f32 * row_height;
    let max_offset = (content_height - viewport_height).max(0.0);
    target.max(0.0).min(max_offset)
}

pub(crate) fn next_smooth_scroll_offset(current: f32, target: f32, row_height: f32) -> f32 {
    let target = finite_nonnegative_or_zero(target);
    let current = if current.is_finite() {
        current.max(0.0)
    } else {
        target
    };

    if smooth_scroll_finished(current, target, row_height) {
        return target;
    }

    current + (target - current) * 0.28
}

pub(crate) fn selected_row_scroll_offset(
    selected: usize,
    len: usize,
    row_height: f32,
    viewport_height: f32,
) -> f32 {
    if len == 0 || !row_height.is_finite() || !viewport_height.is_finite() {
        return 0.0;
    }
    if row_height <= 0.0 || viewport_height <= 0.0 {
        return 0.0;
    }

    let selected_center = (selected.min(len - 1) as f32 + 0.5) * row_height;
    let target = selected_center - (viewport_height * 0.5);
    clamp_scroll_target(target, len, row_height, viewport_height)
}

pub(crate) fn smooth_scroll_finished(current: f32, target: f32, row_height: f32) -> bool {
    if !current.is_finite() || !target.is_finite() {
        return false;
    }

    (target - current).abs() <= smooth_scroll_epsilon(row_height)
}

fn smooth_scroll_epsilon(row_height: f32) -> f32 {
    if !row_height.is_finite() {
        return 1.0;
    }

    (row_height * 0.15).clamp(1.0, 6.0)
}

fn finite_nonnegative_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn finite_positive_or_one(value: f32) -> f32 {
    if value.is_finite() {
        value.max(1.0)
    } else {
        1.0
    }
}

pub(crate) fn wrapped_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }

    let current = current % len;
    if delta >= 0 {
        current
            .wrapping_add((delta as usize) % len)
            .wrapping_rem(len)
    } else {
        let step = delta.unsigned_abs() % len;
        if step <= current {
            current - step
        } else {
            len - (step - current)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_scroll_target, clamp_selection, move_selection, next_smooth_scroll_offset,
        selected_row_scroll_offset, smooth_scroll_finished, wrapped_index,
    };

    #[test]
    fn selection_helpers_recover_from_stale_indexes() {
        let mut selection = usize::MAX;
        clamp_selection(&mut selection, 3);
        assert_eq!(selection, 2);

        clamp_selection(&mut selection, 0);
        assert_eq!(selection, 0);

        move_selection(&mut selection, 0, 1);
        assert_eq!(selection, 0);

        assert_eq!(wrapped_index(usize::MAX, 4, 1), 0);
        assert_eq!(wrapped_index(0, 4, isize::MIN), 0);
    }

    #[test]
    fn clamp_scroll_target_sanitizes_invalid_runtime_offsets() {
        assert_eq!(clamp_scroll_target(f32::NAN, 10, 20.0, 100.0), 0.0);
        assert_eq!(clamp_scroll_target(f32::INFINITY, 10, 20.0, 100.0), 0.0);
        assert_eq!(clamp_scroll_target(80.0, 10, f32::NAN, 100.0), 0.0);
        assert_eq!(clamp_scroll_target(80.0, 10, 20.0, f32::NAN), 80.0);
    }

    #[test]
    fn smooth_scroll_helpers_sanitize_invalid_runtime_offsets() {
        assert_eq!(next_smooth_scroll_offset(f32::NAN, 120.0, 20.0), 120.0);
        let offset = next_smooth_scroll_offset(20.0, f32::NAN, 20.0);
        assert!((offset - 14.4).abs() < 0.001);
        assert!(!smooth_scroll_finished(f32::NAN, 0.0, 20.0));
        assert!(smooth_scroll_finished(0.0, 0.5, f32::NAN));
    }

    #[test]
    fn selected_row_scroll_offset_keeps_invalid_dimensions_at_zero() {
        assert_eq!(selected_row_scroll_offset(3, 10, f32::NAN, 100.0), 0.0);
        assert_eq!(selected_row_scroll_offset(3, 10, 20.0, f32::NAN), 0.0);
    }
}
