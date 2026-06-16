use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HoverMarkdownBlock {
    Heading {
        level: usize,
        text: String,
    },
    Paragraph(String),
    List(Vec<String>),
    Quote(Vec<String>),
    Code {
        language: Option<String>,
        text: String,
    },
}

const MAX_HOVER_MARKDOWN_PARSE_CHARS: usize = 16_000;
const MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS: usize = 512;
const HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE: &str = "[Hover truncated]";
const HOVER_MARKDOWN_EXTERNAL_TRUNCATED_NOTICES: [&str; 2] = [
    "[Documentation truncated]",
    "[Signature documentation truncated]",
];

pub(crate) fn parse_hover_markdown(input: &str) -> Vec<HoverMarkdownBlock> {
    let (input, parse_truncated) = bounded_hover_markdown_parse_input(input);
    let mut blocks = Vec::new();
    let mut paragraph = String::new();
    let mut list = Vec::new();
    let mut quote = Vec::new();
    let mut lines = input.lines();

    while let Some(line) = lines.next() {
        if let Some(fence) = code_fence(line) {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_list(&mut list, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            let mut code = String::new();
            let mut pending_blank_code_lines = String::new();
            for code_line in lines.by_ref() {
                if is_code_fence(code_line, fence.kind, fence.marker_len) {
                    break;
                }
                push_code_block_line(&mut code, &mut pending_blank_code_lines, code_line);
            }
            if !code.is_empty() {
                blocks.push(HoverMarkdownBlock::Code {
                    language: fence.language.map(Cow::into_owned),
                    text: code,
                });
            }
        } else if line.trim().is_empty() {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_list(&mut list, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
        } else if let Some((level, text)) = markdown_heading_line(line) {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_list(&mut list, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            blocks.push(HoverMarkdownBlock::Heading { level, text });
        } else if let Some(item) = markdown_quote_line(line) {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_list(&mut list, &mut blocks);
            quote.push(item);
        } else if let Some(item) = markdown_list_item(line) {
            flush_paragraph(&mut paragraph, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            list.push(item);
        } else if markdown_list_continuation(line) && !list.is_empty() {
            if let Some(item) = list.last_mut() {
                let continuation = line.trim();
                let continuation = clean_markdown_text(continuation);
                if !continuation.is_empty() {
                    item.reserve(1 + continuation.len());
                    item.push(' ');
                    item.push_str(&continuation);
                }
            }
        } else {
            flush_list(&mut list, &mut blocks);
            flush_quote(&mut quote, &mut blocks);
            push_paragraph_line(&mut paragraph, line.trim());
        }
    }

    flush_paragraph(&mut paragraph, &mut blocks);
    flush_list(&mut list, &mut blocks);
    flush_quote(&mut quote, &mut blocks);
    if parse_truncated {
        push_parse_truncation_notice(&mut blocks);
    }
    limit_hover_markdown_render_rows(&mut blocks);
    blocks
}

fn bounded_hover_markdown_parse_input(input: &str) -> (&str, bool) {
    let Some((cut, _)) = input.char_indices().nth(MAX_HOVER_MARKDOWN_PARSE_CHARS) else {
        return (input, false);
    };

    (&input[..cut], true)
}

fn push_parse_truncation_notice(blocks: &mut Vec<HoverMarkdownBlock>) {
    push_hover_markdown_truncation_notice(blocks, HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE);
}

fn push_hover_markdown_truncation_notice(blocks: &mut Vec<HoverMarkdownBlock>, notice: &str) {
    if matches!(blocks.last(), Some(HoverMarkdownBlock::Paragraph(text)) if text == notice) {
        return;
    }

    blocks.push(HoverMarkdownBlock::Paragraph(notice.to_owned()));
}

fn limit_hover_markdown_render_rows(blocks: &mut Vec<HoverMarkdownBlock>) {
    let trailing_notice = trailing_hover_markdown_truncation_notice(blocks);
    let content_blocks = blocks
        .len()
        .saturating_sub(usize::from(trailing_notice.is_some()));
    let max_content_rows = MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS
        .saturating_sub(usize::from(trailing_notice.is_some()));
    let mut rows = 0usize;
    for index in 0..content_blocks {
        let block_rows = hover_markdown_block_render_rows(&blocks[index]);
        if rows.saturating_add(block_rows) <= max_content_rows {
            rows += block_rows;
            continue;
        }

        let remaining = max_content_rows.saturating_sub(rows);
        let keep_blocks =
            if remaining > 0 && truncate_hover_markdown_block_rows(&mut blocks[index], remaining) {
                index + 1
            } else {
                index
            };
        blocks.truncate(keep_blocks);
        push_hover_markdown_truncation_notice(
            blocks,
            trailing_notice.unwrap_or(HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE),
        );
        return;
    }
}

fn trailing_hover_markdown_truncation_notice(
    blocks: &[HoverMarkdownBlock],
) -> Option<&'static str> {
    let Some(HoverMarkdownBlock::Paragraph(text)) = blocks.last() else {
        return None;
    };
    hover_markdown_truncation_notice(text)
}

fn hover_markdown_truncation_notice(text: &str) -> Option<&'static str> {
    if text == HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE {
        return Some(HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE);
    }
    HOVER_MARKDOWN_EXTERNAL_TRUNCATED_NOTICES
        .iter()
        .copied()
        .find(|notice| text == *notice)
}

fn hover_markdown_block_render_rows(block: &HoverMarkdownBlock) -> usize {
    match block {
        HoverMarkdownBlock::List(items) => items.len(),
        HoverMarkdownBlock::Quote(lines) => lines.len(),
        HoverMarkdownBlock::Code { text, .. } => code_block_render_rows(text),
        HoverMarkdownBlock::Heading { .. } | HoverMarkdownBlock::Paragraph(_) => 1,
    }
}

fn truncate_hover_markdown_block_rows(block: &mut HoverMarkdownBlock, max_rows: usize) -> bool {
    match block {
        HoverMarkdownBlock::List(items) if max_rows < items.len() => {
            items.truncate(max_rows);
            !items.is_empty()
        }
        HoverMarkdownBlock::Quote(lines) if max_rows < lines.len() => {
            lines.truncate(max_rows);
            !lines.is_empty()
        }
        HoverMarkdownBlock::Code { text, .. } if max_rows < code_block_render_rows(text) => {
            truncate_code_block_render_rows(text, max_rows);
            !text.is_empty()
        }
        _ => false,
    }
}

fn code_block_render_rows(text: &str) -> usize {
    text.lines().count().max(1)
}

fn truncate_code_block_render_rows(text: &mut String, max_rows: usize) {
    if max_rows == 0 {
        text.clear();
        return;
    }

    let mut rows = 1usize;
    let mut cursor = 0usize;
    while let Some(relative_newline) = text[cursor..].find('\n') {
        let newline = cursor + relative_newline;
        if rows == max_rows {
            text.truncate(newline);
            return;
        }
        rows += 1;
        cursor = newline + 1;
    }
}

fn push_paragraph_line(paragraph: &mut String, line: &str) {
    let line = clean_markdown_text(line);
    if line.is_empty() {
        return;
    }

    paragraph.reserve(line.len() + usize::from(!paragraph.is_empty()));
    if !paragraph.is_empty() {
        paragraph.push(' ');
    }
    paragraph.push_str(&line);
}

fn flush_paragraph(paragraph: &mut String, blocks: &mut Vec<HoverMarkdownBlock>) {
    if paragraph.is_empty() {
        return;
    }

    blocks.push(HoverMarkdownBlock::Paragraph(std::mem::take(paragraph)));
}

fn flush_list(list: &mut Vec<String>, blocks: &mut Vec<HoverMarkdownBlock>) {
    if list.is_empty() {
        return;
    }

    blocks.push(HoverMarkdownBlock::List(std::mem::take(list)));
}

fn flush_quote(quote: &mut Vec<String>, blocks: &mut Vec<HoverMarkdownBlock>) {
    while quote.last().is_some_and(|line| line.is_empty()) {
        quote.pop();
    }

    if quote.is_empty() {
        return;
    }

    blocks.push(HoverMarkdownBlock::Quote(std::mem::take(quote)));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodeFenceKind {
    Backtick,
    Tilde,
}

impl CodeFenceKind {
    fn marker_byte(self) -> u8 {
        match self {
            Self::Backtick => b'`',
            Self::Tilde => b'~',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodeFence<'a> {
    kind: CodeFenceKind,
    marker_len: usize,
    language: Option<Cow<'a, str>>,
}

fn code_fence(line: &str) -> Option<CodeFence<'_>> {
    let trimmed = line.trim_start();
    let first = *trimmed.as_bytes().first()?;
    let kind = match first {
        b'`' => CodeFenceKind::Backtick,
        b'~' => CodeFenceKind::Tilde,
        _ => return None,
    };
    let marker_len = trimmed
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == first)
        .count();
    if marker_len < 3 {
        return None;
    }
    let suffix = &trimmed[marker_len..];
    let language = suffix
        .split_whitespace()
        .next()
        .filter(|part| !part.is_empty())
        .map(sanitize_language_label)
        .filter(|label| !label.is_empty());
    Some(CodeFence {
        kind,
        marker_len,
        language,
    })
}

fn is_code_fence(line: &str, kind: CodeFenceKind, opening_marker_len: usize) -> bool {
    let trimmed = line.trim_start();
    let marker = kind.marker_byte();
    let marker_len = trimmed
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == marker)
        .count();

    marker_len >= opening_marker_len && trimmed[marker_len..].trim().is_empty()
}

fn push_code_block_line(text: &mut String, pending_blank_lines: &mut String, line: &str) {
    if line.trim().is_empty() {
        if !text.is_empty() {
            pending_blank_lines.push('\n');
            pending_blank_lines.push_str(line);
        }
        return;
    }

    if !text.is_empty() {
        if !pending_blank_lines.is_empty() {
            text.push_str(pending_blank_lines);
            pending_blank_lines.clear();
            text.push('\n');
        } else {
            text.push('\n');
        }
    }
    text.push_str(line);
}

fn markdown_list_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let item = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
        .or_else(|| markdown_numbered_list_item(trimmed))?;
    let item = item.trim();
    (!item.is_empty()).then(|| clean_markdown_text(item).into_owned())
}

fn markdown_numbered_list_item(line: &str) -> Option<&str> {
    let marker_end = line.find(". ")?;
    (marker_end > 0 && marker_end <= 4 && line[..marker_end].chars().all(|ch| ch.is_ascii_digit()))
        .then_some(&line[marker_end + 2..])
}

fn markdown_list_continuation(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

fn markdown_quote_line(line: &str) -> Option<String> {
    let item = line.trim_start().strip_prefix('>')?.trim_start();
    Some(clean_markdown_text(item).into_owned())
}

fn markdown_heading_line(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    let suffix = &trimmed[level..];
    if !(1..=6).contains(&level) || !matches!(suffix.chars().next(), Some(ch) if ch.is_whitespace())
    {
        return None;
    }

    let text = clean_heading_text(suffix);
    (!text.is_empty()).then_some((level, text))
}

fn clean_heading_text(text: &str) -> String {
    let text = text.trim();
    let text = text.trim_end_matches('#').trim_end();
    clean_markdown_text(text).into_owned()
}

fn clean_markdown_text(text: &str) -> Cow<'_, str> {
    let Some(first_control) = text.find(|ch| !markdown_text_char_allowed(ch)) else {
        return Cow::Borrowed(text);
    };

    let mut cleaned = String::with_capacity(text.len());
    cleaned.push_str(&text[..first_control]);
    cleaned.extend(
        text[first_control..]
            .chars()
            .filter(|ch| markdown_text_char_allowed(*ch)),
    );
    Cow::Owned(cleaned)
}

fn markdown_text_char_allowed(ch: char) -> bool {
    !ch.is_control() && !matches!(ch, '\u{2028}' | '\u{2029}') && !is_bidi_format_control(ch)
}

fn is_bidi_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
    )
}

fn sanitize_language_label(label: &str) -> Cow<'_, str> {
    if label.len() <= 32
        && label
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'+'))
    {
        return Cow::Borrowed(label);
    }

    Cow::Owned(
        label
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+'))
            .take(32)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE, HoverMarkdownBlock, MAX_HOVER_MARKDOWN_PARSE_CHARS,
        MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS, parse_hover_markdown,
    };

    #[test]
    fn hover_markdown_parser_splits_paragraphs_and_code_fences() {
        let blocks = parse_hover_markdown(
            "HashMap stores keys.\n\n```rust\n    use std::collections::HashMap;\n```\n\nMore text.",
        );

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Paragraph("HashMap stores keys.".to_owned()),
                HoverMarkdownBlock::Code {
                    language: Some("rust".to_owned()),
                    text: "    use std::collections::HashMap;".to_owned(),
                },
                HoverMarkdownBlock::Paragraph("More text.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_supports_tilde_code_fences() {
        let blocks = parse_hover_markdown("Text.\n\n~~~rust\nfn main() {}\n~~~\n\nDone.");

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Paragraph("Text.".to_owned()),
                HoverMarkdownBlock::Code {
                    language: Some("rust".to_owned()),
                    text: "fn main() {}".to_owned(),
                },
                HoverMarkdownBlock::Paragraph("Done.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_matches_closing_fence_kind() {
        let blocks = parse_hover_markdown("~~~rust\nlet marker = \"```\";\n~~~");

        assert_eq!(
            blocks,
            vec![HoverMarkdownBlock::Code {
                language: Some("rust".to_owned()),
                text: "let marker = \"```\";".to_owned(),
            }]
        );
    }

    #[test]
    fn hover_markdown_parser_requires_closing_fence_to_match_opening_length() {
        let blocks =
            parse_hover_markdown("````rust\nlet marker = \"```\";\n```\nstill code\n````\n\nDone.");

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Code {
                    language: Some("rust".to_owned()),
                    text: "let marker = \"```\";\n```\nstill code".to_owned(),
                },
                HoverMarkdownBlock::Paragraph("Done.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_requires_closing_fence_to_have_only_whitespace_suffix() {
        let blocks = parse_hover_markdown("```rust\n```not closing\nstill code\n```   \n\nDone.");

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Code {
                    language: Some("rust".to_owned()),
                    text: "```not closing\nstill code".to_owned(),
                },
                HoverMarkdownBlock::Paragraph("Done.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_preserves_internal_blank_code_lines() {
        let blocks = parse_hover_markdown("```rust\n\nfn main() {}\n   \nprintln!();\n\t\n```");

        assert_eq!(
            blocks,
            vec![HoverMarkdownBlock::Code {
                language: Some("rust".to_owned()),
                text: "fn main() {}\n   \nprintln!();".to_owned(),
            }]
        );
    }

    #[test]
    fn hover_markdown_parser_supports_long_tilde_fences() {
        let blocks = parse_hover_markdown("~~~~text\n~~~\nbody\n~~~~");

        assert_eq!(
            blocks,
            vec![HoverMarkdownBlock::Code {
                language: Some("text".to_owned()),
                text: "~~~\nbody".to_owned(),
            }]
        );
    }

    #[test]
    fn hover_markdown_parser_preserves_bullet_lists() {
        let blocks = parse_hover_markdown(
            "Build options:\n\n- Fast path\n- Stable fallback\n  with details\n\nDone.",
        );

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Paragraph("Build options:".to_owned()),
                HoverMarkdownBlock::List(vec![
                    "Fast path".to_owned(),
                    "Stable fallback with details".to_owned(),
                ]),
                HoverMarkdownBlock::Paragraph("Done.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_preserves_numbered_lists() {
        let blocks = parse_hover_markdown("1. Prepare\n2. Run\n\nText.");

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::List(vec!["Prepare".to_owned(), "Run".to_owned()]),
                HoverMarkdownBlock::Paragraph("Text.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_preserves_block_quotes() {
        let blocks =
            parse_hover_markdown("Summary.\n\n> Important note\n> Continued detail\n\nDone.");

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Paragraph("Summary.".to_owned()),
                HoverMarkdownBlock::Quote(vec![
                    "Important note".to_owned(),
                    "Continued detail".to_owned(),
                ]),
                HoverMarkdownBlock::Paragraph("Done.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_preserves_headings() {
        let blocks = parse_hover_markdown("### Safety ###\n\nCall only after init.");

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Heading {
                    level: 3,
                    text: "Safety".to_owned(),
                },
                HoverMarkdownBlock::Paragraph("Call only after init.".to_owned()),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_strips_control_characters_from_text() {
        let blocks = parse_hover_markdown("### Use \u{0}clean\u{7} text\n\n- Item\u{1}");

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Heading {
                    level: 3,
                    text: "Use clean text".to_owned(),
                },
                HoverMarkdownBlock::List(vec!["Item".to_owned()]),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_sanitizes_paragraphs_and_continuations() {
        let blocks = parse_hover_markdown(
            "Use \u{202e}clean\u{0} text\ncontinued\u{7}\u{200b}\n\n- Item\n  cont\u{202d}inued\u{2060}\n\n> Quote\u{202a}line\u{feff}",
        );

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Paragraph("Use clean text continued".to_owned()),
                HoverMarkdownBlock::List(vec!["Item continued".to_owned()]),
                HoverMarkdownBlock::Quote(vec!["Quoteline".to_owned()]),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_strips_unicode_line_separators_from_text() {
        let blocks = parse_hover_markdown(
            "Before\u{2028}after\n\n- Item\u{2029}tail\n\n> Quote\u{2028}tail",
        );

        assert_eq!(
            blocks,
            vec![
                HoverMarkdownBlock::Paragraph("Beforeafter".to_owned()),
                HoverMarkdownBlock::List(vec!["Itemtail".to_owned()]),
                HoverMarkdownBlock::Quote(vec!["Quotetail".to_owned()]),
            ]
        );
    }

    #[test]
    fn hover_markdown_parser_sanitizes_fence_languages() {
        let blocks = parse_hover_markdown("```rust,<script>\nfn main() {}\n```");

        assert_eq!(
            blocks,
            vec![HoverMarkdownBlock::Code {
                language: Some("rust".to_owned()),
                text: "fn main() {}".to_owned(),
            }]
        );
    }

    #[test]
    fn hover_markdown_parser_caps_direct_oversized_input_on_utf8_boundary() {
        let input = format!(
            "{}tail",
            "\u{03b1}".repeat(MAX_HOVER_MARKDOWN_PARSE_CHARS + 4)
        );

        let blocks = parse_hover_markdown(&input);

        assert_eq!(blocks.len(), 2);
        let HoverMarkdownBlock::Paragraph(paragraph) = &blocks[0] else {
            panic!("expected capped paragraph");
        };
        assert_eq!(
            paragraph.matches('\u{03b1}').count(),
            MAX_HOVER_MARKDOWN_PARSE_CHARS
        );
        assert!(!paragraph.contains("tail"));
        assert_eq!(
            blocks.last(),
            Some(&HoverMarkdownBlock::Paragraph(
                HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE.to_owned()
            ))
        );
    }

    #[test]
    fn hover_markdown_parser_does_not_duplicate_existing_parse_truncation_notice() {
        let padding_len =
            MAX_HOVER_MARKDOWN_PARSE_CHARS - HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE.len() - 2;
        let input = format!(
            "{}\n\n{}tail",
            "a".repeat(padding_len),
            HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE
        );

        let blocks = parse_hover_markdown(&input);

        assert_eq!(
            blocks.last(),
            Some(&HoverMarkdownBlock::Paragraph(
                HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE.to_owned()
            ))
        );
        assert_eq!(
            blocks
                .iter()
                .filter(|block| {
                    matches!(
                        block,
                        HoverMarkdownBlock::Paragraph(text)
                            if text == HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE
                    )
                })
                .count(),
            1
        );
    }

    #[test]
    fn hover_markdown_parser_caps_pathological_list_rows() {
        let input = (0..MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS + 20)
            .map(|idx| format!("- item {idx}"))
            .collect::<Vec<_>>()
            .join("\n");

        let blocks = parse_hover_markdown(&input);

        assert_eq!(blocks.len(), 2);
        let HoverMarkdownBlock::List(items) = &blocks[0] else {
            panic!("expected capped list block");
        };
        let last_visible_item = format!("item {}", MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS - 1);
        let first_dropped_item = format!("item {MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS}");
        assert_eq!(items.len(), MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS);
        assert_eq!(
            items.last().map(String::as_str),
            Some(last_visible_item.as_str())
        );
        assert!(!items.iter().any(|item| item == &first_dropped_item));
        assert_eq!(
            blocks.last(),
            Some(&HoverMarkdownBlock::Paragraph(
                HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE.to_owned()
            ))
        );
    }

    #[test]
    fn hover_markdown_parser_caps_pathological_block_rows() {
        let input = (0..MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS + 20)
            .map(|idx| format!("paragraph {idx}"))
            .collect::<Vec<_>>()
            .join("\n\n");

        let blocks = parse_hover_markdown(&input);
        let last_visible_paragraph =
            format!("paragraph {}", MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS - 1);
        let first_dropped_paragraph = format!("paragraph {MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS}");
        let content_blocks = blocks
            .iter()
            .filter(|block| {
                !matches!(
                    block,
                    HoverMarkdownBlock::Paragraph(text)
                        if text == HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(content_blocks.len(), MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS);
        assert!(matches!(
            content_blocks.last(),
            Some(HoverMarkdownBlock::Paragraph(text)) if text == &last_visible_paragraph
        ));
        assert!(!blocks.iter().any(|block| {
            matches!(
                block,
                HoverMarkdownBlock::Paragraph(text)
                    if text == &first_dropped_paragraph
            )
        }));
        assert_eq!(
            blocks.last(),
            Some(&HoverMarkdownBlock::Paragraph(
                HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE.to_owned()
            ))
        );
    }

    #[test]
    fn hover_markdown_parser_caps_pathological_code_rows() {
        let code = (0..MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS + 20)
            .map(|idx| format!("line {idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        let input = format!("```text\n{code}\n```");

        let blocks = parse_hover_markdown(&input);

        assert_eq!(blocks.len(), 2);
        let HoverMarkdownBlock::Code { text, .. } = &blocks[0] else {
            panic!("expected capped code block");
        };
        let expected_last_line = format!("line {}", MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS - 1);
        assert_eq!(text.lines().count(), MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS);
        assert_eq!(text.lines().last(), Some(expected_last_line.as_str()));
        assert!(!text.contains(&format!("line {MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS}")));
        assert_eq!(
            blocks.last(),
            Some(&HoverMarkdownBlock::Paragraph(
                HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE.to_owned()
            ))
        );
    }

    #[test]
    fn hover_markdown_parser_preserves_existing_truncation_notice_when_capping_code_rows() {
        let code = (0..MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS + 20)
            .map(|idx| format!("line {idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        let input = format!("```text\n{code}\n```\n\n[Documentation truncated]");

        let blocks = parse_hover_markdown(&input);

        assert_eq!(blocks.len(), 2);
        let HoverMarkdownBlock::Code { text, .. } = &blocks[0] else {
            panic!("expected capped code block");
        };
        assert_eq!(
            text.lines().count(),
            MAX_HOVER_MARKDOWN_RENDER_CONTENT_ROWS - 1
        );
        assert_eq!(
            blocks.last(),
            Some(&HoverMarkdownBlock::Paragraph(
                "[Documentation truncated]".to_owned()
            ))
        );
        assert!(!blocks.iter().any(|block| {
            matches!(
                block,
                HoverMarkdownBlock::Paragraph(text)
                    if text == HOVER_MARKDOWN_PARSE_TRUNCATED_NOTICE
            )
        }));
    }
}
