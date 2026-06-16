use crate::buffer::TextBuffer;
use regex::Regex;
use std::collections::BTreeMap;

pub(super) const MINIMAP_SECTION_HEADER_SCAN_CHAR_LIMIT: usize = 2_048;

pub fn minimap_section_header_lines(
    buffer: &TextBuffer,
    show_region_headers: bool,
    show_mark_headers: bool,
    mark_section_header_regex: &str,
) -> BTreeMap<usize, String> {
    if !show_region_headers && !show_mark_headers {
        return BTreeMap::new();
    }

    let mark_regex = show_mark_headers
        .then(|| Regex::new(mark_section_header_regex).ok())
        .flatten();
    let mut headers = BTreeMap::new();
    for line_idx in 0..buffer.len_lines() {
        let Some(line) =
            buffer.line_content_prefix(line_idx, MINIMAP_SECTION_HEADER_SCAN_CHAR_LIMIT)
        else {
            continue;
        };
        let line = line.trim_end_matches(['\r', '\n']);
        let label = if show_region_headers {
            minimap_region_section_header_label(line)
        } else {
            None
        }
        .or_else(|| {
            mark_regex
                .as_ref()
                .and_then(|regex| minimap_mark_section_header_label(line, regex))
        });

        if let Some(label) = label {
            headers.insert(line_idx + 1, label);
        }
    }
    headers
}

fn minimap_region_section_header_label(line: &str) -> Option<String> {
    let start = find_ascii_ignore_case(line, "#region")?;
    let label_start = start + "#region".len();
    let label = clean_minimap_section_label(&line[label_start..]);
    Some(if label.is_empty() {
        "region".to_owned()
    } else {
        label
    })
}

fn minimap_mark_section_header_label(line: &str, regex: &Regex) -> Option<String> {
    let captures = regex.captures(line)?;
    let label = captures
        .name("label")
        .or_else(|| captures.get(1))
        .or_else(|| captures.get(0))
        .map(|matched| clean_minimap_section_label(matched.as_str()))
        .unwrap_or_default();
    Some(if label.is_empty() {
        "MARK".to_owned()
    } else {
        label
    })
}

fn clean_minimap_section_label(label: &str) -> String {
    label
        .trim()
        .trim_start_matches(['-', ':', '#'])
        .trim()
        .trim_end_matches("*/")
        .trim_end_matches("-->")
        .trim_end_matches(['-', '*', '/', '>'])
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(80)
        .collect()
}

fn find_ascii_ignore_case(value: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    value.as_bytes().windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle.bytes())
            .all(|(left, right)| left.eq_ignore_ascii_case(&right))
    })
}
