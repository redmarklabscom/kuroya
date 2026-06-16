use super::{TREE_SITTER_FOLDING_CANDIDATE_LIMIT, TREE_SITTER_FOLDING_RANGE_LIMIT};
use kuroya_core::LspFoldingRange;
use tree_sitter::{Node, Tree};

pub(super) fn rust_tree_folding_ranges(tree: &Tree) -> Vec<LspFoldingRange> {
    let mut ranges = Vec::new();
    collect_rust_folding_ranges(tree.root_node(), &mut ranges);
    if ranges.len() < TREE_SITTER_FOLDING_RANGE_LIMIT {
        collect_rust_import_group_folding_ranges(tree.root_node(), &mut ranges);
    }
    if ranges.len() < TREE_SITTER_FOLDING_RANGE_LIMIT {
        collect_rust_comment_folding_ranges(tree.root_node(), &mut ranges);
    }
    normalize_tree_sitter_folding_ranges(&mut ranges);
    ranges
}

fn collect_rust_folding_ranges(node: Node<'_>, ranges: &mut Vec<LspFoldingRange>) {
    if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
        return;
    }

    if rust_node_is_foldable(node)
        && let Some(range) = folding_range_for_node(node)
    {
        ranges.push(range);
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_rust_folding_ranges(child, ranges);
        }
        if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
            return;
        }
    }
}

fn rust_node_is_foldable(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_item"
            | "impl_item"
            | "trait_item"
            | "struct_item"
            | "enum_item"
            | "mod_item"
            | "block"
            | "match_block"
            | "match_expression"
            | "macro_invocation"
            | "where_clause"
            | "type_parameters"
            | "type_arguments"
            | "arguments"
            | "parameters"
            | "field_declaration_list"
            | "ordered_field_declaration_list"
            | "field_initializer_list"
            | "enum_variant_list"
            | "array_expression"
            | "tuple_expression"
            | "use_list"
            | "scoped_use_list"
            | "use_declaration"
    )
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RustUseDeclarationRange {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

fn collect_rust_import_group_folding_ranges(node: Node<'_>, ranges: &mut Vec<LspFoldingRange>) {
    if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
        return;
    }

    let mut declarations = Vec::new();
    collect_rust_use_declaration_ranges(node, &mut declarations);
    if declarations.len() < 2 {
        return;
    }

    declarations.sort_by(|left, right| {
        left.start_line
            .cmp(&right.start_line)
            .then(left.start_column.cmp(&right.start_column))
    });

    let mut group_start = declarations[0];
    let mut group_end = declarations[0];
    for declaration in declarations.into_iter().skip(1) {
        if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
            return;
        }
        if declaration.start_line <= group_end.end_line.saturating_add(1) {
            group_end = declaration;
            continue;
        }

        push_rust_import_group_folding_range(ranges, group_start, group_end);
        group_start = declaration;
        group_end = declaration;
    }
    push_rust_import_group_folding_range(ranges, group_start, group_end);
}

pub(crate) fn collect_rust_use_declaration_ranges(
    node: Node<'_>,
    ranges: &mut Vec<RustUseDeclarationRange>,
) {
    if ranges.len() >= TREE_SITTER_FOLDING_CANDIDATE_LIMIT {
        return;
    }

    if node.kind() == "use_declaration" && !node.is_error() && !node.is_missing() {
        let start = node.start_position();
        let end = node.end_position();
        ranges.push(RustUseDeclarationRange {
            start_line: start.row + 1,
            start_column: start.column + 1,
            end_line: end.row + 1,
            end_column: end.column + 1,
        });
        if ranges.len() >= TREE_SITTER_FOLDING_CANDIDATE_LIMIT {
            return;
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_rust_use_declaration_ranges(child, ranges);
        }
        if ranges.len() >= TREE_SITTER_FOLDING_CANDIDATE_LIMIT {
            return;
        }
    }
}

fn push_rust_import_group_folding_range(
    ranges: &mut Vec<LspFoldingRange>,
    group_start: RustUseDeclarationRange,
    group_end: RustUseDeclarationRange,
) {
    if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
        return;
    }
    if group_end.end_line <= group_start.start_line {
        return;
    }

    ranges.push(LspFoldingRange {
        start_line: group_start.start_line,
        start_column: Some(group_start.start_column),
        end_line: group_end.end_line,
        end_column: Some(group_end.end_column),
        kind: Some("imports".to_owned()),
    });
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RustCommentRange {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
    line_comment: bool,
}

fn collect_rust_comment_folding_ranges(node: Node<'_>, ranges: &mut Vec<LspFoldingRange>) {
    if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
        return;
    }

    let mut comments = Vec::new();
    collect_rust_comment_ranges(node, &mut comments);
    if comments.is_empty() {
        return;
    }

    comments.sort_by(|left, right| {
        left.start_line
            .cmp(&right.start_line)
            .then(left.start_column.cmp(&right.start_column))
    });

    let mut line_group: Option<(RustCommentRange, RustCommentRange)> = None;
    for comment in comments {
        if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
            break;
        }
        if !comment.line_comment {
            push_rust_line_comment_group_folding_range(ranges, line_group.take());
            push_rust_comment_folding_range(ranges, comment);
            continue;
        }

        match line_group {
            Some((start, end)) if comment.start_line == end.end_line.saturating_add(1) => {
                line_group = Some((start, comment));
            }
            Some(group) => {
                push_rust_line_comment_group_folding_range(ranges, Some(group));
                line_group = Some((comment, comment));
            }
            None => {
                line_group = Some((comment, comment));
            }
        }
    }
    push_rust_line_comment_group_folding_range(ranges, line_group);
}

pub(crate) fn collect_rust_comment_ranges(node: Node<'_>, ranges: &mut Vec<RustCommentRange>) {
    if ranges.len() >= TREE_SITTER_FOLDING_CANDIDATE_LIMIT {
        return;
    }

    match node.kind() {
        "line_comment" | "block_comment" if !node.is_error() && !node.is_missing() => {
            let start = node.start_position();
            let end = node.end_position();
            ranges.push(RustCommentRange {
                start_line: start.row + 1,
                start_column: start.column + 1,
                end_line: end.row + 1,
                end_column: end.column + 1,
                line_comment: node.kind() == "line_comment",
            });
            if ranges.len() >= TREE_SITTER_FOLDING_CANDIDATE_LIMIT {
                return;
            }
        }
        _ => {}
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_rust_comment_ranges(child, ranges);
        }
        if ranges.len() >= TREE_SITTER_FOLDING_CANDIDATE_LIMIT {
            return;
        }
    }
}

fn push_rust_line_comment_group_folding_range(
    ranges: &mut Vec<LspFoldingRange>,
    group: Option<(RustCommentRange, RustCommentRange)>,
) {
    if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
        return;
    }
    let Some((start, end)) = group else {
        return;
    };
    if end.end_line <= start.start_line {
        return;
    }

    ranges.push(LspFoldingRange {
        start_line: start.start_line,
        start_column: Some(start.start_column),
        end_line: end.end_line,
        end_column: Some(end.end_column),
        kind: Some("comment".to_owned()),
    });
}

fn push_rust_comment_folding_range(ranges: &mut Vec<LspFoldingRange>, comment: RustCommentRange) {
    if ranges.len() >= TREE_SITTER_FOLDING_RANGE_LIMIT {
        return;
    }
    if comment.end_line <= comment.start_line {
        return;
    }

    ranges.push(LspFoldingRange {
        start_line: comment.start_line,
        start_column: Some(comment.start_column),
        end_line: comment.end_line,
        end_column: Some(comment.end_column),
        kind: Some("comment".to_owned()),
    });
}

fn folding_range_for_node(node: Node<'_>) -> Option<LspFoldingRange> {
    let start = node.start_position();
    let end = node.end_position();
    let start_line = start.row + 1;
    let end_line = end.row + 1;
    (end_line > start_line).then(|| LspFoldingRange {
        start_line,
        start_column: Some(start.column + 1),
        end_line,
        end_column: Some(end.column + 1),
        kind: rust_folding_kind(node.kind()).map(ToOwned::to_owned),
    })
}

fn rust_folding_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "use_declaration" => Some("imports"),
        _ => None,
    }
}

fn normalize_tree_sitter_folding_ranges(ranges: &mut Vec<LspFoldingRange>) {
    ranges.retain(|range| range.start_line > 0 && range.end_line > range.start_line);
    ranges.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.end_line.cmp(&b.end_line))
            .then(a.start_column.cmp(&b.start_column))
            .then(a.end_column.cmp(&b.end_column))
            .then(a.kind.cmp(&b.kind))
    });
    ranges.dedup_by(|left, right| {
        left.start_line == right.start_line
            && left.end_line == right.end_line
            && left.kind == right.kind
    });
    ranges.truncate(TREE_SITTER_FOLDING_RANGE_LIMIT);
}
