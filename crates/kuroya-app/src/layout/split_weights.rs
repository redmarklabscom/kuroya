use super::MIN_EDITOR_PANE_WIDTH;

pub(crate) fn normalize_weights(weights: &mut [f32]) {
    if weights.is_empty() {
        return;
    }

    let mut sum = 0.0f64;
    for weight in weights.iter_mut() {
        if !weight.is_finite() || *weight <= 0.0 {
            *weight = 1.0;
        }
        sum += f64::from(*weight);
    }

    if !sum.is_finite() || sum <= f64::from(f32::EPSILON) {
        let equal = 1.0 / weights.len() as f32;
        weights.fill(equal);
        return;
    }

    for weight in weights {
        *weight = (f64::from(*weight) / sum) as f32;
    }
}

pub(crate) fn adjust_split_weights(
    weights: &mut [f32],
    left_index: usize,
    delta_pixels: f32,
    content_width: f32,
) -> bool {
    let Some(right_index) = left_index.checked_add(1) else {
        return false;
    };
    if right_index >= weights.len() || !content_width.is_finite() || content_width <= 0.0 {
        return false;
    }

    normalize_weights(weights);
    if !delta_pixels.is_finite() {
        return false;
    }

    let pair_total = weights[left_index] + weights[right_index];
    if !pair_total.is_finite() || pair_total <= f32::EPSILON {
        return false;
    }

    let min_weight = min_split_weight(weights.len(), content_width).min(pair_total * 0.45);
    if pair_total <= min_weight * 2.0 {
        return false;
    }

    let delta_weight = delta_pixels / content_width;
    let next_left = (weights[left_index] + delta_weight).clamp(min_weight, pair_total - min_weight);
    if (next_left - weights[left_index]).abs() <= f32::EPSILON {
        return false;
    }

    weights[left_index] = next_left;
    weights[right_index] = pair_total - next_left;
    true
}

fn min_split_weight(pane_count: usize, content_width: f32) -> f32 {
    if pane_count == 0 || !content_width.is_finite() || content_width <= 0.0 {
        return 0.01;
    }

    let ideal = MIN_EDITOR_PANE_WIDTH / content_width;
    if ideal * pane_count as f32 <= 0.92 {
        ideal.max(0.01)
    } else {
        (1.0 / pane_count as f32 * 0.45).max(0.01)
    }
}

#[cfg(test)]
mod tests {
    use super::{adjust_split_weights, normalize_weights};

    #[test]
    fn normalize_weights_recovers_from_non_finite_values() {
        let mut weights = vec![f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 0.0];

        normalize_weights(&mut weights);

        assert!(weights.iter().all(|weight| weight.is_finite()));
        assert!(weights.iter().all(|weight| *weight > 0.0));
        assert!((weights.iter().sum::<f32>() - 1.0).abs() < 0.001);
    }

    #[test]
    fn adjust_split_weights_rejects_invalid_indexes_without_overflow() {
        let mut weights = vec![2.0, 1.0];

        assert!(!adjust_split_weights(
            &mut weights,
            usize::MAX,
            10.0,
            1000.0
        ));
        assert_eq!(weights, vec![2.0, 1.0]);

        assert!(!adjust_split_weights(&mut weights, 1, 10.0, 1000.0));
        assert_eq!(weights, vec![2.0, 1.0]);
    }
}
