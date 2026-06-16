use crate::{
    KuroyaApp,
    folding::{best_folding_range_starting_at, toggle_folded_range},
    path_display::display_path_label_cow,
};
use std::path::Path;

impl KuroyaApp {
    pub(crate) fn apply_fold_at_line(&mut self, path: &Path, line: usize) -> bool {
        let Some(range) = self
            .folding_ranges
            .get(path)
            .and_then(|ranges| best_folding_range_starting_at(ranges, line))
        else {
            self.status = format!("No fold starts at line {line}");
            return false;
        };

        let folded = self.folded_ranges.entry(path.to_path_buf()).or_default();
        let folded_now = toggle_folded_range(folded, range);
        let hidden = range.end_line.saturating_sub(range.start_line);
        let path_label = display_path_label_cow(path);
        self.status = if folded_now {
            format!("Folded {hidden} lines at {path_label}:{}", range.start_line)
        } else {
            format!("Expanded fold at {path_label}:{}", range.start_line)
        };
        true
    }
}
