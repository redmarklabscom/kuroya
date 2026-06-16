use super::{
    bounded_lsp_text, bounded_lsp_value, file_uri_to_path, one_based_lsp_position_component,
    parse_lsp_location_range, parse_lsp_range, value_as_u8, value_as_usize,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(super) const MAX_LSP_REFERENCES: usize = 5_000;
const MAX_LSP_CALL_HIERARCHY_ITEMS: usize = 100;
const MAX_LSP_CALL_HIERARCHY_CALLS: usize = 500;
const MAX_LSP_CALL_HIERARCHY_RANGES: usize = 100;
pub(super) const MAX_LSP_CALL_HIERARCHY_NAME_CHARS: usize = 512;
pub(super) const MAX_LSP_CALL_HIERARCHY_DETAIL_CHARS: usize = 1_024;
pub(super) const MAX_LSP_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES: usize = 64 * 1024;
const MAX_LSP_TYPE_HIERARCHY_PREPARE_ITEMS: usize = 100;
const MAX_LSP_TYPE_HIERARCHY_RELATION_ITEMS: usize = 500;
const MAX_LSP_TYPE_HIERARCHY_NAME_CHARS: usize = 512;
const MAX_LSP_TYPE_HIERARCHY_DETAIL_CHARS: usize = 1_024;
const MAX_LSP_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES: usize = 64 * 1024;
pub(super) const MAX_LSP_DOCUMENT_SYMBOLS: usize = 5_000;
const MAX_LSP_DOCUMENT_SYMBOL_DEPTH: usize = 64;
const MAX_LSP_DOCUMENT_SYMBOL_NAME_CHARS: usize = 512;
const MAX_LSP_DOCUMENT_SYMBOL_DETAIL_CHARS: usize = 1_024;
const MAX_LSP_FOLDING_RANGES: usize = 1_000;
const MAX_LSP_FOLDING_RANGE_KIND_CHARS: usize = 64;
pub(super) const MAX_LSP_WORKSPACE_SYMBOLS: usize = 300;
pub(super) const MAX_LSP_WORKSPACE_SYMBOL_NAME_CHARS: usize = 512;
pub(super) const MAX_LSP_WORKSPACE_SYMBOL_DETAIL_CHARS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspReference {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCallHierarchyItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u8,
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCallHierarchyRange {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCallHierarchyCall {
    pub item: LspCallHierarchyItem,
    pub ranges: Vec<LspCallHierarchyRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspTypeHierarchyItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u8,
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u8,
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspFoldingRange {
    pub start_line: usize,
    pub start_column: Option<usize>,
    pub end_line: usize,
    pub end_column: Option<usize>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspWorkspaceSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: u8,
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

pub fn parse_references_response(value: &Value) -> Option<Vec<LspReference>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let result = result.as_array()?;
    let mut references = Vec::with_capacity(result.len().min(MAX_LSP_REFERENCES));
    for item in result.iter().take(MAX_LSP_REFERENCES) {
        if let Some(reference) = parse_lsp_reference(item) {
            references.push(reference);
        }
    }
    references.sort_unstable_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.end_line.cmp(&b.end_line))
            .then(a.end_column.cmp(&b.end_column))
    });
    references.dedup();
    Some(references)
}

pub fn parse_call_hierarchy_prepare_response(value: &Value) -> Option<Vec<LspCallHierarchyItem>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let result = result.as_array()?;
    let mut items = Vec::with_capacity(result.len().min(MAX_LSP_CALL_HIERARCHY_ITEMS));
    for item in result.iter().take(MAX_LSP_CALL_HIERARCHY_ITEMS) {
        if let Some(item) = parse_call_hierarchy_item(item) {
            items.push(item);
        }
    }
    sort_call_hierarchy_items(&mut items);
    items.dedup();
    Some(items)
}

pub fn parse_call_hierarchy_incoming_response(value: &Value) -> Option<Vec<LspCallHierarchyCall>> {
    parse_call_hierarchy_calls_response(value, "from", "fromRanges")
}

pub fn parse_call_hierarchy_outgoing_response(value: &Value) -> Option<Vec<LspCallHierarchyCall>> {
    parse_call_hierarchy_calls_response(value, "to", "fromRanges")
}

pub fn parse_type_hierarchy_prepare_response(value: &Value) -> Option<Vec<LspTypeHierarchyItem>> {
    parse_type_hierarchy_items_response(value, MAX_LSP_TYPE_HIERARCHY_PREPARE_ITEMS)
}

pub fn parse_type_hierarchy_supertypes_response(
    value: &Value,
) -> Option<Vec<LspTypeHierarchyItem>> {
    parse_type_hierarchy_items_response(value, MAX_LSP_TYPE_HIERARCHY_RELATION_ITEMS)
}

pub fn parse_type_hierarchy_subtypes_response(value: &Value) -> Option<Vec<LspTypeHierarchyItem>> {
    parse_type_hierarchy_items_response(value, MAX_LSP_TYPE_HIERARCHY_RELATION_ITEMS)
}

pub fn parse_document_symbols_response(
    value: &Value,
    document_path: &Path,
) -> Option<Vec<LspDocumentSymbol>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let items = result.as_array()?;
    let mut symbols = Vec::with_capacity(items.len().min(MAX_LSP_DOCUMENT_SYMBOLS));
    for item in items {
        if symbols.len() >= MAX_LSP_DOCUMENT_SYMBOLS {
            break;
        }
        collect_document_symbol(&mut symbols, item, document_path, 0)?;
    }
    symbols.sort_unstable_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.end_line.cmp(&b.end_line))
            .then(a.end_column.cmp(&b.end_column))
            .then(a.depth.cmp(&b.depth))
            .then(a.name.cmp(&b.name))
            .then(a.detail.cmp(&b.detail))
            .then(a.kind.cmp(&b.kind))
    });

    Some(symbols)
}

pub fn parse_folding_ranges_response(value: &Value) -> Option<Vec<LspFoldingRange>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let result = result.as_array()?;
    let mut ranges = Vec::with_capacity(result.len().min(MAX_LSP_FOLDING_RANGES));
    for item in result.iter().take(MAX_LSP_FOLDING_RANGES) {
        if let Some(range) = parse_folding_range_item(item)
            && range.end_line > range.start_line
        {
            ranges.push(range);
        }
    }
    ranges.sort_unstable_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.end_line.cmp(&b.end_line))
            .then(a.start_column.cmp(&b.start_column))
            .then(a.end_column.cmp(&b.end_column))
            .then(a.kind.cmp(&b.kind))
    });
    ranges.dedup();
    Some(ranges)
}

pub fn parse_workspace_symbols_response(value: &Value) -> Option<Vec<LspWorkspaceSymbol>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let result = result.as_array()?;
    let mut symbols = Vec::with_capacity(result.len().min(MAX_LSP_WORKSPACE_SYMBOLS));
    for item in result.iter().take(MAX_LSP_WORKSPACE_SYMBOLS) {
        if let Some(symbol) = parse_workspace_symbol_item(item) {
            symbols.push(symbol);
        }
    }
    symbols.sort_unstable_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.end_line.cmp(&b.end_line))
            .then(a.end_column.cmp(&b.end_column))
            .then(a.name.cmp(&b.name))
            .then(a.detail.cmp(&b.detail))
            .then(a.kind.cmp(&b.kind))
    });
    symbols.dedup();
    Some(symbols)
}

fn parse_lsp_reference(value: &Value) -> Option<LspReference> {
    let (path, line, column, end_line, end_column) = parse_lsp_location_range(value)?;
    Some(LspReference {
        path,
        line,
        column,
        end_line,
        end_column,
    })
}

fn parse_call_hierarchy_item(value: &Value) -> Option<LspCallHierarchyItem> {
    let path = file_uri_to_path(value.get("uri")?.as_str()?)?;
    let selection_range = value.get("selectionRange").or_else(|| value.get("range"))?;
    let (line, column, end_line, end_column) = parse_lsp_range(selection_range)?;
    let name = bounded_lsp_text(
        value.get("name")?.as_str()?,
        MAX_LSP_CALL_HIERARCHY_NAME_CHARS,
    )?;
    let raw = bounded_lsp_value(value, MAX_LSP_CALL_HIERARCHY_ITEM_PAYLOAD_BYTES)?
        .as_ref()
        .clone();
    (!name.is_empty()).then_some(LspCallHierarchyItem {
        name,
        detail: value
            .get("detail")
            .and_then(Value::as_str)
            .and_then(|detail| bounded_lsp_text(detail, MAX_LSP_CALL_HIERARCHY_DETAIL_CHARS)),
        kind: value.get("kind").and_then(value_as_u8)?,
        path,
        line,
        column,
        end_line,
        end_column,
        raw,
    })
}

fn parse_call_hierarchy_calls_response(
    value: &Value,
    item_key: &str,
    ranges_key: &str,
) -> Option<Vec<LspCallHierarchyCall>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let result = result.as_array()?;
    let mut calls = Vec::with_capacity(result.len().min(MAX_LSP_CALL_HIERARCHY_CALLS));
    for call in result.iter().take(MAX_LSP_CALL_HIERARCHY_CALLS) {
        if let Some(call) = parse_call_hierarchy_call(call, item_key, ranges_key) {
            calls.push(call);
        }
    }
    calls.sort_by(|a, b| {
        a.item
            .path
            .cmp(&b.item.path)
            .then(a.item.line.cmp(&b.item.line))
            .then(a.item.column.cmp(&b.item.column))
            .then(a.item.name.cmp(&b.item.name))
    });
    calls.dedup();
    Some(calls)
}

fn parse_call_hierarchy_call(
    value: &Value,
    item_key: &str,
    ranges_key: &str,
) -> Option<LspCallHierarchyCall> {
    let item = parse_call_hierarchy_item(value.get(item_key)?)?;
    let range_values = value.get(ranges_key)?.as_array()?;
    let mut ranges = Vec::with_capacity(range_values.len().min(MAX_LSP_CALL_HIERARCHY_RANGES));
    for range in range_values.iter().take(MAX_LSP_CALL_HIERARCHY_RANGES) {
        if let Some(range) = parse_call_hierarchy_range(range) {
            ranges.push(range);
        }
    }
    Some(LspCallHierarchyCall { item, ranges })
}

fn parse_call_hierarchy_range(value: &Value) -> Option<LspCallHierarchyRange> {
    let (line, column, end_line, end_column) = parse_lsp_range(value)?;
    Some(LspCallHierarchyRange {
        line,
        column,
        end_line,
        end_column,
    })
}

fn sort_call_hierarchy_items(items: &mut [LspCallHierarchyItem]) {
    items.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.name.cmp(&b.name))
    });
}

fn parse_type_hierarchy_items_response(
    value: &Value,
    limit: usize,
) -> Option<Vec<LspTypeHierarchyItem>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let result = result.as_array()?;
    let mut items = Vec::with_capacity(result.len().min(limit));
    for item in result.iter().take(limit) {
        if let Some(item) = parse_type_hierarchy_item(item) {
            items.push(item);
        }
    }
    sort_type_hierarchy_items(&mut items);
    items.dedup();
    Some(items)
}

fn parse_type_hierarchy_item(value: &Value) -> Option<LspTypeHierarchyItem> {
    let path = file_uri_to_path(value.get("uri")?.as_str()?)?;
    let selection_range = value.get("selectionRange").or_else(|| value.get("range"))?;
    let (line, column, end_line, end_column) = parse_lsp_range(selection_range)?;
    let name = bounded_lsp_text(
        value.get("name")?.as_str()?,
        MAX_LSP_TYPE_HIERARCHY_NAME_CHARS,
    )?;
    let raw = bounded_lsp_value(value, MAX_LSP_TYPE_HIERARCHY_ITEM_PAYLOAD_BYTES)?
        .as_ref()
        .clone();
    (!name.is_empty()).then_some(LspTypeHierarchyItem {
        name,
        detail: value
            .get("detail")
            .and_then(Value::as_str)
            .and_then(|detail| bounded_lsp_text(detail, MAX_LSP_TYPE_HIERARCHY_DETAIL_CHARS)),
        kind: value.get("kind").and_then(value_as_u8)?,
        path,
        line,
        column,
        end_line,
        end_column,
        raw,
    })
}

fn sort_type_hierarchy_items(items: &mut [LspTypeHierarchyItem]) {
    items.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.name.cmp(&b.name))
    });
}

fn collect_document_symbol(
    output: &mut Vec<LspDocumentSymbol>,
    value: &Value,
    document_path: &Path,
    depth: usize,
) -> Option<()> {
    if output.len() >= MAX_LSP_DOCUMENT_SYMBOLS {
        return Some(());
    }
    if depth > MAX_LSP_DOCUMENT_SYMBOL_DEPTH {
        return Some(());
    }
    if value.get("selectionRange").is_some() {
        let symbol = parse_hierarchical_symbol(value, document_path, depth)?;
        output.push(symbol);
        if let Some(children) = value.get("children").and_then(Value::as_array) {
            for child in children {
                if output.len() >= MAX_LSP_DOCUMENT_SYMBOLS {
                    break;
                }
                collect_document_symbol(output, child, document_path, depth + 1)?;
            }
        }
        return Some(());
    }

    if output.len() >= MAX_LSP_DOCUMENT_SYMBOLS {
        return Some(());
    }
    output.push(parse_symbol_information(value, depth)?);
    Some(())
}

fn parse_hierarchical_symbol(
    value: &Value,
    document_path: &Path,
    depth: usize,
) -> Option<LspDocumentSymbol> {
    let selection_range = value.get("selectionRange").or_else(|| value.get("range"))?;
    let range = value.get("range").unwrap_or(selection_range);
    let (line, column, _, _) = parse_lsp_range(selection_range)?;
    let (_, _, end_line, end_column) = parse_lsp_range(range)?;
    Some(LspDocumentSymbol {
        name: bounded_lsp_text(
            value.get("name")?.as_str()?,
            MAX_LSP_DOCUMENT_SYMBOL_NAME_CHARS,
        )?,
        detail: value
            .get("detail")
            .and_then(Value::as_str)
            .and_then(|detail| bounded_lsp_text(detail, MAX_LSP_DOCUMENT_SYMBOL_DETAIL_CHARS)),
        kind: value.get("kind").and_then(value_as_u8)?,
        path: document_path.to_path_buf(),
        line,
        column,
        end_line,
        end_column,
        depth,
    })
}

fn parse_symbol_information(value: &Value, depth: usize) -> Option<LspDocumentSymbol> {
    let location = value.get("location")?;
    let path = file_uri_to_path(location.get("uri")?.as_str()?)?;
    let (line, column, end_line, end_column) = parse_lsp_range(location.get("range")?)?;
    Some(LspDocumentSymbol {
        name: bounded_lsp_text(
            value.get("name")?.as_str()?,
            MAX_LSP_DOCUMENT_SYMBOL_NAME_CHARS,
        )?,
        detail: value
            .get("containerName")
            .and_then(Value::as_str)
            .and_then(|detail| bounded_lsp_text(detail, MAX_LSP_DOCUMENT_SYMBOL_DETAIL_CHARS)),
        kind: value.get("kind").and_then(value_as_u8)?,
        path,
        line,
        column,
        end_line,
        end_column,
        depth,
    })
}

fn parse_folding_range_item(value: &Value) -> Option<LspFoldingRange> {
    let start_line = value_as_usize(value.get("startLine")?)?;
    let end_line = value_as_usize(value.get("endLine")?)?;
    if end_line < start_line {
        return None;
    }
    Some(LspFoldingRange {
        start_line: one_based_lsp_position_component(start_line)?,
        start_column: value
            .get("startCharacter")
            .and_then(value_as_usize)
            .and_then(one_based_lsp_position_component),
        end_line: one_based_lsp_position_component(end_line)?,
        end_column: value
            .get("endCharacter")
            .and_then(value_as_usize)
            .and_then(one_based_lsp_position_component),
        kind: value
            .get("kind")
            .and_then(Value::as_str)
            .and_then(|kind| bounded_lsp_text(kind, MAX_LSP_FOLDING_RANGE_KIND_CHARS)),
    })
}

fn parse_workspace_symbol_item(value: &Value) -> Option<LspWorkspaceSymbol> {
    let (path, line, column, end_line, end_column) =
        parse_workspace_symbol_location(value.get("location")?)?;

    Some(LspWorkspaceSymbol {
        name: bounded_lsp_text(
            value.get("name")?.as_str()?,
            MAX_LSP_WORKSPACE_SYMBOL_NAME_CHARS,
        )?,
        detail: value
            .get("containerName")
            .or_else(|| value.get("detail"))
            .and_then(Value::as_str)
            .and_then(|detail| bounded_lsp_text(detail, MAX_LSP_WORKSPACE_SYMBOL_DETAIL_CHARS)),
        kind: value.get("kind").and_then(value_as_u8)?,
        path,
        line,
        column,
        end_line,
        end_column,
    })
}

fn parse_workspace_symbol_location(value: &Value) -> Option<(PathBuf, usize, usize, usize, usize)> {
    if value.get("targetUri").is_some()
        || value.get("targetRange").is_some()
        || value.get("targetSelectionRange").is_some()
    {
        return None;
    }

    let path = file_uri_to_path(value.get("uri")?.as_str()?)?;
    let Some(range) = value.get("range") else {
        return Some((path, 1, 1, 1, 1));
    };
    let (line, column, end_line, end_column) = parse_lsp_range(range)?;
    Some((path, line, column, end_line, end_column))
}
