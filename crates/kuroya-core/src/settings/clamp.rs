use super::{
    DEFAULT_DIFF_SPLIT_VIEW_DEFAULT_RATIO, DEFAULT_EDITOR_CURSOR_WIDTH, DEFAULT_EDITOR_FONT_WEIGHT,
    DEFAULT_EDITOR_LETTER_SPACING, DEFAULT_EDITOR_LINE_DECORATIONS_WIDTH,
    DEFAULT_EDITOR_LINE_HEIGHT, DEFAULT_EDITOR_MINIMAP_SECTION_HEADER_FONT_SIZE,
    DEFAULT_EDITOR_MINIMAP_SECTION_HEADER_LETTER_SPACING, DEFAULT_SCM_INPUT_FONT_SIZE,
    DEFAULT_TERMINAL_CURSOR_WIDTH, DEFAULT_TERMINAL_FONT_SIZE, DEFAULT_TERMINAL_LETTER_SPACING,
    DEFAULT_TERMINAL_LINE_HEIGHT, DEFAULT_TERMINAL_MINIMUM_CONTRAST_RATIO,
    DEFAULT_WINDOW_ZOOM_LEVEL, MAX_DIFF_RENDER_SIDE_BY_SIDE_INLINE_BREAKPOINT,
    MAX_DIFF_SPLIT_VIEW_DEFAULT_RATIO, MAX_EDITOR_ACCESSIBILITY_PAGE_SIZE,
    MAX_EDITOR_CODE_LENS_FONT_SIZE, MAX_EDITOR_COLOR_DECORATORS_LIMIT, MAX_EDITOR_CURSOR_HEIGHT,
    MAX_EDITOR_CURSOR_SURROUNDING_LINES, MAX_EDITOR_CURSOR_WIDTH,
    MAX_EDITOR_FOLDING_MAXIMUM_REGIONS, MAX_EDITOR_FONT_SIZE, MAX_EDITOR_INLAY_HINTS_FONT_SIZE,
    MAX_EDITOR_INLAY_HINTS_MAXIMUM_LENGTH, MAX_EDITOR_LETTER_SPACING,
    MAX_EDITOR_LINE_DECORATIONS_WIDTH, MAX_EDITOR_LINE_HEIGHT, MAX_EDITOR_LINE_NUMBERS_MIN_CHARS,
    MAX_EDITOR_MINIMAP_MAX_COLUMN, MAX_EDITOR_MINIMAP_SCALE,
    MAX_EDITOR_MINIMAP_SECTION_HEADER_FONT_SIZE, MAX_EDITOR_MINIMAP_SECTION_HEADER_LETTER_SPACING,
    MAX_EDITOR_MULTI_CURSOR_LIMIT, MAX_EDITOR_OVERVIEW_RULER_LANES, MAX_EDITOR_PADDING,
    MAX_EDITOR_REVEAL_HORIZONTAL_RIGHT_PADDING, MAX_EDITOR_RULER_COLUMN,
    MAX_EDITOR_SCROLL_BEYOND_LAST_COLUMN, MAX_EDITOR_SCROLL_SENSITIVITY, MAX_EDITOR_SCROLLBAR_SIZE,
    MAX_EDITOR_STICKY_SCROLL_MAX_LINE_COUNT, MAX_EDITOR_STOP_RENDERING_LINE_AFTER,
    MAX_EDITOR_TAB_INDEX, MAX_EDITOR_WORD_WRAP_COLUMN, MAX_GIT_AUTOFETCH_PERIOD,
    MAX_GIT_DETECT_WORKTREES_LIMIT, MAX_GIT_INPUT_VALIDATION_LENGTH,
    MAX_GIT_REPOSITORY_SCAN_MAX_DEPTH, MAX_HOVER_DELAY_MS, MAX_HOVER_HIDING_DELAY_MS,
    MAX_INLINE_SUGGEST_MIN_SHOW_DELAY_MS, MAX_QUICK_SUGGESTIONS_DELAY_MS,
    MAX_SCM_DIFF_DECORATIONS_GUTTER_WIDTH, MAX_SCM_GRAPH_PAGE_SIZE, MAX_SCM_INPUT_FONT_SIZE,
    MAX_SCM_INPUT_LINE_COUNT, MAX_SCM_REPOSITORIES_VISIBLE, MAX_SUGGEST_FONT_SIZE,
    MAX_SUGGEST_LINE_HEIGHT, MAX_TERMINAL_BELL_DURATION_MS, MAX_TERMINAL_CURSOR_WIDTH,
    MAX_TERMINAL_FONT_SIZE, MAX_TERMINAL_LETTER_SPACING, MAX_TERMINAL_LINE_HEIGHT,
    MAX_TERMINAL_MIN_COLUMNS, MAX_TERMINAL_MIN_ROWS, MAX_TERMINAL_MINIMUM_CONTRAST_RATIO,
    MAX_TERMINAL_SCROLL_SENSITIVITY, MAX_TERMINAL_SCROLLBACK_ROWS, MAX_WINDOW_ZOOM_LEVEL,
    MIN_AUTOSAVE_DELAY_MS, MIN_DIFF_RENDER_SIDE_BY_SIDE_INLINE_BREAKPOINT,
    MIN_DIFF_SPLIT_VIEW_DEFAULT_RATIO, MIN_EDITOR_ACCESSIBILITY_PAGE_SIZE,
    MIN_EDITOR_CODE_LENS_FONT_SIZE, MIN_EDITOR_COLOR_DECORATORS_LIMIT, MIN_EDITOR_CURSOR_HEIGHT,
    MIN_EDITOR_CURSOR_WIDTH, MIN_EDITOR_FOLDING_MAXIMUM_REGIONS, MIN_EDITOR_FONT_SIZE,
    MIN_EDITOR_INLAY_HINTS_FONT_SIZE, MIN_EDITOR_INLAY_HINTS_MAXIMUM_LENGTH,
    MIN_EDITOR_LETTER_SPACING, MIN_EDITOR_LINE_DECORATIONS_WIDTH, MIN_EDITOR_LINE_HEIGHT,
    MIN_EDITOR_LINE_NUMBERS_MIN_CHARS, MIN_EDITOR_MINIMAP_MAX_COLUMN, MIN_EDITOR_MINIMAP_SCALE,
    MIN_EDITOR_MINIMAP_SECTION_HEADER_FONT_SIZE, MIN_EDITOR_MINIMAP_SECTION_HEADER_LETTER_SPACING,
    MIN_EDITOR_MULTI_CURSOR_LIMIT, MIN_EDITOR_OVERVIEW_RULER_LANES, MIN_EDITOR_PADDING,
    MIN_EDITOR_REVEAL_HORIZONTAL_RIGHT_PADDING, MIN_EDITOR_SCROLL_BEYOND_LAST_COLUMN,
    MIN_EDITOR_SCROLL_SENSITIVITY, MIN_EDITOR_SCROLLBAR_SIZE,
    MIN_EDITOR_STICKY_SCROLL_MAX_LINE_COUNT, MIN_EDITOR_STOP_RENDERING_LINE_AFTER,
    MIN_EDITOR_TAB_INDEX, MIN_EDITOR_WORD_WRAP_COLUMN, MIN_GIT_AUTOFETCH_PERIOD,
    MIN_GIT_DETECT_WORKTREES_LIMIT, MIN_GIT_INPUT_VALIDATION_LENGTH,
    MIN_GIT_REPOSITORY_SCAN_MAX_DEPTH, MIN_HOVER_DELAY_MS, MIN_HOVER_HIDING_DELAY_MS,
    MIN_INLINE_SUGGEST_MIN_SHOW_DELAY_MS, MIN_QUICK_SUGGESTIONS_DELAY_MS,
    MIN_SCM_DIFF_DECORATIONS_GUTTER_WIDTH, MIN_SCM_GRAPH_PAGE_SIZE, MIN_SCM_INPUT_FONT_SIZE,
    MIN_SCM_INPUT_LINE_COUNT, MIN_SCM_REPOSITORIES_VISIBLE, MIN_SUGGEST_FONT_SIZE,
    MIN_SUGGEST_LINE_HEIGHT, MIN_TERMINAL_BELL_DURATION_MS, MIN_TERMINAL_CURSOR_WIDTH,
    MIN_TERMINAL_FONT_SIZE, MIN_TERMINAL_LETTER_SPACING, MIN_TERMINAL_LINE_HEIGHT,
    MIN_TERMINAL_MIN_COLUMNS, MIN_TERMINAL_MIN_ROWS, MIN_TERMINAL_MINIMUM_CONTRAST_RATIO,
    MIN_TERMINAL_SCROLL_SENSITIVITY, MIN_TERMINAL_SCROLLBACK_ROWS, MIN_WINDOW_ZOOM_LEVEL,
};

pub fn clamp_terminal_scrollback_rows(rows: usize) -> usize {
    rows.clamp(MIN_TERMINAL_SCROLLBACK_ROWS, MAX_TERMINAL_SCROLLBACK_ROWS)
}

pub fn clamp_terminal_min_rows(rows: u16) -> u16 {
    rows.clamp(MIN_TERMINAL_MIN_ROWS, MAX_TERMINAL_MIN_ROWS)
}

pub fn clamp_terminal_min_columns(columns: u16) -> u16 {
    columns.clamp(MIN_TERMINAL_MIN_COLUMNS, MAX_TERMINAL_MIN_COLUMNS)
}

pub fn clamp_terminal_font_size(size: f32) -> f32 {
    if size.is_finite() {
        size.clamp(MIN_TERMINAL_FONT_SIZE, MAX_TERMINAL_FONT_SIZE)
    } else {
        DEFAULT_TERMINAL_FONT_SIZE
    }
}

pub fn clamp_terminal_line_height(line_height: f32) -> f32 {
    if line_height.is_finite() {
        line_height.clamp(MIN_TERMINAL_LINE_HEIGHT, MAX_TERMINAL_LINE_HEIGHT)
    } else {
        DEFAULT_TERMINAL_LINE_HEIGHT
    }
}

pub fn clamp_terminal_letter_spacing(letter_spacing: f32) -> f32 {
    if letter_spacing.is_finite() {
        letter_spacing.clamp(MIN_TERMINAL_LETTER_SPACING, MAX_TERMINAL_LETTER_SPACING)
    } else {
        DEFAULT_TERMINAL_LETTER_SPACING
    }
}

pub fn clamp_terminal_cursor_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_TERMINAL_CURSOR_WIDTH, MAX_TERMINAL_CURSOR_WIDTH)
    } else {
        DEFAULT_TERMINAL_CURSOR_WIDTH
    }
}

pub fn clamp_terminal_minimum_contrast_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(
            MIN_TERMINAL_MINIMUM_CONTRAST_RATIO,
            MAX_TERMINAL_MINIMUM_CONTRAST_RATIO,
        )
    } else {
        DEFAULT_TERMINAL_MINIMUM_CONTRAST_RATIO
    }
}

pub fn clamp_terminal_bell_duration_ms(duration_ms: u64) -> u64 {
    duration_ms.clamp(MIN_TERMINAL_BELL_DURATION_MS, MAX_TERMINAL_BELL_DURATION_MS)
}

pub fn clamp_terminal_scroll_sensitivity(sensitivity: f32, default: f32) -> f32 {
    if sensitivity.is_finite() {
        sensitivity.clamp(
            MIN_TERMINAL_SCROLL_SENSITIVITY,
            MAX_TERMINAL_SCROLL_SENSITIVITY,
        )
    } else {
        default
    }
}

pub fn clamp_editor_font_size(size: f32, default: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        size.clamp(MIN_EDITOR_FONT_SIZE, MAX_EDITOR_FONT_SIZE)
    } else {
        default
    }
}

pub fn clamp_editor_cursor_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_EDITOR_CURSOR_WIDTH, MAX_EDITOR_CURSOR_WIDTH)
    } else {
        DEFAULT_EDITOR_CURSOR_WIDTH
    }
}

pub fn clamp_editor_cursor_height(height: usize) -> usize {
    height.clamp(MIN_EDITOR_CURSOR_HEIGHT, MAX_EDITOR_CURSOR_HEIGHT)
}

pub fn clamp_editor_accessibility_page_size(page_size: usize) -> usize {
    page_size.clamp(
        MIN_EDITOR_ACCESSIBILITY_PAGE_SIZE,
        MAX_EDITOR_ACCESSIBILITY_PAGE_SIZE,
    )
}

pub fn clamp_editor_tab_index(tab_index: i64) -> i64 {
    tab_index.clamp(MIN_EDITOR_TAB_INDEX, MAX_EDITOR_TAB_INDEX)
}

pub fn clamp_editor_reveal_horizontal_right_padding(padding: usize) -> usize {
    padding.clamp(
        MIN_EDITOR_REVEAL_HORIZONTAL_RIGHT_PADDING,
        MAX_EDITOR_REVEAL_HORIZONTAL_RIGHT_PADDING,
    )
}

pub fn clamp_editor_overview_ruler_lanes(lanes: usize) -> usize {
    lanes.clamp(
        MIN_EDITOR_OVERVIEW_RULER_LANES,
        MAX_EDITOR_OVERVIEW_RULER_LANES,
    )
}

pub fn clamp_editor_cursor_surrounding_lines(lines: usize) -> usize {
    lines.min(MAX_EDITOR_CURSOR_SURROUNDING_LINES)
}

pub fn clamp_autosave_delay_ms(delay_ms: u64) -> u64 {
    delay_ms.clamp(MIN_AUTOSAVE_DELAY_MS, super::MAX_AUTOSAVE_DELAY_MS)
}

pub fn clamp_editor_line_height(line_height: f32) -> f32 {
    if line_height.is_finite() {
        line_height.clamp(MIN_EDITOR_LINE_HEIGHT, MAX_EDITOR_LINE_HEIGHT)
    } else {
        DEFAULT_EDITOR_LINE_HEIGHT
    }
}

pub fn clamp_editor_letter_spacing(letter_spacing: f32) -> f32 {
    if letter_spacing.is_finite() {
        letter_spacing.clamp(MIN_EDITOR_LETTER_SPACING, MAX_EDITOR_LETTER_SPACING)
    } else {
        DEFAULT_EDITOR_LETTER_SPACING
    }
}

pub fn sanitize_editor_font_weight(value: &str) -> String {
    let trimmed = value.trim();
    if matches!(trimmed, "normal" | "bold") {
        return trimmed.to_owned();
    }

    if let Ok(weight) = trimmed.parse::<u16>()
        && (1..=1000).contains(&weight)
    {
        return weight.to_string();
    }

    DEFAULT_EDITOR_FONT_WEIGHT.to_owned()
}

pub fn clamp_editor_ruler_column(column: usize) -> usize {
    column.min(MAX_EDITOR_RULER_COLUMN)
}

pub fn clamp_editor_line_decorations_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(
            MIN_EDITOR_LINE_DECORATIONS_WIDTH,
            MAX_EDITOR_LINE_DECORATIONS_WIDTH,
        )
    } else {
        DEFAULT_EDITOR_LINE_DECORATIONS_WIDTH
    }
}

pub(super) fn clamp_editor_line_decorations_width_ch(chars: f32) -> f32 {
    let max_chars = MAX_EDITOR_LINE_DECORATIONS_WIDTH / 8.0;
    if chars.is_finite() {
        chars.clamp(0.0, max_chars)
    } else {
        DEFAULT_EDITOR_LINE_DECORATIONS_WIDTH / 8.0
    }
}

pub fn clamp_editor_line_numbers_min_chars(chars: usize) -> usize {
    chars.clamp(
        MIN_EDITOR_LINE_NUMBERS_MIN_CHARS,
        MAX_EDITOR_LINE_NUMBERS_MIN_CHARS,
    )
}

pub fn clamp_editor_word_wrap_column(column: usize) -> usize {
    column.clamp(MIN_EDITOR_WORD_WRAP_COLUMN, MAX_EDITOR_WORD_WRAP_COLUMN)
}

pub fn clamp_quick_suggestions_delay_ms(delay_ms: usize) -> usize {
    delay_ms.clamp(
        MIN_QUICK_SUGGESTIONS_DELAY_MS,
        MAX_QUICK_SUGGESTIONS_DELAY_MS,
    )
}

pub fn clamp_hover_delay_ms(delay_ms: usize) -> usize {
    delay_ms.clamp(MIN_HOVER_DELAY_MS, MAX_HOVER_DELAY_MS)
}

pub fn clamp_hover_hiding_delay_ms(delay_ms: usize) -> usize {
    delay_ms.clamp(MIN_HOVER_HIDING_DELAY_MS, MAX_HOVER_HIDING_DELAY_MS)
}

pub fn clamp_suggest_font_size(font_size: usize) -> usize {
    font_size.clamp(MIN_SUGGEST_FONT_SIZE, MAX_SUGGEST_FONT_SIZE)
}

pub fn clamp_suggest_line_height(line_height: usize) -> usize {
    line_height.clamp(MIN_SUGGEST_LINE_HEIGHT, MAX_SUGGEST_LINE_HEIGHT)
}

pub fn clamp_editor_inlay_hints_font_size(font_size: usize) -> usize {
    font_size.clamp(
        MIN_EDITOR_INLAY_HINTS_FONT_SIZE,
        MAX_EDITOR_INLAY_HINTS_FONT_SIZE,
    )
}

pub fn clamp_editor_inlay_hints_maximum_length(length: usize) -> usize {
    length.clamp(
        MIN_EDITOR_INLAY_HINTS_MAXIMUM_LENGTH,
        MAX_EDITOR_INLAY_HINTS_MAXIMUM_LENGTH,
    )
}

pub fn clamp_editor_code_lens_font_size(font_size: usize) -> usize {
    font_size.clamp(
        MIN_EDITOR_CODE_LENS_FONT_SIZE,
        MAX_EDITOR_CODE_LENS_FONT_SIZE,
    )
}

pub fn clamp_editor_multi_cursor_limit(limit: usize) -> usize {
    limit.clamp(MIN_EDITOR_MULTI_CURSOR_LIMIT, MAX_EDITOR_MULTI_CURSOR_LIMIT)
}

pub fn clamp_inline_suggest_min_show_delay_ms(delay_ms: usize) -> usize {
    delay_ms.clamp(
        MIN_INLINE_SUGGEST_MIN_SHOW_DELAY_MS,
        MAX_INLINE_SUGGEST_MIN_SHOW_DELAY_MS,
    )
}

pub fn clamp_editor_stop_rendering_line_after(chars: i64) -> i64 {
    chars.clamp(
        MIN_EDITOR_STOP_RENDERING_LINE_AFTER,
        MAX_EDITOR_STOP_RENDERING_LINE_AFTER,
    )
}

pub fn editor_stop_rendering_line_after_limit(chars: i64) -> Option<usize> {
    let chars = clamp_editor_stop_rendering_line_after(chars);
    (chars >= 0).then_some(chars as usize)
}

pub fn clamp_editor_minimap_max_column(column: usize) -> usize {
    column.clamp(MIN_EDITOR_MINIMAP_MAX_COLUMN, MAX_EDITOR_MINIMAP_MAX_COLUMN)
}

pub fn clamp_editor_minimap_scale(scale: usize) -> usize {
    scale.clamp(MIN_EDITOR_MINIMAP_SCALE, MAX_EDITOR_MINIMAP_SCALE)
}

pub fn clamp_editor_minimap_section_header_font_size(font_size: f32) -> f32 {
    if font_size.is_finite() {
        font_size.clamp(
            MIN_EDITOR_MINIMAP_SECTION_HEADER_FONT_SIZE,
            MAX_EDITOR_MINIMAP_SECTION_HEADER_FONT_SIZE,
        )
    } else {
        DEFAULT_EDITOR_MINIMAP_SECTION_HEADER_FONT_SIZE
    }
}

pub fn clamp_editor_minimap_section_header_letter_spacing(letter_spacing: f32) -> f32 {
    if letter_spacing.is_finite() {
        letter_spacing.clamp(
            MIN_EDITOR_MINIMAP_SECTION_HEADER_LETTER_SPACING,
            MAX_EDITOR_MINIMAP_SECTION_HEADER_LETTER_SPACING,
        )
    } else {
        DEFAULT_EDITOR_MINIMAP_SECTION_HEADER_LETTER_SPACING
    }
}

pub fn clamp_editor_sticky_scroll_max_line_count(lines: usize) -> usize {
    lines.clamp(
        MIN_EDITOR_STICKY_SCROLL_MAX_LINE_COUNT,
        MAX_EDITOR_STICKY_SCROLL_MAX_LINE_COUNT,
    )
}

pub fn clamp_editor_padding(pixels: usize) -> usize {
    pixels.clamp(MIN_EDITOR_PADDING, MAX_EDITOR_PADDING)
}

pub fn clamp_editor_scrollbar_size(pixels: usize) -> usize {
    pixels.clamp(MIN_EDITOR_SCROLLBAR_SIZE, MAX_EDITOR_SCROLLBAR_SIZE)
}

pub fn clamp_editor_scroll_beyond_last_column(columns: usize) -> usize {
    columns.clamp(
        MIN_EDITOR_SCROLL_BEYOND_LAST_COLUMN,
        MAX_EDITOR_SCROLL_BEYOND_LAST_COLUMN,
    )
}

pub fn clamp_editor_scroll_sensitivity(sensitivity: f32, default: f32) -> f32 {
    if sensitivity.is_finite() && sensitivity > 0.0 {
        sensitivity.clamp(MIN_EDITOR_SCROLL_SENSITIVITY, MAX_EDITOR_SCROLL_SENSITIVITY)
    } else {
        default
    }
}

pub fn clamp_editor_color_decorators_limit(limit: usize) -> usize {
    limit.clamp(
        MIN_EDITOR_COLOR_DECORATORS_LIMIT,
        MAX_EDITOR_COLOR_DECORATORS_LIMIT,
    )
}

pub fn clamp_editor_folding_maximum_regions(regions: usize) -> usize {
    regions.clamp(
        MIN_EDITOR_FOLDING_MAXIMUM_REGIONS,
        MAX_EDITOR_FOLDING_MAXIMUM_REGIONS,
    )
}

pub fn clamp_diff_render_side_by_side_inline_breakpoint(width: usize) -> usize {
    width.clamp(
        MIN_DIFF_RENDER_SIDE_BY_SIDE_INLINE_BREAKPOINT,
        MAX_DIFF_RENDER_SIDE_BY_SIDE_INLINE_BREAKPOINT,
    )
}

pub fn clamp_diff_split_view_default_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(
            MIN_DIFF_SPLIT_VIEW_DEFAULT_RATIO,
            MAX_DIFF_SPLIT_VIEW_DEFAULT_RATIO,
        )
    } else {
        DEFAULT_DIFF_SPLIT_VIEW_DEFAULT_RATIO
    }
}

pub fn clamp_scm_diff_decorations_gutter_width(width: usize) -> usize {
    width.clamp(
        MIN_SCM_DIFF_DECORATIONS_GUTTER_WIDTH,
        MAX_SCM_DIFF_DECORATIONS_GUTTER_WIDTH,
    )
}

pub fn clamp_scm_input_line_count(lines: usize) -> usize {
    lines.clamp(MIN_SCM_INPUT_LINE_COUNT, MAX_SCM_INPUT_LINE_COUNT)
}

pub fn clamp_scm_input_font_size(size: f32) -> f32 {
    if size.is_finite() {
        size.clamp(MIN_SCM_INPUT_FONT_SIZE, MAX_SCM_INPUT_FONT_SIZE)
    } else {
        DEFAULT_SCM_INPUT_FONT_SIZE
    }
}

pub fn clamp_git_input_validation_length(length: usize) -> usize {
    length.clamp(
        MIN_GIT_INPUT_VALIDATION_LENGTH,
        MAX_GIT_INPUT_VALIDATION_LENGTH,
    )
}

pub fn clamp_git_repository_scan_max_depth(depth: usize) -> usize {
    depth.clamp(
        MIN_GIT_REPOSITORY_SCAN_MAX_DEPTH,
        MAX_GIT_REPOSITORY_SCAN_MAX_DEPTH,
    )
}

pub fn clamp_git_autofetch_period(period: usize) -> usize {
    period.clamp(MIN_GIT_AUTOFETCH_PERIOD, MAX_GIT_AUTOFETCH_PERIOD)
}

pub fn clamp_git_detect_worktrees_limit(limit: usize) -> usize {
    limit.clamp(
        MIN_GIT_DETECT_WORKTREES_LIMIT,
        MAX_GIT_DETECT_WORKTREES_LIMIT,
    )
}

pub fn clamp_scm_graph_page_size(size: usize) -> usize {
    size.clamp(MIN_SCM_GRAPH_PAGE_SIZE, MAX_SCM_GRAPH_PAGE_SIZE)
}

pub fn clamp_scm_repositories_visible(count: usize) -> usize {
    count.clamp(MIN_SCM_REPOSITORIES_VISIBLE, MAX_SCM_REPOSITORIES_VISIBLE)
}

pub fn clamp_window_zoom_level(level: f32) -> f32 {
    if level.is_finite() {
        level.clamp(MIN_WINDOW_ZOOM_LEVEL, MAX_WINDOW_ZOOM_LEVEL)
    } else {
        DEFAULT_WINDOW_ZOOM_LEVEL
    }
}

pub fn window_zoom_factor(level: f32) -> f32 {
    1.2_f32.powf(clamp_window_zoom_level(level))
}
