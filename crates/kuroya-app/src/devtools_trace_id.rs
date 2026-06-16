pub(crate) fn next_devtools_trace_id(current: &mut u64) -> u64 {
    *current = match current.wrapping_add(1) {
        0 => 1,
        next => next,
    };
    *current
}

#[cfg(test)]
mod tests {
    use super::next_devtools_trace_id;

    #[test]
    fn devtools_trace_ids_wrap_to_one_instead_of_reusing_max() {
        let mut current = u64::MAX - 1;

        assert_eq!(next_devtools_trace_id(&mut current), u64::MAX);
        assert_eq!(next_devtools_trace_id(&mut current), 1);
        assert_eq!(next_devtools_trace_id(&mut current), 2);
    }
}
