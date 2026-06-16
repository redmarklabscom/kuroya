use super::{
    MAX_SYNTAX_TREE_CACHES, SYNTAX_TREE_MAX_BYTES, SYNTAX_TREE_MAX_LINES, SyntaxTreeCacheEntry,
    SyntaxTreeCacheKey, SyntaxTreeUnavailableReason, TREE_SITTER_FOLDING_CANDIDATE_LIMIT,
    TreeSitterByteInjection, TreeSitterInjection, TreeSitterSyntaxCache,
    collect_rust_comment_ranges, collect_rust_use_declaration_ranges, folding_ranges_fit_buffer,
    rust_tree_sitter_language, syntax_injections_fit_buffer, syntax_tree_cache_key,
    syntax_tree_input_edit, syntax_tree_unavailable_reason, tree_sitter_adapter_for_language,
    tree_sitter_injections_for_entry,
};
use crate::large_file_mode::LARGE_FILE_MODE_MAX_BYTES;
use kuroya_core::{LanguageId, LspFoldingRange, TextBuffer, TextEdit};
use std::path::PathBuf;
use tree_sitter::{Parser, Tree};

#[test]
fn rust_tree_sitter_folding_ranges_cover_nested_items() {
    let buffer = TextBuffer::from_text_with_language(
            7,
            None,
            "impl App {\n    pub fn run(&self) {\n        match self.state {\n            State::Ready => {}\n            State::Idle => {}\n        }\n    }\n}\n"
                .to_owned(),
            LanguageId::Rust,
        );
    let mut cache = TreeSitterSyntaxCache::default();

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 1 && range.end_line == 8)
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 2 && range.end_line == 7)
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 3 && range.end_line == 6)
    );
}

#[test]
fn rust_tree_sitter_folding_ranges_group_contiguous_imports() {
    let buffer = TextBuffer::from_text_with_language(
        8,
        None,
        "use std::fs;\nuse std::path::Path;\n\nuse crate::single;\n\nfn main() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(ranges.iter().any(|range| {
        range.start_line == 1 && range.end_line == 2 && range.kind.as_deref() == Some("imports")
    }));
    assert!(!ranges.iter().any(|range| {
        range.start_line == 4 && range.end_line == 4 && range.kind.as_deref() == Some("imports")
    }));
}

#[test]
fn rust_tree_sitter_folding_ranges_cover_comment_blocks() {
    let buffer = TextBuffer::from_text_with_language(
            9,
            None,
            "fn main() {\n    // first\n    // second\n    let value = 1;\n    /*\n     * block\n     */\n}\n"
                .to_owned(),
            LanguageId::Rust,
        );
    let mut cache = TreeSitterSyntaxCache::default();

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(ranges.iter().any(|range| {
        range.start_line == 2 && range.end_line == 3 && range.kind.as_deref() == Some("comment")
    }));
    assert!(ranges.iter().any(|range| {
        range.start_line == 5 && range.end_line == 7 && range.kind.as_deref() == Some("comment")
    }));
    assert!(!ranges.iter().any(|range| {
        range.start_line == 2 && range.end_line == 2 && range.kind.as_deref() == Some("comment")
    }));
}

#[test]
fn rust_tree_sitter_folding_ranges_cover_macro_invocations_and_where_clauses() {
    let buffer = TextBuffer::from_text_with_language(
            10,
            None,
            "fn run<T>(value: T)\nwhere\n    T: Clone\n        + Send,\n{\n    trace!(\n        \"value: {:?}\",\n        value,\n    );\n}\n"
            .to_owned(),
            LanguageId::Rust,
        );
    let mut cache = TreeSitterSyntaxCache::default();

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 2 && range.end_line == 4)
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 6 && range.end_line == 9)
    );
}

#[test]
fn rust_tree_sitter_folding_ranges_cover_multiline_literal_expressions() {
    let buffer = TextBuffer::from_text_with_language(
            11,
            None,
            "fn run() {\n    let values = [\n        1,\n        2,\n    ];\n\n    let pair = (\n        \"left\",\n        \"right\",\n    );\n}\n"
                .to_owned(),
            LanguageId::Rust,
        );
    let mut cache = TreeSitterSyntaxCache::default();

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 2 && range.end_line == 5)
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 7 && range.end_line == 10)
    );
}

#[test]
fn rust_tree_sitter_folding_ranges_cover_struct_literals_and_generics() {
    let buffer = TextBuffer::from_text_with_language(
            55,
            None,
            "fn build<\n    T: Clone,\n    U,\n>(\n    value: Widget<\n        T,\n        U,\n    >,\n) {\n    let item = Widget {\n        first: value,\n        second: None,\n    };\n}\n"
                .to_owned(),
            LanguageId::Rust,
        );
    let mut cache = TreeSitterSyntaxCache::default();

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 1 && range.end_line == 4),
        "expected multiline type parameters to fold: {ranges:?}"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 5 && range.end_line == 8),
        "expected multiline type arguments to fold: {ranges:?}"
    );
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 10 && range.end_line == 13),
        "expected struct literal fields to fold: {ranges:?}"
    );
}

#[test]
fn rust_folding_import_collection_stops_at_candidate_limit() {
    let text = (0..TREE_SITTER_FOLDING_CANDIDATE_LIMIT + 50)
        .map(|index| format!("use crate::module_{index};"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = parse_rust_tree(&text);
    let mut declarations = Vec::new();

    collect_rust_use_declaration_ranges(tree.root_node(), &mut declarations);

    assert_eq!(declarations.len(), TREE_SITTER_FOLDING_CANDIDATE_LIMIT);
}

#[test]
fn rust_folding_comment_collection_stops_at_candidate_limit() {
    let text = (0..TREE_SITTER_FOLDING_CANDIDATE_LIMIT + 50)
        .map(|index| format!("// comment {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let tree = parse_rust_tree(&text);
    let mut comments = Vec::new();

    collect_rust_comment_ranges(tree.root_node(), &mut comments);

    assert_eq!(comments.len(), TREE_SITTER_FOLDING_CANDIDATE_LIMIT);
}

#[test]
fn tree_sitter_folding_cache_reuses_non_empty_ranges() {
    let buffer = TextBuffer::from_text_with_language(
        13,
        None,
        "fn run() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let key = syntax_tree_cache_key(&buffer).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    let first = cache.folding_ranges_for_buffer(&buffer).unwrap();
    let second = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(!first.is_empty());
    assert_eq!(first, second);
    assert_eq!(cache.folding_compute_count(), 1);
    assert!(cache.trees[&key].cached_folding_ranges.is_some());
}

#[test]
fn tree_sitter_folding_cache_stores_empty_ranges() {
    let buffer = TextBuffer::from_text_with_language(
        14,
        None,
        "fn main() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let key = syntax_tree_cache_key(&buffer).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.folding_ranges_for_buffer(&buffer).unwrap().is_empty());
    assert!(cache.folding_ranges_for_buffer(&buffer).unwrap().is_empty());

    assert_eq!(cache.folding_compute_count(), 1);
    assert_eq!(
        cache.trees[&key].cached_folding_ranges.as_ref(),
        Some(&Vec::new())
    );
}

#[test]
fn tree_sitter_folding_cache_recomputes_invalid_cached_ranges() {
    let buffer = TextBuffer::from_text_with_language(
        58,
        None,
        "fn run() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let key = syntax_tree_cache_key(&buffer).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(!cache.folding_ranges_for_buffer(&buffer).unwrap().is_empty());
    assert_eq!(cache.folding_compute_count(), 1);
    cache.trees.get_mut(&key).unwrap().cached_folding_ranges = Some(vec![LspFoldingRange {
        start_line: 2,
        start_column: None,
        end_line: buffer.len_lines() + 10,
        end_column: None,
        kind: Some("stale".to_owned()),
    }]);

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert_eq!(cache.folding_compute_count(), 2);
    assert!(folding_ranges_fit_buffer(&ranges, buffer.len_lines()));
    assert!(
        !ranges
            .iter()
            .any(|range| range.kind.as_deref() == Some("stale"))
    );
}

#[test]
fn tree_sitter_folding_cache_invalidates_after_buffer_version_changes() {
    let mut buffer = TextBuffer::from_text_with_language(
        15,
        None,
        "fn run() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();
    let old_key = syntax_tree_cache_key(&buffer).unwrap();

    assert!(!cache.folding_ranges_for_buffer(&buffer).unwrap().is_empty());
    assert_eq!(cache.folding_compute_count(), 1);

    buffer.replace_from_disk("fn main() {}\n".to_owned());
    let new_key = syntax_tree_cache_key(&buffer).unwrap();

    assert_ne!(old_key, new_key);
    assert!(cache.folding_ranges_for_buffer(&buffer).unwrap().is_empty());
    assert_eq!(cache.folding_compute_count(), 2);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(!cache.trees.contains_key(&old_key));
    assert_eq!(
        cache.trees[&new_key].cached_folding_ranges.as_ref(),
        Some(&Vec::new())
    );
}

#[test]
fn tree_sitter_folding_cache_survives_same_text_version_changes() {
    let text = "fn run() {\n    call();\n}\n".to_owned();
    let mut buffer = TextBuffer::from_text_with_language(51, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();
    let old_key = syntax_tree_cache_key(&buffer).unwrap();

    let first = cache.folding_ranges_for_buffer(&buffer).unwrap();
    assert!(!first.is_empty());
    assert_eq!(cache.folding_compute_count(), 1);

    buffer.replace_from_disk(text);
    let new_key = syntax_tree_cache_key(&buffer).unwrap();
    let second = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert_ne!(old_key, new_key);
    assert_eq!(first, second);
    assert_eq!(cache.folding_compute_count(), 1);
    assert_eq!(cache.incremental_reparse_count(), 0);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(!cache.trees.contains_key(&old_key));
    assert!(cache.trees[&new_key].cached_folding_ranges.is_some());
}

#[test]
fn tree_sitter_cache_discards_same_key_entries_with_mismatched_text() {
    let buffer = TextBuffer::from_text_with_language(
        16,
        None,
        "fn run() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let key = syntax_tree_cache_key(&buffer).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();
    let stale_range = LspFoldingRange {
        start_line: 1,
        start_column: None,
        end_line: 2,
        end_column: None,
        kind: Some("stale".to_owned()),
    };

    assert!(!cache.folding_ranges_for_buffer(&buffer).unwrap().is_empty());
    {
        let entry = cache.trees.get_mut(&key).unwrap();
        entry.text = "fn stale() {}\n".to_owned();
        entry.cached_folding_ranges = Some(vec![stale_range.clone()]);
    }

    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert!(!ranges.contains(&stale_range));
    assert_eq!(cache.folding_compute_count(), 2);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(buffer.text_equals(&cache.trees[&key].text));
    assert_eq!(
        cache
            .order
            .iter()
            .filter(|cached_key| **cached_key == key)
            .count(),
        1
    );
}

#[test]
fn tree_sitter_cache_reuses_buffer_version_and_replaces_stale_versions() {
    let mut buffer = TextBuffer::from_text_with_language(
        12,
        None,
        "fn first() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    let first = cache.folding_ranges_for_buffer(&buffer).unwrap();
    let second = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert_eq!(first, second);
    assert_eq!(cache.cached_tree_count(), 1);

    buffer.replace_from_disk("fn second() {\n    call();\n}\n".to_owned());
    let _ = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert_eq!(cache.cached_tree_count(), 1);
}

#[test]
fn tree_sitter_cache_rejects_same_id_version_text_shape_changes() {
    let first = TextBuffer::from_text_with_language(
        59,
        None,
        "fn first() {\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let second = TextBuffer::from_text_with_language(
        59,
        None,
        "fn second() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let first_key = syntax_tree_cache_key(&first).unwrap();
    let second_key = syntax_tree_cache_key(&second).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    let _ = cache.folding_ranges_for_buffer(&first).unwrap();
    let ranges = cache.folding_ranges_for_buffer(&second).unwrap();

    assert_ne!(first_key, second_key);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(!cache.trees.contains_key(&first_key));
    assert!(cache.trees.contains_key(&second_key));
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 1 && range.end_line == 3)
    );
}

#[test]
fn tree_sitter_cache_insert_replaces_stale_order_key_once() {
    let mut first_buffer = TextBuffer::from_text_with_language(
        56,
        None,
        "fn first() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let second_buffer = TextBuffer::from_text_with_language(
        57,
        None,
        "fn second() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    let _ = cache.folding_ranges_for_buffer(&first_buffer).unwrap();
    let _ = cache.folding_ranges_for_buffer(&second_buffer).unwrap();
    let first_old_key = syntax_tree_cache_key(&first_buffer).unwrap();
    let second_key = syntax_tree_cache_key(&second_buffer).unwrap();

    assert_eq!(
        cache.order.iter().copied().collect::<Vec<_>>(),
        vec![first_old_key, second_key]
    );

    first_buffer.replace_from_disk("fn first() {\n    call();\n}\n".to_owned());
    let first_new_key = syntax_tree_cache_key(&first_buffer).unwrap();
    let _ = cache.folding_ranges_for_buffer(&first_buffer).unwrap();

    assert_eq!(
        cache.order.iter().copied().collect::<Vec<_>>(),
        vec![second_key, first_new_key]
    );
    assert!(!cache.trees.contains_key(&first_old_key));
    assert!(cache.trees.contains_key(&first_new_key));
}

#[test]
fn tree_sitter_cache_incrementally_reparses_next_buffer_version() {
    let text = "fn run() {\n    call();\n}\n".to_owned();
    let mut buffer = TextBuffer::from_text_with_language(12, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();
    let _ = cache.folding_ranges_for_buffer(&buffer).unwrap();
    let call_start = char_index_of(&text, "call();");

    buffer.apply_edit(TextEdit {
        range: call_start..call_start,
        inserted: "let value = 1;\n    ".to_owned(),
    });
    let ranges = cache.folding_ranges_for_buffer(&buffer).unwrap();

    assert_eq!(cache.incremental_reparse_count(), 1);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(
        ranges
            .iter()
            .any(|range| range.start_line == 1 && range.end_line == 4)
    );
}

#[test]
fn tree_sitter_cache_does_not_reparse_incrementally_from_newer_entry() {
    let mut old_buffer = TextBuffer::from_text_with_language(
        52,
        None,
        "fn old() {\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    let _ = cache.folding_ranges_for_buffer(&old_buffer).unwrap();
    old_buffer.apply_edit(TextEdit {
        range: 0..0,
        inserted: "// edited\n".to_owned(),
    });
    let _ = cache.folding_ranges_for_buffer(&old_buffer).unwrap();
    assert_eq!(cache.incremental_reparse_count(), 1);

    let fresh_buffer = TextBuffer::from_text_with_language(
        52,
        None,
        "fn fresh() {\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let fresh_key = syntax_tree_cache_key(&fresh_buffer).unwrap();

    let _ = cache.folding_ranges_for_buffer(&fresh_buffer).unwrap();

    assert_eq!(cache.incremental_reparse_count(), 1);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(cache.trees.contains_key(&fresh_key));
}

#[test]
fn tree_sitter_cache_hit_refreshes_lru_order() {
    let buffers = (0..=(MAX_SYNTAX_TREE_CACHES as u64))
        .map(|id| {
            TextBuffer::from_text_with_language(
                100 + id,
                None,
                format!("fn run_{id}() {{\n}}\n"),
                LanguageId::Rust,
            )
        })
        .collect::<Vec<_>>();
    let first_key = syntax_tree_cache_key(&buffers[0]).unwrap();
    let second_key = syntax_tree_cache_key(&buffers[1]).unwrap();
    let newest_key = syntax_tree_cache_key(buffers.last().unwrap()).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    for buffer in buffers.iter().take(MAX_SYNTAX_TREE_CACHES) {
        let _ = cache.folding_ranges_for_buffer(buffer).unwrap();
    }
    assert_eq!(cache.cached_tree_count(), MAX_SYNTAX_TREE_CACHES);

    let _ = cache.folding_ranges_for_buffer(&buffers[0]).unwrap();
    let _ = cache
        .folding_ranges_for_buffer(buffers.last().unwrap())
        .unwrap();

    assert_eq!(cache.cached_tree_count(), MAX_SYNTAX_TREE_CACHES);
    assert!(cache.trees.contains_key(&first_key));
    assert!(!cache.trees.contains_key(&second_key));
    assert!(cache.trees.contains_key(&newest_key));
}

#[test]
fn tree_sitter_selection_expansion_rejects_reversed_ranges_without_parsing() {
    let buffer = TextBuffer::from_text_with_language(
        53,
        None,
        "fn run() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();
    let reversed_start = 8;
    let reversed_end = 4;

    assert!(
        cache
            .selection_expansion_for_buffer(&buffer, reversed_start..reversed_end)
            .is_none()
    );
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn tree_sitter_selection_expansion_rejects_out_of_bounds_ranges_without_parsing() {
    let buffer = TextBuffer::from_text_with_language(
        60,
        None,
        "fn run() {\n    call();\n}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();
    let out_of_bounds = buffer.len_chars() + 1;

    assert!(
        cache
            .selection_expansion_for_buffer(&buffer, out_of_bounds..out_of_bounds)
            .is_none()
    );
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn syntax_tree_input_edit_tracks_multiline_unicode_replacements() {
    let edit = syntax_tree_input_edit(
        "fn run() {\n    café();\n}\n",
        "fn run() {\n    cafe();\n}\n",
    )
    .expect("replacement should produce a tree-sitter input edit");

    assert_eq!(edit.start_position.row, 1);
    assert_eq!(edit.old_end_position.row, 1);
    assert_eq!(edit.new_end_position.row, 1);
    assert!(edit.start_byte < edit.old_end_byte);
    assert!(edit.start_byte < edit.new_end_byte);
}

#[test]
fn tree_sitter_cache_skips_unsupported_languages() {
    let buffer = TextBuffer::from_text_with_language(
        3,
        None,
        "def run():\n    pass\n".to_owned(),
        LanguageId::Python,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.folding_ranges_for_buffer(&buffer).is_none());
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn tree_sitter_language_adapter_registry_supports_rust_only() {
    assert!(tree_sitter_adapter_for_language(LanguageId::Rust).is_some());
    assert!(tree_sitter_adapter_for_language(LanguageId::Python).is_none());
}

#[test]
fn syntax_tree_unavailable_reason_names_unsupported_languages() {
    let buffer = TextBuffer::from_text_with_language(
        3,
        None,
        "def run():\n    pass\n".to_owned(),
        LanguageId::Python,
    );

    assert_eq!(
        syntax_tree_unavailable_reason(&buffer),
        Some(SyntaxTreeUnavailableReason::UnsupportedLanguage(
            LanguageId::Python
        ))
    );
}

#[test]
fn syntax_tree_unavailable_reason_keeps_unsupported_language_before_size_limits() {
    let buffer = TextBuffer::from_text_with_language(
        4,
        None,
        "x".repeat(LARGE_FILE_MODE_MAX_BYTES + 1),
        LanguageId::Python,
    );

    assert_eq!(
        syntax_tree_unavailable_reason(&buffer),
        Some(SyntaxTreeUnavailableReason::UnsupportedLanguage(
            LanguageId::Python
        ))
    );
}

#[test]
fn syntax_tree_unavailable_reason_names_sync_parse_byte_budget() {
    let buffer = TextBuffer::from_text_with_language(
        4,
        None,
        "x".repeat(SYNTAX_TREE_MAX_BYTES + 1),
        LanguageId::Rust,
    );

    assert_eq!(
        syntax_tree_unavailable_reason(&buffer),
        Some(SyntaxTreeUnavailableReason::ParseByteBudget {
            bytes: SYNTAX_TREE_MAX_BYTES + 1,
            max_bytes: SYNTAX_TREE_MAX_BYTES,
        })
    );
}

#[test]
fn syntax_tree_unavailable_reason_names_sync_parse_line_budget() {
    let text = std::iter::repeat_n("x", SYNTAX_TREE_MAX_LINES + 1)
        .collect::<Vec<_>>()
        .join("\n");
    let buffer = TextBuffer::from_text_with_language(5, None, text, LanguageId::Rust);

    assert_eq!(
        syntax_tree_unavailable_reason(&buffer),
        Some(SyntaxTreeUnavailableReason::ParseLineBudget {
            lines: SYNTAX_TREE_MAX_LINES + 1,
            max_lines: SYNTAX_TREE_MAX_LINES,
        })
    );
}

#[test]
fn syntax_tree_unavailable_reason_names_large_file_mode_before_parse_budget() {
    let buffer = TextBuffer::from_text_with_language(
        6,
        None,
        "x".repeat(LARGE_FILE_MODE_MAX_BYTES + 1),
        LanguageId::Rust,
    );

    assert_eq!(
        syntax_tree_unavailable_reason(&buffer),
        Some(SyntaxTreeUnavailableReason::LargeFileMode)
    );
}

#[test]
fn rust_tree_sitter_expands_selection_through_ast_nodes() {
    let text = "fn run() {\n    let value = compute(1 + 2);\n}\n".to_owned();
    let buffer = TextBuffer::from_text_with_language(19, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();
    let compute_start = char_index_of(&text, "compute");

    let identifier = cache
        .selection_expansion_for_buffer(&buffer, compute_start + 1..compute_start + 1)
        .expect("caret inside identifier should expand to a syntax node");
    assert_eq!(text_for_range(&text, identifier.clone()), "compute");

    let call = cache
        .selection_expansion_for_buffer(&buffer, identifier)
        .expect("identifier should expand to its call expression");
    assert_eq!(text_for_range(&text, call.clone()), "compute(1 + 2)");

    let statement = cache
        .selection_expansion_for_buffer(&buffer, call)
        .expect("call expression should expand to its statement");
    assert_eq!(
        text_for_range(&text, statement),
        "let value = compute(1 + 2);"
    );
}

#[test]
fn rust_tree_sitter_expands_selection_from_identifier_boundary() {
    let text = "fn run() {\n    let value = compute(1 + 2);\n}\n".to_owned();
    let buffer = TextBuffer::from_text_with_language(54, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();
    let compute_start = char_index_of(&text, "compute");
    let compute_end = compute_start + "compute".chars().count();

    let identifier = cache
        .selection_expansion_for_buffer(&buffer, compute_end..compute_end)
        .expect("caret after identifier should expand to the preceding syntax node");
    assert_eq!(text_for_range(&text, identifier.clone()), "compute");

    let call = cache
        .selection_expansion_for_buffer(&buffer, identifier)
        .expect("identifier should still expand to its call expression");
    assert_eq!(text_for_range(&text, call), "compute(1 + 2)");
}

#[test]
fn rust_tree_sitter_newline_indent_uses_enclosing_block() {
    let text = "fn run() {\nlet value = 1;\n}\n".to_owned();
    let mut buffer = TextBuffer::from_text_with_language(31, None, text.clone(), LanguageId::Rust);
    buffer.set_single_cursor(buffer.line_content_end_char(1));
    let mut cache = TreeSitterSyntaxCache::default();

    let overrides = cache
        .newline_indent_overrides_for_buffer(&buffer, "    ")
        .expect("underindented Rust block line should get a syntax indent override");

    assert_eq!(overrides, vec![Some("    ".to_owned())]);
}

#[test]
fn rust_tree_sitter_newline_indent_keeps_deeper_manual_indent() {
    let text = "fn run() {\n        let value = 1;\n}\n".to_owned();
    let mut buffer = TextBuffer::from_text_with_language(37, None, text.clone(), LanguageId::Rust);
    buffer.set_single_cursor(buffer.line_content_end_char(1));
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(
        cache
            .newline_indent_overrides_for_buffer(&buffer, "    ")
            .is_none()
    );
}

#[test]
fn rust_tree_sitter_newline_indent_after_match_arm_arrow() {
    let text =
        "fn run(state: State) {\n    match state {\n        State::Ready =>\n    }\n}\n".to_owned();
    let mut buffer = TextBuffer::from_text_with_language(38, None, text.clone(), LanguageId::Rust);
    buffer.set_single_cursor(buffer.line_content_end_char(2));
    let mut cache = TreeSitterSyntaxCache::default();

    let overrides = cache
        .newline_indent_overrides_for_buffer(&buffer, "    ")
        .expect("match arm arrows should indent the expression line");

    assert_eq!(overrides, vec![Some("            ".to_owned())]);
}

#[test]
fn rust_tree_sitter_newline_indent_ignores_macro_arrow() {
    let text = "macro_rules! ready {\n    ($value:expr) =>\n}\n".to_owned();
    let mut buffer = TextBuffer::from_text_with_language(39, None, text.clone(), LanguageId::Rust);
    buffer.set_single_cursor(buffer.line_content_end_char(1));
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(
        cache
            .newline_indent_overrides_for_buffer(&buffer, "    ")
            .is_none()
    );
}

#[test]
fn rust_tree_sitter_injections_follow_language_comments() {
    let text = r##"fn main() {
    let emoji = "🙂";
    // language=json
    let config = r#"{"name":"kuroya"}"#;
    let query = /* lang: sql */ "select * from users";
}
"##
    .to_owned();
    let buffer = TextBuffer::from_text_with_language(41, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();

    let injections = cache
        .injections_for_buffer(&buffer)
        .expect("language comments should produce injected ranges");

    assert_eq!(injections.len(), 2);
    assert_eq!(injections[0].language, LanguageId::Json);
    let json = r#"{"name":"kuroya"}"#;
    assert_eq!(text_for_range(&text, injections[0].range.clone()), json);
    assert_eq!(injections[0].range.start, char_index_of(&text, json));
    assert_eq!(
        injections[0].range.end,
        injections[0].range.start + json.chars().count()
    );
    assert!(text.find(json).unwrap() > injections[0].range.start);
    assert_eq!(injections[1].language, LanguageId::Sql);
    assert_eq!(
        text_for_range(&text, injections[1].range.clone()),
        "select * from users"
    );
}

#[test]
fn tree_sitter_injection_cache_reuses_non_empty_results() {
    let text = r##"fn main() {
    // language=json
    let config = r#"{"name":"kuroya"}"#;
}
"##
    .to_owned();
    let buffer = TextBuffer::from_text_with_language(42, None, text.clone(), LanguageId::Rust);
    let key = syntax_tree_cache_key(&buffer).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    let first = cache
        .injections_for_buffer(&buffer)
        .expect("language comment should produce an injected JSON range");
    let second = cache
        .injections_for_buffer(&buffer)
        .expect("cached injected range should still be returned");

    assert_eq!(first, second);
    assert_eq!(cache.injection_compute_count(), 1);
    assert!(cache.trees[&key].cached_injections.is_some());
}

#[test]
fn tree_sitter_injection_cache_stores_empty_results() {
    let buffer = TextBuffer::from_text_with_language(
        46,
        None,
        "fn main() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let key = syntax_tree_cache_key(&buffer).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.injections_for_buffer(&buffer).is_none());
    assert!(cache.injections_for_buffer(&buffer).is_none());

    assert_eq!(cache.injection_compute_count(), 1);
    assert_eq!(
        cache.trees[&key].cached_injections.as_ref(),
        Some(&Vec::new())
    );
}

#[test]
fn tree_sitter_injection_cache_recomputes_invalid_cached_ranges() {
    let text = r##"fn main() {
    // language=json
    let config = r#"{"name":"kuroya"}"#;
}
"##
    .to_owned();
    let buffer = TextBuffer::from_text_with_language(61, None, text.clone(), LanguageId::Rust);
    let key = syntax_tree_cache_key(&buffer).unwrap();
    let mut cache = TreeSitterSyntaxCache::default();

    let first = cache
        .injections_for_buffer(&buffer)
        .expect("language comment should produce an injected JSON range");
    assert_eq!(cache.injection_compute_count(), 1);
    cache.trees.get_mut(&key).unwrap().cached_injections = Some(vec![TreeSitterInjection {
        language: LanguageId::Json,
        range: 0..buffer.len_chars() + 10,
    }]);

    let second = cache
        .injections_for_buffer(&buffer)
        .expect("invalid cached injection should be recomputed");

    assert_eq!(cache.injection_compute_count(), 2);
    assert_eq!(first, second);
    assert!(syntax_injections_fit_buffer(&second, buffer.len_chars()));
}

#[test]
fn tree_sitter_injections_drop_invalid_byte_ranges_before_char_conversion() {
    let text = "fn main() {\n    let caf\u{00e9} = 1;\n}\n".to_owned();
    let buffer = TextBuffer::from_text_with_language(62, None, text.clone(), LanguageId::Rust);
    let entry = SyntaxTreeCacheEntry::uncached(parse_rust_tree(&text), text.clone());
    let mut adapter = tree_sitter_adapter_for_language(LanguageId::Rust).unwrap();
    adapter.injections = test_byte_injections_with_invalid_ranges;
    let main_start = char_index_of(&text, "main");
    let main_end = main_start + "main".chars().count();

    let injections = tree_sitter_injections_for_entry(&buffer, adapter, &entry);

    assert_eq!(
        injections,
        vec![TreeSitterInjection {
            language: LanguageId::Json,
            range: main_start..main_end,
        }]
    );
}

#[test]
fn tree_sitter_injection_cache_invalidates_after_buffer_version_changes() {
    let text = r##"fn main() {
    // language=json
    let config = r#"{"name":"kuroya"}"#;
}
"##
    .to_owned();
    let mut buffer = TextBuffer::from_text_with_language(48, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();
    let old_key = syntax_tree_cache_key(&buffer).unwrap();

    assert!(cache.injections_for_buffer(&buffer).is_some());
    assert_eq!(cache.injection_compute_count(), 1);

    buffer.replace_from_disk(
        r##"fn main() {
    let config = r#"{}"#;
}
"##
        .to_owned(),
    );
    let new_key = syntax_tree_cache_key(&buffer).unwrap();

    assert_ne!(old_key, new_key);
    assert!(cache.injections_for_buffer(&buffer).is_none());
    assert_eq!(cache.injection_compute_count(), 2);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(!cache.trees.contains_key(&old_key));
    assert_eq!(
        cache.trees[&new_key].cached_injections.as_ref(),
        Some(&Vec::new())
    );
}

#[test]
fn tree_sitter_injection_cache_survives_same_text_version_changes() {
    let text = r##"fn main() {
    // language=json
    let config = r#"{"name":"kuroya"}"#;
}
"##
    .to_owned();
    let mut buffer = TextBuffer::from_text_with_language(52, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();
    let old_key = syntax_tree_cache_key(&buffer).unwrap();

    let first = cache
        .injections_for_buffer(&buffer)
        .expect("language comment should produce an injected JSON range");
    assert_eq!(cache.injection_compute_count(), 1);

    buffer.replace_from_disk(text);
    let new_key = syntax_tree_cache_key(&buffer).unwrap();
    let second = cache
        .injections_for_buffer(&buffer)
        .expect("cached injected range should still be returned");

    assert_ne!(old_key, new_key);
    assert_eq!(first, second);
    assert_eq!(cache.injection_compute_count(), 1);
    assert_eq!(cache.incremental_reparse_count(), 0);
    assert_eq!(cache.cached_tree_count(), 1);
    assert!(!cache.trees.contains_key(&old_key));
    assert!(cache.trees[&new_key].cached_injections.is_some());
}

#[test]
fn rust_tree_sitter_injections_detect_sqlx_query_macros() {
    let text = r#"fn run(id: i64) {
    let row = sqlx::query!("select * from users where id = $1", id);
}
"#
    .to_owned();
    let buffer = TextBuffer::from_text_with_language(43, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();

    let injections = cache
        .injections_for_buffer(&buffer)
        .expect("sqlx query macros should tag the query string as SQL");

    assert_eq!(injections.len(), 1);
    assert_eq!(injections[0].language, LanguageId::Sql);
    assert_eq!(
        text_for_range(&text, injections[0].range.clone()),
        "select * from users where id = $1"
    );
}

#[test]
fn rust_tree_sitter_injections_detect_doc_comment_code_fences() {
    let text = r#"/// Example:
/// ```json
/// {"name":"kuroya"}
/// ```
/**
 * ```python
 * print("hi")
 * ```
 */
fn documented() {}
"#
    .to_owned();
    let buffer = TextBuffer::from_text_with_language(44, None, text.clone(), LanguageId::Rust);
    let mut cache = TreeSitterSyntaxCache::default();

    let injections = cache
        .injections_for_buffer(&buffer)
        .expect("Rust doc comment code fences should produce injected ranges");

    assert_eq!(injections.len(), 2);
    assert_eq!(injections[0].language, LanguageId::Json);
    assert_eq!(
        text_for_range(&text, injections[0].range.clone()),
        r#"{"name":"kuroya"}"#
    );
    assert_eq!(injections[1].language, LanguageId::Python);
    assert_eq!(
        text_for_range(&text, injections[1].range.clone()),
        r#"print("hi")"#
    );
}

#[test]
fn tree_sitter_selection_expansion_skips_unsupported_languages() {
    let buffer = TextBuffer::from_text_with_language(
        23,
        None,
        "def run():\n    pass\n".to_owned(),
        LanguageId::Python,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(
        cache
            .selection_expansion_for_buffer(&buffer, 0..0)
            .is_none()
    );
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn tree_sitter_cache_skips_buffers_above_sync_parse_budget() {
    let buffer = TextBuffer::from_text_with_language(
        27,
        None,
        format!("fn main() {{}}\n// {}", "a".repeat(SYNTAX_TREE_MAX_BYTES)),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(buffer.len_bytes() > SYNTAX_TREE_MAX_BYTES);
    assert!(buffer.len_bytes() < LARGE_FILE_MODE_MAX_BYTES);
    assert!(cache.folding_ranges_for_buffer(&buffer).is_none());
    assert!(
        cache
            .selection_expansion_for_buffer(&buffer, 0..0)
            .is_none()
    );
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn tree_sitter_cache_prunes_buffer_when_it_exceeds_parse_budget() {
    let mut buffer = TextBuffer::from_text_with_language(
        47,
        None,
        "fn main() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.folding_ranges_for_buffer(&buffer).is_some());
    assert_eq!(cache.cached_tree_count(), 1);

    buffer.replace_from_disk(format!(
        "fn main() {{}}\n{}",
        "a".repeat(SYNTAX_TREE_MAX_BYTES)
    ));

    assert!(buffer.len_bytes() > SYNTAX_TREE_MAX_BYTES);
    assert!(cache.folding_ranges_for_buffer(&buffer).is_none());
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn tree_sitter_cache_prunes_buffer_when_language_becomes_unsupported() {
    let mut buffer = TextBuffer::from_text_with_language(
        49,
        Some(PathBuf::from("src/main.rs")),
        "fn main() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.folding_ranges_for_buffer(&buffer).is_some());
    assert_eq!(cache.cached_tree_count(), 1);

    buffer.set_path(PathBuf::from("notes.txt"));

    assert_eq!(buffer.language(), LanguageId::PlainText);
    assert!(cache.folding_ranges_for_buffer(&buffer).is_none());
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn tree_sitter_cache_prunes_same_buffer_entries_when_language_changes() {
    let buffer = TextBuffer::from_text_with_language(
        50,
        Some(PathBuf::from("src/main.rs")),
        "fn main() {}\n".to_owned(),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.folding_ranges_for_buffer(&buffer).is_some());
    let rust_key = syntax_tree_cache_key(&buffer).unwrap();
    let tree = cache.trees.get(&rust_key).unwrap().tree.clone();
    let future_supported_key = SyntaxTreeCacheKey {
        language: LanguageId::Python,
        ..rust_key
    };

    cache.insert_tree(
        future_supported_key,
        tree,
        "def run():\n    pass\n".to_owned(),
    );

    assert_eq!(cache.cached_tree_count(), 1);
    assert!(!cache.trees.contains_key(&rust_key));
    assert!(cache.trees.contains_key(&future_supported_key));
}

#[test]
fn tree_sitter_cache_skips_large_buffers() {
    let buffer = TextBuffer::from_text_with_language(
        29,
        None,
        "a".repeat(LARGE_FILE_MODE_MAX_BYTES + 1),
        LanguageId::Rust,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.folding_ranges_for_buffer(&buffer).is_none());
    assert!(
        cache
            .selection_expansion_for_buffer(&buffer, 0..0)
            .is_none()
    );
    assert_eq!(cache.cached_tree_count(), 0);
}

#[test]
fn tree_sitter_injections_skip_unsupported_languages() {
    let buffer = TextBuffer::from_text_with_language(
        45,
        None,
        "// language=json\nvalue = '{}'\n".to_owned(),
        LanguageId::Python,
    );
    let mut cache = TreeSitterSyntaxCache::default();

    assert!(cache.injections_for_buffer(&buffer).is_none());
    assert_eq!(cache.cached_tree_count(), 0);
}

fn char_index_of(text: &str, needle: &str) -> usize {
    let byte_index = text.find(needle).expect("needle should exist in text");
    text[..byte_index].chars().count()
}

fn parse_rust_tree(text: &str) -> Tree {
    let mut parser = Parser::new();
    parser.set_language(&rust_tree_sitter_language()).unwrap();
    parser.parse(text, None).unwrap()
}

fn text_for_range(text: &str, range: std::ops::Range<usize>) -> String {
    text.chars()
        .skip(range.start)
        .take(range.end.saturating_sub(range.start))
        .collect()
}

fn test_byte_injections_with_invalid_ranges(
    _tree: &Tree,
    text: &str,
) -> Vec<TreeSitterByteInjection> {
    let main_start = text.find("main").unwrap();
    let main_end = main_start + "main".len();
    let unicode_midpoint = text.find('\u{00e9}').unwrap() + 1;
    vec![
        TreeSitterByteInjection {
            language: LanguageId::Rust,
            range: 0..text.len() + 1,
        },
        TreeSitterByteInjection {
            language: LanguageId::Sql,
            range: unicode_midpoint..unicode_midpoint + 1,
        },
        TreeSitterByteInjection {
            language: LanguageId::Json,
            range: main_start..main_end,
        },
    ]
}
