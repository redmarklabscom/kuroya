use crate::KuroyaApp;

const MIN_PANE_WEIGHT: f32 = 0.01;

impl KuroyaApp {
    pub(crate) fn normalize_pane_weights(&mut self) {
        if self.panes.is_empty() {
            return;
        }

        if let [pane] = self.panes.as_mut_slice() {
            pane.weight = 1.0;
            return;
        }

        for pane in &mut self.panes {
            if !pane.weight.is_finite() || pane.weight <= 0.0 {
                pane.weight = 1.0;
            }
        }

        let sum: f64 = self.panes.iter().map(|pane| f64::from(pane.weight)).sum();
        if !sum.is_finite() || sum <= f64::from(f32::EPSILON) {
            let equal = 1.0 / self.panes.len() as f32;
            for pane in &mut self.panes {
                pane.weight = equal;
            }
            return;
        }

        for pane in &mut self.panes {
            pane.weight = (f64::from(pane.weight) / sum) as f32;
        }
    }

    pub(crate) fn reset_pane_weights(&mut self) {
        for pane in &mut self.panes {
            pane.weight = 1.0;
        }
        self.normalize_pane_weights();
        self.status = format!("Reset {} pane widths", self.panes.len());
    }

    pub(super) fn new_pane_insert_position_and_weight(&mut self) -> (usize, f32) {
        let insert_at = self
            .panes
            .iter()
            .position(|pane| pane.id == self.active_pane)
            .map(|position| position + 1)
            .unwrap_or(self.panes.len());
        let weight = if insert_at > 0 {
            let current = &mut self.panes[insert_at - 1];
            if !current.weight.is_finite() || current.weight <= 0.0 {
                current.weight = 1.0;
            }
            let split_weight = (current.weight * 0.5).max(MIN_PANE_WEIGHT);
            current.weight = (current.weight - split_weight).max(MIN_PANE_WEIGHT);
            split_weight
        } else {
            1.0
        };
        (insert_at, weight)
    }
}
