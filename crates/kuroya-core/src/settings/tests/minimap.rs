use super::*;

#[test]
fn minimap_section_header_lines_find_region_and_mark_labels() {
    let buffer = TextBuffer::from_text(
        1,
        None,
        "// #region API\nfn main() {}\n// MARK: - Helpers\n// MARK:\n".to_owned(),
    );

    let headers = minimap_section_header_lines(
        &buffer,
        true,
        true,
        DEFAULT_EDITOR_MINIMAP_MARK_SECTION_HEADER_REGEX,
    );

    assert_eq!(headers.get(&1).map(String::as_str), Some("API"));
    assert_eq!(headers.get(&3).map(String::as_str), Some("Helpers"));
    assert_eq!(headers.get(&4).map(String::as_str), Some("MARK"));
}

#[test]
fn minimap_section_header_lines_respect_visibility_and_invalid_regex() {
    let buffer = TextBuffer::from_text(1, None, "// #region API\n// MARK: Helpers\n".to_owned());

    let only_marks = minimap_section_header_lines(
        &buffer,
        false,
        true,
        DEFAULT_EDITOR_MINIMAP_MARK_SECTION_HEADER_REGEX,
    );
    assert_eq!(only_marks.keys().copied().collect::<Vec<_>>(), vec![2]);

    let invalid_regex = minimap_section_header_lines(&buffer, false, true, "(");
    assert!(invalid_regex.is_empty());

    let disabled = minimap_section_header_lines(
        &buffer,
        false,
        false,
        DEFAULT_EDITOR_MINIMAP_MARK_SECTION_HEADER_REGEX,
    );
    assert!(disabled.is_empty());
}

#[test]
fn minimap_section_header_lines_bound_long_line_scan() {
    let long_label = "header-fragment-".repeat(400);
    let late_marker = format!(
        "{}// MARK: Late",
        "x".repeat(MINIMAP_SECTION_HEADER_SCAN_CHAR_LIMIT + 8)
    );
    let buffer = TextBuffer::from_text(1, None, format!("// MARK: {long_label}\n{late_marker}\n"));

    let headers = minimap_section_header_lines(
        &buffer,
        true,
        true,
        DEFAULT_EDITOR_MINIMAP_MARK_SECTION_HEADER_REGEX,
    );

    let label = headers.get(&1).expect("leading marker should be found");
    assert!(label.starts_with("header-fragment-"));
    assert_eq!(label.chars().count(), 80);
    assert!(
        !headers.contains_key(&2),
        "markers beyond the bounded scan prefix should be ignored"
    );
}
