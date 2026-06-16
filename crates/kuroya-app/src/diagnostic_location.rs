use kuroya_core::Diagnostic;

pub(crate) fn diagnostic_jump_location(diagnostic: &Diagnostic) -> (usize, usize) {
    normalize_diagnostic_location(diagnostic.line, diagnostic.column)
}

pub(crate) fn normalize_diagnostic_location(line: usize, column: usize) -> (usize, usize) {
    (line.max(1), column.max(1))
}

#[cfg(test)]
mod tests {
    use super::normalize_diagnostic_location;

    #[test]
    fn diagnostic_location_clamps_zero_components_only() {
        assert_eq!(normalize_diagnostic_location(0, 0), (1, 1));
        assert_eq!(normalize_diagnostic_location(0, 8), (1, 8));
        assert_eq!(normalize_diagnostic_location(7, 0), (7, 1));
        assert_eq!(normalize_diagnostic_location(7, 8), (7, 8));
    }
}
