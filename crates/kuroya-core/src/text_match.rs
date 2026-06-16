pub fn ascii_case_insensitive_starts_with(haystack: &str, needle: &str) -> bool {
    AsciiCaseInsensitiveMatcher::new(needle).starts_with(haystack)
}

pub fn ascii_case_insensitive_contains(haystack: &str, needle: &str) -> bool {
    AsciiCaseInsensitiveMatcher::new(needle).contains(haystack)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AsciiCaseInsensitiveMatcher<'a> {
    needle_bytes: &'a [u8],
    needle_len: usize,
    needle_first: Option<u8>,
    needle_first_needs_ascii_case_fold: bool,
    needle_needs_ascii_case_fold: bool,
    needle_is_ascii: bool,
}

impl<'a> AsciiCaseInsensitiveMatcher<'a> {
    pub(crate) fn new(needle: &'a str) -> Self {
        let needle_bytes = needle.as_bytes();
        Self {
            needle_bytes,
            needle_len: needle_bytes.len(),
            needle_first: needle_bytes.first().copied(),
            needle_first_needs_ascii_case_fold: needle_bytes
                .first()
                .is_some_and(u8::is_ascii_alphabetic),
            needle_needs_ascii_case_fold: needle_bytes
                .iter()
                .any(|byte| byte.is_ascii_alphabetic()),
            needle_is_ascii: needle.is_ascii(),
        }
    }

    pub(crate) fn needle_len(&self) -> usize {
        self.needle_len
    }

    pub(crate) fn contains(&self, haystack: &str) -> bool {
        self.find_from(haystack, 0).is_some()
    }

    pub(crate) fn starts_with(&self, haystack: &str) -> bool {
        if self.needle_len > haystack.len() {
            return false;
        }

        let haystack_bytes = haystack.as_bytes();
        if self.needle_is_ascii && !self.needle_needs_ascii_case_fold {
            return &haystack_bytes[..self.needle_len] == self.needle_bytes;
        }

        if self.needle_is_ascii && self.needle_len == 1 {
            return ascii_case_insensitive_first_byte_matches(
                haystack_bytes[0],
                self.needle_bytes[0],
                self.needle_first_needs_ascii_case_fold,
            );
        }

        if self.needle_is_ascii {
            ascii_case_insensitive_bytes_eq(&haystack_bytes[..self.needle_len], self.needle_bytes)
        } else {
            haystack.is_char_boundary(self.needle_len)
                && ascii_case_insensitive_bytes_eq(
                    &haystack_bytes[..self.needle_len],
                    self.needle_bytes,
                )
        }
    }

    pub(crate) fn find_from(&self, haystack: &str, search_from: usize) -> Option<usize> {
        let Some(needle_first) = self.needle_first else {
            return empty_match_at(haystack, search_from);
        };

        let haystack_bytes = haystack.as_bytes();
        let max_start = haystack_bytes.len().checked_sub(self.needle_len)?;
        if search_from > max_start {
            return None;
        }
        if self.needle_is_ascii && !self.needle_needs_ascii_case_fold {
            return find_exact_ascii_bytes_from(haystack_bytes, self.needle_bytes, search_from);
        }
        if self.needle_is_ascii && self.needle_len == 1 {
            return find_ascii_case_insensitive_byte_from(
                haystack_bytes,
                needle_first,
                self.needle_first_needs_ascii_case_fold,
                search_from,
            );
        }

        let mut start = if self.needle_is_ascii {
            search_from
        } else {
            next_char_boundary_at_or_after(haystack, search_from, max_start)?
        };
        while start <= max_start {
            let offset = haystack_bytes[start..=max_start].iter().position(|byte| {
                ascii_case_insensitive_first_byte_matches(
                    *byte,
                    needle_first,
                    self.needle_first_needs_ascii_case_fold,
                )
            })?;
            start = start.checked_add(offset)?;
            let end = start.checked_add(self.needle_len)?;
            let matched = if self.needle_is_ascii {
                let tail_start = start.checked_add(1)?;
                self.needle_len == 1
                    || ascii_case_insensitive_bytes_eq(
                        &haystack_bytes[tail_start..end],
                        &self.needle_bytes[1..],
                    )
            } else {
                ascii_case_insensitive_bytes_eq(&haystack_bytes[start..end], self.needle_bytes)
                    && haystack.is_char_boundary(start)
                    && haystack.is_char_boundary(end)
            };
            if matched {
                return Some(start);
            }
            start = start.checked_add(1)?;
            if !self.needle_is_ascii {
                start = next_char_boundary_at_or_after(haystack, start, max_start)?;
            }
        }
        None
    }
}

pub fn ascii_case_insensitive_find_from(
    haystack: &str,
    needle: &str,
    search_from: usize,
) -> Option<usize> {
    AsciiCaseInsensitiveMatcher::new(needle).find_from(haystack, search_from)
}

fn empty_match_at(haystack: &str, search_from: usize) -> Option<usize> {
    if search_from > haystack.len() {
        return None;
    }
    haystack
        .is_char_boundary(search_from)
        .then_some(search_from)
}

fn find_exact_ascii_bytes_from(
    haystack: &[u8],
    needle: &[u8],
    search_from: usize,
) -> Option<usize> {
    let max_start = haystack.len().checked_sub(needle.len())?;
    if search_from > max_start {
        return None;
    }

    let first = needle[0];
    if needle.len() == 1 {
        return haystack[search_from..=max_start]
            .iter()
            .position(|byte| *byte == first)
            .and_then(|offset| search_from.checked_add(offset));
    }

    let mut start = search_from;
    while start <= max_start {
        let offset = haystack[start..=max_start]
            .iter()
            .position(|byte| *byte == first)?;
        start = start.checked_add(offset)?;
        let end = start.checked_add(needle.len())?;
        let tail_start = start.checked_add(1)?;
        if haystack[tail_start..end] == needle[1..] {
            return Some(start);
        }
        start = start.checked_add(1)?;
    }
    None
}

fn find_ascii_case_insensitive_byte_from(
    haystack: &[u8],
    needle: u8,
    needs_ascii_case_fold: bool,
    search_from: usize,
) -> Option<usize> {
    haystack
        .get(search_from..)?
        .iter()
        .position(|byte| {
            ascii_case_insensitive_first_byte_matches(*byte, needle, needs_ascii_case_fold)
        })
        .and_then(|offset| search_from.checked_add(offset))
}

fn ascii_case_insensitive_first_byte_matches(
    haystack_byte: u8,
    needle_first: u8,
    needs_ascii_case_fold: bool,
) -> bool {
    if needs_ascii_case_fold {
        haystack_byte.eq_ignore_ascii_case(&needle_first)
    } else {
        haystack_byte == needle_first
    }
}

fn ascii_case_insensitive_bytes_eq(left: &[u8], right: &[u8]) -> bool {
    if left == right {
        return true;
    }

    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

fn next_char_boundary_at_or_after(haystack: &str, mut index: usize, max: usize) -> Option<usize> {
    while index <= max && !haystack.is_char_boundary(index) {
        index = index.checked_add(1)?;
    }
    (index <= max).then_some(index)
}

#[cfg(test)]
mod tests {
    use super::{
        AsciiCaseInsensitiveMatcher, ascii_case_insensitive_contains,
        ascii_case_insensitive_find_from, ascii_case_insensitive_starts_with,
    };

    #[test]
    fn ascii_case_insensitive_contains_preserves_ascii_only_folding() {
        assert!(ascii_case_insensitive_contains(
            "Keyboard Shortcuts",
            "keyboard"
        ));
        assert!(ascii_case_insensitive_contains(
            "R\u{00e9}sum\u{00e9}",
            "r\u{00e9}sum"
        ));
        assert!(!ascii_case_insensitive_contains(
            "R\u{00e9}sum\u{00e9}",
            "R\u{00c9}SUM"
        ));
    }

    #[test]
    fn ascii_case_insensitive_starts_with_respects_char_boundaries() {
        assert!(ascii_case_insensitive_starts_with("TaskRunner", "task"));
        assert!(ascii_case_insensitive_starts_with(
            "R\u{00e9}sum\u{00e9}",
            "r\u{00e9}"
        ));
        assert!(!ascii_case_insensitive_starts_with(
            "R\u{00e9}sum\u{00e9}",
            "r\u{00e9}s\u{00ff}"
        ));
    }

    #[test]
    fn ascii_case_insensitive_starts_with_uses_prepared_matcher() {
        let matcher = AsciiCaseInsensitiveMatcher::new("task");

        assert!(matcher.starts_with("TaskRunner"));
        assert!(!matcher.starts_with("preTaskRunner"));
        assert_eq!(
            matcher.starts_with("TaskRunner"),
            ascii_case_insensitive_starts_with("TaskRunner", "task")
        );
    }

    #[test]
    fn ascii_case_insensitive_empty_needle_respects_search_boundaries() {
        assert_eq!(
            ascii_case_insensitive_find_from("R\u{00e9}sum\u{00e9}", "", 1),
            Some(1)
        );
        assert_eq!(
            ascii_case_insensitive_find_from("R\u{00e9}sum\u{00e9}", "", 2),
            None
        );
        assert_eq!(
            ascii_case_insensitive_find_from("R\u{00e9}sum\u{00e9}", "", 3),
            Some(3)
        );
        assert_eq!(
            ascii_case_insensitive_find_from("R\u{00e9}sum\u{00e9}", "", 10),
            None
        );
        assert_eq!(
            ascii_case_insensitive_find_from("R\u{00e9}sum\u{00e9}", "", usize::MAX),
            None
        );
    }

    #[test]
    fn ascii_case_insensitive_non_ascii_matches_start_on_char_boundaries_only() {
        let haystack = "x\u{00e9}\u{00e9}";

        assert_eq!(
            ascii_case_insensitive_find_from(haystack, "\u{00e9}", 1),
            Some(1)
        );
        assert_eq!(
            ascii_case_insensitive_find_from(haystack, "\u{00e9}", 2),
            Some(3)
        );
        assert_eq!(
            ascii_case_insensitive_find_from(haystack, "\u{00c9}", 1),
            None
        );
    }

    #[test]
    fn ascii_case_insensitive_ascii_needle_resumes_after_non_boundary_offset() {
        assert_eq!(
            ascii_case_insensitive_find_from("\u{00e9}Task", "task", 1),
            Some(2)
        );
        assert_eq!(
            ascii_case_insensitive_find_from("\u{00e9}Task", "task", 2),
            Some(2)
        );
    }

    #[test]
    fn ascii_case_insensitive_non_ascii_needle_resumes_at_char_boundary() {
        assert_eq!(
            ascii_case_insensitive_find_from(
                "R\u{00e9}sum\u{00e9} r\u{00e9}sum\u{00e9}",
                "\u{00e9}",
                2
            ),
            Some(6)
        );
        assert_eq!(
            ascii_case_insensitive_find_from(
                "R\u{00e9}sum\u{00e9} r\u{00e9}sum\u{00e9}",
                "\u{00c9}",
                0
            ),
            None
        );
    }

    #[test]
    fn ascii_case_insensitive_non_ascii_needle_with_ascii_prefix_resumes_at_char_boundary() {
        assert_eq!(
            ascii_case_insensitive_find_from("\u{00e9}a\u{00e9} a\u{00e9}", "a\u{00e9}", 1),
            Some(2)
        );
        assert_eq!(
            ascii_case_insensitive_find_from("\u{00e9}a\u{00e9} a\u{00e9}", "A\u{00e9}", 3),
            Some(6)
        );
    }

    #[test]
    fn ascii_case_insensitive_find_from_skips_before_offset() {
        assert_eq!(
            ascii_case_insensitive_find_from("needle x NEEDLE", "needle", 1),
            Some(9)
        );
        assert_eq!(
            ascii_case_insensitive_find_from("needle x NEEDLE", "needle", 10),
            None
        );
    }

    #[test]
    fn ascii_case_insensitive_matcher_reuses_needle_scan_state() {
        let matcher = AsciiCaseInsensitiveMatcher::new("needle");

        assert_eq!(matcher.needle_len(), 6);
        assert!(matcher.contains("one NEEDLE two needle"));
        assert!(matcher.starts_with("NEEDLE first"));
        assert_eq!(matcher.find_from("one NEEDLE two needle", 0), Some(4));
        assert_eq!(matcher.find_from("one NEEDLE two needle", 5), Some(15));
    }

    #[test]
    fn ascii_case_insensitive_matcher_uses_direct_first_byte_for_non_letters() {
        let digit = AsciiCaseInsensitiveMatcher::new("1alpha");
        let punctuation = AsciiCaseInsensitiveMatcher::new("-flag");

        assert_eq!(digit.find_from("x 1Alpha 1alpha", 0), Some(2));
        assert_eq!(digit.find_from("x 1Alpha 1alpha", 3), Some(9));
        assert_eq!(punctuation.find_from("args -Flag tail", 0), Some(5));
        assert_eq!(punctuation.find_from("args +flag tail", 0), None);
    }

    #[test]
    fn ascii_case_insensitive_matcher_fast_paths_single_ascii_byte() {
        let matcher = AsciiCaseInsensitiveMatcher::new("a");

        assert!(matcher.starts_with("Alpha"));
        assert_eq!(matcher.find_from("\u{00e9}Alpha alpha", 1), Some(2));
        assert_eq!(matcher.find_from("\u{00e9}Alpha alpha", 3), Some(6));
        assert_eq!(matcher.find_from("\u{00e9}Alpha alpha", 13), None);
    }

    #[test]
    fn ascii_case_insensitive_matcher_uses_exact_path_for_foldless_ascii() {
        let matcher = AsciiCaseInsensitiveMatcher::new("::");

        assert!(matcher.starts_with("::module"));
        assert!(!matcher.starts_with(":Module"));
        assert_eq!(matcher.find_from("one::two::three", 0), Some(3));
        assert_eq!(matcher.find_from("one::two::three", 4), Some(8));
        assert_eq!(matcher.find_from("\u{00e9}::tail", 1), Some(2));
    }

    #[test]
    fn ascii_case_insensitive_find_from_rejects_extreme_non_empty_offsets() {
        assert_eq!(
            ascii_case_insensitive_find_from("Task", "task", usize::MAX),
            None
        );
        assert_eq!(
            ascii_case_insensitive_find_from("one::two", "::", usize::MAX),
            None
        );
        assert_eq!(
            ascii_case_insensitive_find_from("R\u{00e9}sum\u{00e9}", "\u{00e9}", usize::MAX),
            None
        );
    }
}
