use portable_pty::PtySize;

#[cfg(test)]
use kuroya_core::{
    DEFAULT_TERMINAL_FONT_SIZE, DEFAULT_TERMINAL_LETTER_SPACING, DEFAULT_TERMINAL_LINE_HEIGHT,
    DEFAULT_TERMINAL_MIN_COLUMNS, DEFAULT_TERMINAL_MIN_ROWS, DEFAULT_TERMINAL_SCROLLBACK_ROWS,
};

#[cfg(test)]
pub(crate) const TERMINAL_SCROLLBACK_ROWS: usize = DEFAULT_TERMINAL_SCROLLBACK_ROWS;

pub(crate) fn initial_terminal_size(min_rows: u16, min_columns: u16) -> PtySize {
    PtySize {
        rows: 24.max(min_rows),
        cols: 100.max(min_columns),
        pixel_width: 0,
        pixel_height: 0,
    }
}

pub(crate) fn terminal_size_from_points(
    width: f32,
    output_height: f32,
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
    min_rows: u16,
    min_columns: u16,
) -> PtySize {
    let (cell_width, cell_height) = terminal_cell_size(font_size, line_height, letter_spacing);
    let min_rows = min_rows.min(200);
    let min_columns = min_columns.min(400);
    PtySize {
        rows: ((output_height / cell_height).floor() as u16).clamp(min_rows, 200),
        cols: ((width / cell_width).floor() as u16).clamp(min_columns, 400),
        pixel_width: 0,
        pixel_height: 0,
    }
}

pub(crate) fn terminal_cell_size(
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
) -> (f32, f32) {
    (
        (font_size * 0.62 + letter_spacing).max(1.0),
        font_size * line_height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_size_tracks_available_points_with_clamps() {
        let size = terminal_size_from_points(
            721.0,
            162.0,
            DEFAULT_TERMINAL_FONT_SIZE,
            DEFAULT_TERMINAL_LINE_HEIGHT,
            DEFAULT_TERMINAL_LETTER_SPACING,
            DEFAULT_TERMINAL_MIN_ROWS,
            DEFAULT_TERMINAL_MIN_COLUMNS,
        );
        assert_eq!(size.cols, 96);
        assert_eq!(size.rows, 10);

        let min = terminal_size_from_points(
            1.0,
            1.0,
            DEFAULT_TERMINAL_FONT_SIZE,
            DEFAULT_TERMINAL_LINE_HEIGHT,
            DEFAULT_TERMINAL_LETTER_SPACING,
            DEFAULT_TERMINAL_MIN_ROWS,
            DEFAULT_TERMINAL_MIN_COLUMNS,
        );
        assert_eq!(min.cols, 20);
        assert_eq!(min.rows, 4);
    }

    #[test]
    fn terminal_size_uses_configured_minimum_rows_and_columns() {
        let size = terminal_size_from_points(
            1.0,
            1.0,
            DEFAULT_TERMINAL_FONT_SIZE,
            DEFAULT_TERMINAL_LINE_HEIGHT,
            DEFAULT_TERMINAL_LETTER_SPACING,
            12,
            60,
        );
        assert_eq!(size.cols, 60);
        assert_eq!(size.rows, 12);
    }

    #[test]
    fn terminal_cell_size_uses_configured_font_metrics() {
        assert_eq!(terminal_cell_size(14.0, 1.5, 0.0), (8.68, 21.0));
        assert_eq!(terminal_cell_size(14.0, 1.5, 2.0), (10.68, 21.0));
    }

    #[test]
    fn terminal_scrollback_is_larger_than_vscode_default() {
        assert_eq!(TERMINAL_SCROLLBACK_ROWS, 10_000);
    }
}
