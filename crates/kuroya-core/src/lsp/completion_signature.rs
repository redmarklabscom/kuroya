use serde_json::Value;
use std::{ops::Range, path::Path, sync::Arc};

use super::snippet::expand_lsp_completion_snippet;
use super::text::{
    bounded_lsp_insert_text, bounded_lsp_markdown_text, bounded_lsp_text, push_bounded_lsp_text,
    trim_lsp_text_in_place,
};
use super::{
    LspTextEdit, MAX_LSP_TEXT_EDITS, bounded_lsp_value, collect_lsp_text_edits,
    hover_contents_to_text, parse_lsp_range, value_as_u8, value_as_usize,
};

const LSP_INSERT_TEXT_FORMAT_SNIPPET: u64 = 2;
pub(super) const MAX_LSP_COMPLETION_ITEMS: usize = 200;
pub(super) const MAX_LSP_COMPLETION_LABEL_CHARS: usize = 512;
pub(super) const MAX_LSP_COMPLETION_DETAIL_CHARS: usize = 1_024;
pub(super) const MAX_LSP_COMPLETION_DOCUMENTATION_CHARS: usize = 16_000;
pub(super) const MAX_LSP_COMPLETION_SORT_TEXT_CHARS: usize = 512;
pub(super) const MAX_LSP_COMPLETION_FILTER_TEXT_CHARS: usize = 512;
pub(super) const MAX_LSP_COMPLETION_COMMIT_CHARACTERS: usize = 16;
pub(super) const MAX_LSP_COMPLETION_COMMIT_CHARACTER_CHARS: usize = 16;
pub(super) const MAX_LSP_COMPLETION_RESOLVE_PAYLOAD_BYTES: usize = 64 * 1024;
pub(super) const MAX_LSP_SIGNATURES: usize = 20;
pub(super) const MAX_LSP_SIGNATURE_PARAMETERS: usize = 30;
pub(super) const MAX_LSP_SIGNATURE_LABEL_CHARS: usize = 16_000;
pub(super) const MAX_LSP_SIGNATURE_DOCUMENTATION_CHARS: usize = 16_000;
pub(super) const MAX_LSP_SIGNATURE_PARAMETER_LABEL_CHARS: usize = 4_000;
pub(super) const MAX_LSP_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSignatureHelp {
    pub signatures: Vec<LspSignatureInformation>,
    pub active_signature: usize,
    pub active_parameter: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspSignatureInformation {
    pub label: String,
    pub documentation: Option<String>,
    pub parameters: Vec<LspParameterInformation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspParameterInformation {
    pub label: String,
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspCompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub kind: Option<u8>,
    pub deprecated: bool,
    pub is_snippet: bool,
    pub sort_text: Option<String>,
    pub filter_text: Option<String>,
    pub preselect: bool,
    pub commit_characters: Vec<String>,
    pub insert_text: String,
    pub snippet_selection: Option<Range<usize>>,
    pub snippet_tabstops: Vec<Range<usize>>,
    pub snippet_tabstop_groups: Vec<Vec<Range<usize>>>,
    pub text_edit: Option<LspTextEdit>,
    pub insert_text_edit: Option<LspTextEdit>,
    pub additional_text_edits: Vec<LspTextEdit>,
    pub resolve_payload: Option<Arc<Value>>,
}

impl LspCompletionItem {
    pub fn needs_resolve(&self) -> bool {
        self.resolve_payload.is_some()
    }
}

pub fn parse_completion_response(
    value: &Value,
    document_path: &Path,
) -> Option<Vec<LspCompletionItem>> {
    let result = value.get("result")?;
    if result.is_null() {
        return Some(Vec::new());
    }

    let (items, defaults) = if let Some(items) = result.as_array() {
        (items, CompletionItemDefaults::default())
    } else {
        (
            result.get("items")?.as_array()?,
            CompletionItemDefaults::from_completion_list(result),
        )
    };

    let mut completions = Vec::with_capacity(items.len().min(MAX_LSP_COMPLETION_ITEMS));
    for item in items.iter().take(MAX_LSP_COMPLETION_ITEMS) {
        if let Some(item) = parse_completion_item(item, document_path, defaults) {
            completions.push(item);
        }
    }
    Some(completions)
}

pub fn parse_completion_item_resolve_response(
    value: &Value,
    document_path: &Path,
    original_item: &LspCompletionItem,
) -> Option<LspCompletionItem> {
    let result = value.get("result")?;
    if result.is_null() {
        return None;
    }

    let mut item = parse_completion_item(result, document_path, CompletionItemDefaults::default())?;
    merge_unresolved_completion_fields(&mut item, result, original_item);
    item.resolve_payload = None;
    Some(item)
}

pub fn parse_signature_help_response(value: &Value) -> Option<LspSignatureHelp> {
    let result = value.get("result")?;
    if result.is_null() {
        return None;
    }

    let signature_items = result.get("signatures")?.as_array()?;
    let mut signatures = Vec::with_capacity(signature_items.len().min(MAX_LSP_SIGNATURES));
    for item in signature_items.iter().take(MAX_LSP_SIGNATURES) {
        if let Some(signature) = parse_signature_information(item) {
            signatures.push(signature);
        }
    }
    if signatures.is_empty() {
        return None;
    }

    let active_signature = result
        .get("activeSignature")
        .and_then(value_as_usize)
        .unwrap_or(0)
        .min(signatures.len().saturating_sub(1));
    let active_parameter = active_signature_parameter(result, active_signature).and_then(|idx| {
        signatures[active_signature]
            .parameters
            .get(idx)
            .map(|_| idx)
    });

    Some(LspSignatureHelp {
        signatures,
        active_signature,
        active_parameter,
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct CompletionItemDefaults<'a> {
    edit_range: Option<&'a Value>,
    insert_text_format: Option<u64>,
    commit_characters: Option<&'a Value>,
}

impl<'a> CompletionItemDefaults<'a> {
    fn from_completion_list(value: &'a Value) -> Self {
        let defaults = value.get("itemDefaults");
        Self {
            edit_range: defaults.and_then(|defaults| defaults.get("editRange")),
            insert_text_format: defaults
                .and_then(|defaults| defaults.get("insertTextFormat"))
                .and_then(Value::as_u64),
            commit_characters: defaults.and_then(|defaults| defaults.get("commitCharacters")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletionTextEdits {
    text_edit: Option<LspTextEdit>,
    insert_text_edit: Option<LspTextEdit>,
}

fn parse_completion_item(
    value: &Value,
    document_path: &Path,
    defaults: CompletionItemDefaults<'_>,
) -> Option<LspCompletionItem> {
    let label = bounded_lsp_text(
        value.get("label")?.as_str()?,
        MAX_LSP_COMPLETION_LABEL_CHARS,
    )?;
    let is_snippet = completion_item_is_snippet(value, defaults);
    let mut snippet_selection = None;
    let mut snippet_tabstops = Vec::new();
    let mut snippet_tabstop_groups = Vec::new();
    let default_new_text =
        completion_item_default_new_text(value, &label, defaults.edit_range.is_some())?;
    let text_edit_value = value.get("textEdit");
    let mut text_edit =
        text_edit_value.and_then(|text_edit| parse_completion_text_edit(text_edit, document_path));
    let mut insert_text_edit = text_edit_value
        .and_then(|text_edit| parse_completion_insert_text_edit(text_edit, document_path));
    if text_edit_value.is_some() && text_edit.is_none() && insert_text_edit.is_none() {
        return None;
    }
    if text_edit_value.is_none()
        && let Some(default_edit_range) = defaults.edit_range
    {
        let default_edits = parse_completion_default_text_edits(
            default_edit_range,
            document_path,
            &default_new_text,
        )?;
        text_edit = default_edits.text_edit;
        insert_text_edit = default_edits.insert_text_edit;
    }
    if is_snippet {
        if let Some(edit) = text_edit.as_mut() {
            let expansion = expand_lsp_completion_snippet(&edit.new_text)?;
            snippet_selection = expansion.selection.clone();
            snippet_tabstops = expansion.tabstops.clone();
            snippet_tabstop_groups = expansion.tabstop_groups.clone();
            edit.new_text = expansion.text;
        }
        if let Some(edit) = insert_text_edit.as_mut() {
            let expansion = expand_lsp_completion_snippet(&edit.new_text)?;
            if snippet_selection.is_none() {
                snippet_selection = expansion.selection.clone();
            }
            if snippet_tabstops.is_empty() {
                snippet_tabstops = expansion.tabstops.clone();
            }
            if snippet_tabstop_groups.is_empty() {
                snippet_tabstop_groups = expansion.tabstop_groups.clone();
            }
            edit.new_text = expansion.text;
        }
    }
    let additional_text_edits = if let Some(edits) = value.get("additionalTextEdits") {
        let edits = edits.as_array()?;
        let mut output = Vec::with_capacity(edits.len().min(MAX_LSP_TEXT_EDITS));
        collect_lsp_text_edits(&mut output, document_path, edits)?;
        output
    } else {
        Vec::new()
    };
    let insert_text = if let Some(edit) = text_edit.as_ref() {
        edit.new_text.clone()
    } else if is_snippet {
        let expansion = expand_lsp_completion_snippet(&default_new_text)?;
        if snippet_selection.is_none() {
            snippet_selection = expansion.selection.clone();
        }
        if snippet_tabstops.is_empty() {
            snippet_tabstops = expansion.tabstops.clone();
        }
        if snippet_tabstop_groups.is_empty() {
            snippet_tabstop_groups = expansion.tabstop_groups.clone();
        }
        expansion.text
    } else {
        default_new_text
    };

    Some(LspCompletionItem {
        label,
        detail: completion_item_detail(value),
        documentation: value
            .get("documentation")
            .and_then(hover_contents_to_text)
            .and_then(|documentation| {
                bounded_lsp_markdown_text(&documentation, MAX_LSP_COMPLETION_DOCUMENTATION_CHARS)
            }),
        kind: value.get("kind").and_then(value_as_u8),
        deprecated: completion_item_is_deprecated(value),
        is_snippet,
        sort_text: value
            .get("sortText")
            .and_then(Value::as_str)
            .and_then(|text| bounded_lsp_text(text, MAX_LSP_COMPLETION_SORT_TEXT_CHARS)),
        filter_text: value
            .get("filterText")
            .and_then(Value::as_str)
            .and_then(|text| bounded_lsp_text(text, MAX_LSP_COMPLETION_FILTER_TEXT_CHARS)),
        preselect: value
            .get("preselect")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        commit_characters: completion_item_commit_characters(value, defaults),
        insert_text,
        snippet_selection,
        snippet_tabstops,
        snippet_tabstop_groups,
        text_edit,
        insert_text_edit,
        additional_text_edits,
        resolve_payload: completion_item_resolve_payload(value),
    })
}

fn completion_item_resolve_payload(value: &Value) -> Option<Arc<Value>> {
    if value.get("data").is_some() {
        bounded_lsp_value(value, MAX_LSP_COMPLETION_RESOLVE_PAYLOAD_BYTES)
    } else {
        None
    }
}

fn merge_unresolved_completion_fields(
    item: &mut LspCompletionItem,
    value: &Value,
    original_item: &LspCompletionItem,
) {
    if value.get("detail").is_none() && value.get("labelDetails").is_none() {
        item.detail = original_item.detail.clone();
    }
    if value.get("documentation").is_none() {
        item.documentation = original_item.documentation.clone();
    }
    if value.get("kind").is_none() {
        item.kind = original_item.kind;
    }
    if value.get("deprecated").is_none() && value.get("tags").is_none() {
        item.deprecated = original_item.deprecated;
    }
    if value.get("sortText").is_none() {
        item.sort_text = original_item.sort_text.clone();
    }
    if value.get("filterText").is_none() {
        item.filter_text = original_item.filter_text.clone();
    }
    if value.get("preselect").is_none() {
        item.preselect = original_item.preselect;
    }
    if value.get("commitCharacters").is_none() {
        item.commit_characters = original_item.commit_characters.clone();
    }
    if value.get("additionalTextEdits").is_none() {
        item.additional_text_edits = original_item.additional_text_edits.clone();
    }
    if !completion_item_has_insert_payload(value) {
        item.is_snippet = original_item.is_snippet;
        item.insert_text = original_item.insert_text.clone();
        item.snippet_selection = original_item.snippet_selection.clone();
        item.snippet_tabstops = original_item.snippet_tabstops.clone();
        item.snippet_tabstop_groups = original_item.snippet_tabstop_groups.clone();
        item.text_edit = original_item.text_edit.clone();
        item.insert_text_edit = original_item.insert_text_edit.clone();
    }
}

fn completion_item_has_insert_payload(value: &Value) -> bool {
    value.get("textEdit").is_some()
        || value.get("insertText").is_some()
        || value.get("textEditText").is_some()
}

fn completion_item_commit_characters(
    value: &Value,
    defaults: CompletionItemDefaults<'_>,
) -> Vec<String> {
    value
        .get("commitCharacters")
        .or(defaults.commit_characters)
        .and_then(Value::as_array)
        .map(|characters| {
            characters
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|text| {
                    bounded_lsp_text(text, MAX_LSP_COMPLETION_COMMIT_CHARACTER_CHARS)
                })
                .take(MAX_LSP_COMPLETION_COMMIT_CHARACTERS)
                .collect()
        })
        .unwrap_or_default()
}

fn completion_item_is_snippet(value: &Value, defaults: CompletionItemDefaults<'_>) -> bool {
    value
        .get("insertTextFormat")
        .and_then(Value::as_u64)
        .or(defaults.insert_text_format)
        .is_some_and(|format| format == LSP_INSERT_TEXT_FORMAT_SNIPPET)
}

fn completion_item_default_new_text(
    value: &Value,
    label: &str,
    allow_text_edit_text: bool,
) -> Option<String> {
    if allow_text_edit_text {
        if let Some(text_edit_text) = value.get("textEditText").and_then(Value::as_str) {
            return bounded_lsp_insert_text(text_edit_text);
        }
        return Some(label.to_owned());
    }

    if let Some(insert_text) = value.get("insertText").and_then(Value::as_str) {
        return bounded_lsp_insert_text(insert_text);
    }

    Some(label.to_owned())
}

fn parse_completion_default_text_edits(
    edit_range: &Value,
    document_path: &Path,
    new_text: &str,
) -> Option<CompletionTextEdits> {
    if edit_range.get("start").is_some() || edit_range.get("end").is_some() {
        return Some(CompletionTextEdits {
            text_edit: Some(parse_completion_text_edit_range_with_text(
                edit_range,
                document_path,
                new_text,
            )?),
            insert_text_edit: None,
        });
    }

    let insert = edit_range.get("insert")?;
    let replace = edit_range.get("replace")?;
    Some(CompletionTextEdits {
        text_edit: Some(parse_completion_text_edit_range_with_text(
            replace,
            document_path,
            new_text,
        )?),
        insert_text_edit: Some(parse_completion_text_edit_range_with_text(
            insert,
            document_path,
            new_text,
        )?),
    })
}

fn completion_item_is_deprecated(value: &Value) -> bool {
    value
        .get("deprecated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || value
            .get("tags")
            .and_then(Value::as_array)
            .is_some_and(|tags| tags.iter().any(|tag| tag.as_u64() == Some(1)))
}

fn completion_item_detail(value: &Value) -> Option<String> {
    value
        .get("detail")
        .and_then(Value::as_str)
        .and_then(|detail| bounded_lsp_text(detail, MAX_LSP_COMPLETION_DETAIL_CHARS))
        .or_else(|| {
            value
                .get("labelDetails")
                .and_then(completion_item_label_details)
        })
}

fn completion_item_label_details(value: &Value) -> Option<String> {
    let mut detail = String::new();
    let mut chars = 0usize;
    for key in ["detail", "description"] {
        let Some(part) = value
            .get(key)
            .and_then(Value::as_str)
            .and_then(|part| bounded_lsp_text(part, MAX_LSP_COMPLETION_DETAIL_CHARS))
        else {
            continue;
        };
        if !detail.is_empty() {
            push_bounded_lsp_text(
                &mut detail,
                &mut chars,
                " ",
                MAX_LSP_COMPLETION_DETAIL_CHARS,
            );
        }
        push_bounded_lsp_text(
            &mut detail,
            &mut chars,
            &part,
            MAX_LSP_COMPLETION_DETAIL_CHARS,
        );
        if chars >= MAX_LSP_COMPLETION_DETAIL_CHARS {
            break;
        }
    }

    trim_lsp_text_in_place(detail)
}

fn parse_signature_information(value: &Value) -> Option<LspSignatureInformation> {
    let raw_label = value.get("label")?.as_str()?;
    let label = bounded_lsp_text(raw_label, MAX_LSP_SIGNATURE_LABEL_CHARS)?;
    let parameters = if let Some(parameters) = value.get("parameters").and_then(Value::as_array) {
        let mut parsed = Vec::with_capacity(parameters.len().min(MAX_LSP_SIGNATURE_PARAMETERS));
        for parameter in parameters.iter().take(MAX_LSP_SIGNATURE_PARAMETERS) {
            if let Some(parameter) = parse_parameter_information(parameter, &label) {
                parsed.push(parameter);
            }
        }
        parsed
    } else {
        Vec::new()
    };

    Some(LspSignatureInformation {
        label,
        documentation: value
            .get("documentation")
            .and_then(hover_contents_to_text)
            .and_then(|text| {
                bounded_lsp_markdown_text(&text, MAX_LSP_SIGNATURE_DOCUMENTATION_CHARS)
            }),
        parameters,
    })
}

fn active_signature_parameter(result: &Value, active_signature: usize) -> Option<usize> {
    result
        .get("activeParameter")
        .and_then(value_as_usize)
        .or_else(|| {
            result
                .get("signatures")
                .and_then(Value::as_array)
                .and_then(|items| items.get(active_signature))
                .and_then(|signature| signature.get("activeParameter"))
                .and_then(value_as_usize)
        })
        .or(Some(0))
}

fn parse_parameter_information(
    value: &Value,
    signature_label: &str,
) -> Option<LspParameterInformation> {
    let label_value = value.get("label")?;
    let label = if let Some(label) = label_value.as_str() {
        bounded_lsp_text(label, MAX_LSP_SIGNATURE_PARAMETER_LABEL_CHARS)?
    } else {
        let range = label_value.as_array()?;
        if range.len() != 2 {
            return None;
        }
        let start = value_as_usize(range.first()?)?;
        let end = value_as_usize(range.get(1)?)?;
        if start > end {
            return None;
        }
        let label = slice_by_char_range(signature_label, start, end)
            .unwrap_or_else(|| signature_label.to_owned());
        bounded_lsp_text(&label, MAX_LSP_SIGNATURE_PARAMETER_LABEL_CHARS)?
    };

    Some(LspParameterInformation {
        label,
        documentation: value
            .get("documentation")
            .and_then(hover_contents_to_text)
            .and_then(|text| {
                bounded_lsp_markdown_text(&text, MAX_LSP_SIGNATURE_PARAMETER_DOCUMENTATION_CHARS)
            }),
    })
}

fn slice_by_char_range(text: &str, start: usize, end: usize) -> Option<String> {
    if start > end {
        return None;
    }
    let mut output = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= end {
            break;
        }
        if idx >= start {
            output.push(ch);
        }
    }
    (!output.is_empty() || start == end).then_some(output)
}

fn parse_completion_text_edit(value: &Value, document_path: &Path) -> Option<LspTextEdit> {
    let range = value.get("range").or_else(|| value.get("replace"))?;
    parse_completion_text_edit_range(value, range, document_path)
}

fn parse_completion_insert_text_edit(value: &Value, document_path: &Path) -> Option<LspTextEdit> {
    let range = value.get("insert")?;
    parse_completion_text_edit_range(value, range, document_path)
}

fn parse_completion_text_edit_range(
    value: &Value,
    range: &Value,
    document_path: &Path,
) -> Option<LspTextEdit> {
    parse_completion_text_edit_range_with_text(
        range,
        document_path,
        value.get("newText")?.as_str()?,
    )
}

fn parse_completion_text_edit_range_with_text(
    range: &Value,
    document_path: &Path,
    new_text: &str,
) -> Option<LspTextEdit> {
    let (start_line, start_column, end_line, end_column) = parse_lsp_range(range)?;
    Some(LspTextEdit {
        path: document_path.to_path_buf(),
        start_line,
        start_column,
        end_line,
        end_column,
        new_text: bounded_lsp_insert_text(new_text)?,
    })
}
