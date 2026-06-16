use crate::large_file_mode::buffer_uses_large_file_mode;
use kuroya_core::{BufferId, LanguageId, LspFoldingRange, TextBuffer};
use std::{
    collections::{HashMap, VecDeque, hash_map::Entry},
    hash::{Hash, Hasher},
    ops::Range,
};
use tree_sitter::{InputEdit, Language, Node, Parser, Point, Tree};

#[path = "syntax_tree_cache/rust_folding.rs"]
mod rust_folding;

use self::rust_folding::rust_tree_folding_ranges;
#[cfg(test)]
pub(crate) use self::rust_folding::{
    collect_rust_comment_ranges, collect_rust_use_declaration_ranges,
};

const MAX_SYNTAX_TREE_CACHES: usize = 16;
pub(crate) const SYNTAX_TREE_MAX_BYTES: usize = 512 * 1024;
pub(crate) const SYNTAX_TREE_MAX_LINES: usize = 20_000;
const TREE_SITTER_FOLDING_RANGE_LIMIT: usize = 1_000;
const TREE_SITTER_FOLDING_CANDIDATE_LIMIT: usize = TREE_SITTER_FOLDING_RANGE_LIMIT * 2;
const TREE_SITTER_INJECTION_LIMIT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TreeSitterInjection {
    pub(crate) language: LanguageId,
    pub(crate) range: Range<usize>,
}

#[derive(Default)]
pub(crate) struct TreeSitterSyntaxCache {
    parsers: HashMap<LanguageId, Parser>,
    trees: HashMap<SyntaxTreeCacheKey, SyntaxTreeCacheEntry>,
    order: VecDeque<SyntaxTreeCacheKey>,
    #[cfg(test)]
    incremental_reparse_count: usize,
    #[cfg(test)]
    folding_compute_count: usize,
    #[cfg(test)]
    injection_compute_count: usize,
}

#[derive(Clone, Copy)]
struct TreeSitterLanguageAdapter {
    language: LanguageId,
    parser_language: fn() -> Language,
    folding_ranges: fn(&Tree) -> Vec<LspFoldingRange>,
    selection_expansion: fn(&Tree, Range<usize>) -> Option<Range<usize>>,
    newline_indent_override: fn(&Tree, &TextBuffer, usize, &str) -> Option<String>,
    injections: fn(&Tree, &str) -> Vec<TreeSitterByteInjection>,
}

const TREE_SITTER_LANGUAGE_ADAPTERS: &[TreeSitterLanguageAdapter] = &[TreeSitterLanguageAdapter {
    language: LanguageId::Rust,
    parser_language: rust_tree_sitter_language,
    folding_ranges: rust_tree_folding_ranges,
    selection_expansion: rust_tree_selection_expansion,
    newline_indent_override: rust_tree_newline_indent_override,
    injections: rust_tree_injections,
}];

struct SyntaxTreeCacheEntry {
    tree: Tree,
    text: String,
    cached_folding_ranges: Option<Vec<LspFoldingRange>>,
    cached_injections: Option<Vec<TreeSitterInjection>>,
}

impl SyntaxTreeCacheEntry {
    fn uncached(tree: Tree, text: String) -> Self {
        Self {
            tree,
            text,
            cached_folding_ranges: None,
            cached_injections: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq)]
struct SyntaxTreeCacheKey {
    buffer_id: BufferId,
    version: u64,
    len_chars: usize,
    len_bytes: usize,
    len_lines: usize,
    language: LanguageId,
}

impl PartialEq for SyntaxTreeCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.buffer_id == other.buffer_id
            && self.version == other.version
            && self.len_chars == other.len_chars
            && self.len_bytes == other.len_bytes
            && self.len_lines == other.len_lines
            && self.language == other.language
    }
}

impl Hash for SyntaxTreeCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.buffer_id.hash(state);
        self.version.hash(state);
        self.len_chars.hash(state);
        self.len_bytes.hash(state);
        self.len_lines.hash(state);
        self.language.hash(state);
    }
}

impl TreeSitterSyntaxCache {
    pub(crate) fn clear(&mut self) {
        self.trees.clear();
        self.order.clear();
        #[cfg(test)]
        {
            self.incremental_reparse_count = 0;
            self.folding_compute_count = 0;
            self.injection_compute_count = 0;
        }
    }

    pub(crate) fn clear_for_buffer(&mut self, buffer_id: BufferId) {
        self.remove_entries_for_buffer(buffer_id);
    }

    pub(crate) fn folding_ranges_for_buffer(
        &mut self,
        buffer: &TextBuffer,
    ) -> Option<Vec<LspFoldingRange>> {
        let key = self.cache_key_for_buffer(buffer)?;
        let adapter = tree_sitter_adapter_for_language(key.language)?;
        let line_count = buffer.len_lines();
        #[cfg(test)]
        let mut computed = false;
        let ranges = {
            let entry = self.tree_entry_for_buffer(buffer, key, adapter)?;
            if entry
                .cached_folding_ranges
                .as_ref()
                .is_some_and(|ranges| !folding_ranges_fit_buffer(ranges, line_count))
            {
                entry.cached_folding_ranges = None;
            }
            if entry.cached_folding_ranges.is_none() {
                let mut ranges = (adapter.folding_ranges)(&entry.tree);
                retain_valid_folding_ranges(&mut ranges, line_count);
                entry.cached_folding_ranges = Some(ranges);
                #[cfg(test)]
                {
                    computed = true;
                }
            }
            entry.cached_folding_ranges.clone()
        };
        #[cfg(test)]
        if computed {
            self.folding_compute_count += 1;
        }
        ranges
    }

    pub(crate) fn selection_expansion_for_buffer(
        &mut self,
        buffer: &TextBuffer,
        range: Range<usize>,
    ) -> Option<Range<usize>> {
        let key = self.cache_key_for_buffer(buffer)?;
        let char_range = bounded_char_range_for_buffer(buffer, range)?;
        let adapter = tree_sitter_adapter_for_language(key.language)?;
        let byte_range = buffer.char_to_byte(char_range.start)..buffer.char_to_byte(char_range.end);
        let tree = self.tree_for_buffer(buffer, key, adapter)?;
        let expanded = (adapter.selection_expansion)(tree, byte_range)?;
        let expanded = buffer.byte_to_char(expanded.start)..buffer.byte_to_char(expanded.end);
        range_strictly_expands(&expanded, &char_range).then_some(expanded)
    }

    pub(crate) fn newline_indent_overrides_for_buffer(
        &mut self,
        buffer: &TextBuffer,
        indent_unit: &str,
    ) -> Option<Vec<Option<String>>> {
        if indent_unit.is_empty() {
            return None;
        }

        let key = self.cache_key_for_buffer(buffer)?;
        let adapter = tree_sitter_adapter_for_language(key.language)?;
        let tree = self.tree_for_buffer(buffer, key, adapter)?;
        let overrides = buffer
            .selections()
            .iter()
            .map(|selection| {
                (adapter.newline_indent_override)(tree, buffer, selection.cursor, indent_unit)
            })
            .collect::<Vec<_>>();
        overrides.iter().any(Option::is_some).then_some(overrides)
    }

    pub(crate) fn injections_for_buffer(
        &mut self,
        buffer: &TextBuffer,
    ) -> Option<Vec<TreeSitterInjection>> {
        let key = self.cache_key_for_buffer(buffer)?;
        let adapter = tree_sitter_adapter_for_language(key.language)?;
        #[cfg(test)]
        let mut computed = false;
        let injections = {
            let len_chars = buffer.len_chars();
            let entry = self.tree_entry_for_buffer(buffer, key, adapter)?;
            if entry
                .cached_injections
                .as_ref()
                .is_some_and(|injections| !syntax_injections_fit_buffer(injections, len_chars))
            {
                entry.cached_injections = None;
            }
            if entry.cached_injections.is_none() {
                let mut entry_injections = tree_sitter_injections_for_entry(buffer, adapter, entry);
                retain_valid_syntax_injections(&mut entry_injections, len_chars);
                entry.cached_injections = Some(entry_injections);
                #[cfg(test)]
                {
                    computed = true;
                }
            }
            entry
                .cached_injections
                .as_ref()
                .filter(|injections| !injections.is_empty())
                .cloned()
        };
        #[cfg(test)]
        if computed {
            self.injection_compute_count += 1;
        }
        injections
    }

    fn cache_key_for_buffer(&mut self, buffer: &TextBuffer) -> Option<SyntaxTreeCacheKey> {
        let key = syntax_tree_cache_key(buffer);
        if key.is_none() {
            self.remove_entries_for_buffer(buffer.id());
        }
        key
    }

    fn tree_for_buffer(
        &mut self,
        buffer: &TextBuffer,
        key: SyntaxTreeCacheKey,
        adapter: TreeSitterLanguageAdapter,
    ) -> Option<&Tree> {
        self.tree_entry_for_buffer(buffer, key, adapter)
            .map(|entry| &entry.tree)
    }

    fn tree_entry_for_buffer(
        &mut self,
        buffer: &TextBuffer,
        key: SyntaxTreeCacheKey,
        adapter: TreeSitterLanguageAdapter,
    ) -> Option<&mut SyntaxTreeCacheEntry> {
        if let Some(entry) = self.trees.get(&key) {
            if buffer.text_equals(&entry.text) {
                self.refresh_tree_order(key);
                return self.trees.get_mut(&key);
            }

            self.remove_tree_entry(key);
        }

        let text = buffer.text();
        let Some(entry) = self.parse_tree_entry_for_buffer(key, &text, adapter) else {
            self.remove_entries_for_buffer(buffer.id());
            return None;
        };
        self.insert_tree_entry(key, entry);

        self.trees.get_mut(&key)
    }

    fn parse_tree_entry_for_buffer(
        &mut self,
        key: SyntaxTreeCacheKey,
        text: &str,
        adapter: TreeSitterLanguageAdapter,
    ) -> Option<SyntaxTreeCacheEntry> {
        if let Some(previous) = self.latest_tree_for_buffer(key) {
            if previous.text == text {
                return Some(previous);
            }

            if let Some(edit) = syntax_tree_input_edit(&previous.text, text) {
                let mut old_tree = previous.tree;
                old_tree.edit(&edit);
                let tree = self
                    .parser_for_adapter(adapter)?
                    .parse(text, Some(&old_tree))?;
                #[cfg(test)]
                {
                    self.incremental_reparse_count += 1;
                }
                return Some(SyntaxTreeCacheEntry::uncached(tree, text.to_owned()));
            }
        }

        self.parser_for_adapter(adapter)?
            .parse(text, None)
            .map(|tree| SyntaxTreeCacheEntry::uncached(tree, text.to_owned()))
    }

    fn latest_tree_for_buffer(&self, key: SyntaxTreeCacheKey) -> Option<SyntaxTreeCacheEntry> {
        self.order
            .iter()
            .rev()
            .find(|existing| {
                existing.buffer_id == key.buffer_id
                    && existing.language == key.language
                    && existing.version < key.version
            })
            .and_then(|existing| self.trees.get(existing))
            .map(|entry| SyntaxTreeCacheEntry {
                tree: entry.tree.clone(),
                text: entry.text.clone(),
                cached_folding_ranges: entry.cached_folding_ranges.clone(),
                cached_injections: entry.cached_injections.clone(),
            })
    }

    fn parser_for_adapter(&mut self, adapter: TreeSitterLanguageAdapter) -> Option<&mut Parser> {
        match self.parsers.entry(adapter.language) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(entry) => {
                let mut parser = Parser::new();
                parser.set_language(&(adapter.parser_language)()).ok()?;
                Some(entry.insert(parser))
            }
        }
    }

    #[cfg(test)]
    fn insert_tree(&mut self, key: SyntaxTreeCacheKey, tree: Tree, text: String) {
        self.insert_tree_entry(key, SyntaxTreeCacheEntry::uncached(tree, text));
    }

    fn insert_tree_entry(&mut self, key: SyntaxTreeCacheKey, entry: SyntaxTreeCacheEntry) {
        self.remove_stale_entries_for_buffer(key);
        self.trees.insert(key, entry);
        self.order.push_back(key);
        while self.order.len() > MAX_SYNTAX_TREE_CACHES {
            if let Some(oldest) = self.order.pop_front() {
                self.trees.remove(&oldest);
            }
        }
    }

    fn remove_entries_for_buffer(&mut self, buffer_id: BufferId) {
        self.order
            .retain(|existing| existing.buffer_id != buffer_id);
        self.trees
            .retain(|existing, _| existing.buffer_id != buffer_id);
    }

    fn remove_tree_entry(&mut self, key: SyntaxTreeCacheKey) {
        self.order.retain(|existing| *existing != key);
        self.trees.remove(&key);
    }

    fn remove_stale_entries_for_buffer(&mut self, key: SyntaxTreeCacheKey) {
        self.order
            .retain(|existing| existing.buffer_id != key.buffer_id);
        self.trees
            .retain(|existing, _| existing.buffer_id != key.buffer_id);
    }

    fn refresh_tree_order(&mut self, key: SyntaxTreeCacheKey) {
        if self.order.back() == Some(&key) {
            return;
        }
        self.order.retain(|existing| *existing != key);
        self.order.push_back(key);
    }

    #[cfg(test)]
    fn cached_tree_count(&self) -> usize {
        self.trees.len()
    }

    #[cfg(test)]
    pub(crate) fn contains_buffer_for_test(&self, buffer_id: BufferId) -> bool {
        self.trees.keys().any(|key| key.buffer_id == buffer_id)
    }

    #[cfg(test)]
    fn incremental_reparse_count(&self) -> usize {
        self.incremental_reparse_count
    }

    #[cfg(test)]
    fn folding_compute_count(&self) -> usize {
        self.folding_compute_count
    }

    #[cfg(test)]
    fn injection_compute_count(&self) -> usize {
        self.injection_compute_count
    }
}

fn tree_sitter_injections_for_entry(
    buffer: &TextBuffer,
    adapter: TreeSitterLanguageAdapter,
    entry: &SyntaxTreeCacheEntry,
) -> Vec<TreeSitterInjection> {
    let mut injections = (adapter.injections)(&entry.tree, &entry.text)
        .into_iter()
        .filter(|injection| tree_sitter_byte_injection_fits_text(injection, &entry.text))
        .filter_map(|injection| {
            let start = buffer.byte_to_char(injection.range.start);
            let end = buffer.byte_to_char(injection.range.end);
            (start < end).then_some(TreeSitterInjection {
                language: injection.language,
                range: start..end,
            })
        })
        .collect::<Vec<_>>();
    retain_valid_syntax_injections(&mut injections, buffer.len_chars());
    injections
}

fn tree_sitter_byte_injection_fits_text(injection: &TreeSitterByteInjection, text: &str) -> bool {
    injection.range.start < injection.range.end
        && injection.range.end <= text.len()
        && text.is_char_boundary(injection.range.start)
        && text.is_char_boundary(injection.range.end)
}

fn syntax_tree_cache_key(buffer: &TextBuffer) -> Option<SyntaxTreeCacheKey> {
    buffer_allows_syntax_tree_cache(buffer).then(|| SyntaxTreeCacheKey {
        buffer_id: buffer.id(),
        version: buffer.version(),
        len_chars: buffer.len_chars(),
        len_bytes: buffer.len_bytes(),
        len_lines: buffer.len_lines(),
        language: buffer.language(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyntaxTreeUnavailableReason {
    UnsupportedLanguage(LanguageId),
    LargeFileMode,
    ParseByteBudget { bytes: usize, max_bytes: usize },
    ParseLineBudget { lines: usize, max_lines: usize },
}

pub(crate) fn syntax_tree_unavailable_reason(
    buffer: &TextBuffer,
) -> Option<SyntaxTreeUnavailableReason> {
    if tree_sitter_adapter_for_language(buffer.language()).is_none() {
        return Some(SyntaxTreeUnavailableReason::UnsupportedLanguage(
            buffer.language(),
        ));
    }
    if buffer_uses_large_file_mode(buffer) {
        return Some(SyntaxTreeUnavailableReason::LargeFileMode);
    }
    if buffer.len_bytes() > SYNTAX_TREE_MAX_BYTES {
        return Some(SyntaxTreeUnavailableReason::ParseByteBudget {
            bytes: buffer.len_bytes(),
            max_bytes: SYNTAX_TREE_MAX_BYTES,
        });
    }
    if buffer.len_lines() > SYNTAX_TREE_MAX_LINES {
        return Some(SyntaxTreeUnavailableReason::ParseLineBudget {
            lines: buffer.len_lines(),
            max_lines: SYNTAX_TREE_MAX_LINES,
        });
    }
    None
}

fn tree_sitter_adapter_for_language(language: LanguageId) -> Option<TreeSitterLanguageAdapter> {
    TREE_SITTER_LANGUAGE_ADAPTERS
        .iter()
        .copied()
        .find(|adapter| adapter.language == language)
}

fn rust_tree_sitter_language() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn buffer_allows_syntax_tree_cache(buffer: &TextBuffer) -> bool {
    syntax_tree_unavailable_reason(buffer).is_none()
}

fn bounded_char_range_for_buffer(buffer: &TextBuffer, range: Range<usize>) -> Option<Range<usize>> {
    if range.start > range.end {
        return None;
    }

    let len = buffer.len_chars();
    if range.end > len {
        return None;
    }

    Some(range)
}

fn folding_ranges_fit_buffer(ranges: &[LspFoldingRange], line_count: usize) -> bool {
    ranges.len() <= TREE_SITTER_FOLDING_RANGE_LIMIT
        && ranges
            .iter()
            .all(|range| folding_range_fits_buffer(range, line_count))
}

fn retain_valid_folding_ranges(ranges: &mut Vec<LspFoldingRange>, line_count: usize) {
    ranges.retain(|range| folding_range_fits_buffer(range, line_count));
    ranges.truncate(TREE_SITTER_FOLDING_RANGE_LIMIT);
}

fn folding_range_fits_buffer(range: &LspFoldingRange, line_count: usize) -> bool {
    range.start_line > 0 && range.end_line > range.start_line && range.end_line <= line_count
}

fn syntax_injections_fit_buffer(injections: &[TreeSitterInjection], len_chars: usize) -> bool {
    injections.len() <= TREE_SITTER_INJECTION_LIMIT
        && injections
            .iter()
            .all(|injection| syntax_injection_fits_buffer(injection, len_chars))
}

fn retain_valid_syntax_injections(injections: &mut Vec<TreeSitterInjection>, len_chars: usize) {
    injections.retain(|injection| syntax_injection_fits_buffer(injection, len_chars));
    injections.truncate(TREE_SITTER_INJECTION_LIMIT);
}

fn syntax_injection_fits_buffer(injection: &TreeSitterInjection, len_chars: usize) -> bool {
    injection.range.start < injection.range.end && injection.range.end <= len_chars
}

fn syntax_tree_input_edit(old_text: &str, new_text: &str) -> Option<InputEdit> {
    if old_text == new_text {
        return None;
    }

    let start_byte = common_prefix_bytes(old_text, new_text);
    let (old_suffix_bytes, new_suffix_bytes) = common_suffix_bytes(old_text, new_text, start_byte);
    let old_end_byte = old_text.len().saturating_sub(old_suffix_bytes);
    let new_end_byte = new_text.len().saturating_sub(new_suffix_bytes);

    Some(InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: point_for_byte(old_text, start_byte),
        old_end_position: point_for_byte(old_text, old_end_byte),
        new_end_position: point_for_byte(new_text, new_end_byte),
    })
}

fn common_prefix_bytes(old_text: &str, new_text: &str) -> usize {
    let mut prefix = 0usize;
    let mut old_chars = old_text.char_indices();
    let mut new_chars = new_text.char_indices();

    loop {
        match (old_chars.next(), new_chars.next()) {
            (Some((old_idx, old_ch)), Some((_, new_ch))) if old_ch == new_ch => {
                prefix = old_idx + old_ch.len_utf8();
            }
            _ => return prefix,
        }
    }
}

fn common_suffix_bytes(old_text: &str, new_text: &str, start_byte: usize) -> (usize, usize) {
    let mut old_suffix = 0usize;
    let mut new_suffix = 0usize;
    let mut old_chars = old_text[start_byte..].chars().rev();
    let mut new_chars = new_text[start_byte..].chars().rev();

    loop {
        match (old_chars.next(), new_chars.next()) {
            (Some(old_ch), Some(new_ch)) if old_ch == new_ch => {
                let old_len = old_ch.len_utf8();
                let new_len = new_ch.len_utf8();
                if start_byte + old_suffix + old_len > old_text.len()
                    || start_byte + new_suffix + new_len > new_text.len()
                {
                    return (old_suffix, new_suffix);
                }
                old_suffix += old_len;
                new_suffix += new_len;
            }
            _ => return (old_suffix, new_suffix),
        }
    }
}

fn point_for_byte(text: &str, byte_index: usize) -> Point {
    let mut row = 0usize;
    let mut column = 0usize;
    for byte in text.as_bytes().iter().take(byte_index.min(text.len())) {
        if *byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    Point { row, column }
}

fn rust_tree_selection_expansion(tree: &Tree, range: Range<usize>) -> Option<Range<usize>> {
    let root = tree.root_node();
    if root.end_byte() == 0 {
        return None;
    }
    if range.start > range.end {
        return None;
    }

    let range = range.start.min(root.end_byte())..range.end.min(root.end_byte());
    if range.start > range.end {
        return None;
    }
    let mut expansion = None;
    for lookup in rust_selection_lookup_ranges(&range, root.end_byte()) {
        if let Some(node) = root.descendant_for_byte_range(lookup.start, lookup.end)
            && let Some(candidate) = rust_selectable_ancestor_expansion(node, &range)
            && expansion
                .as_ref()
                .is_none_or(|current| rust_selection_candidate_is_tighter(&candidate, current))
        {
            expansion = Some(candidate);
        }
    }
    expansion
}

fn rust_selection_lookup_ranges(range: &Range<usize>, root_end: usize) -> Vec<Range<usize>> {
    let mut ranges = Vec::with_capacity(2);
    if range.start < range.end {
        ranges.push(range.start..range.end.min(root_end));
        return ranges;
    }

    if range.start > 0 {
        ranges.push(range.start.saturating_sub(1)..range.start);
    }
    if range.start < root_end {
        ranges.push(range.start..(range.start + 1).min(root_end));
    }
    ranges
}

fn rust_selectable_ancestor_expansion(
    mut node: Node<'_>,
    range: &Range<usize>,
) -> Option<Range<usize>> {
    loop {
        let candidate = node.byte_range();
        if rust_node_is_selectable(node) && range_strictly_expands(&candidate, range) {
            return Some(candidate);
        }
        node = node.parent()?;
    }
}

fn rust_selection_candidate_is_tighter(candidate: &Range<usize>, current: &Range<usize>) -> bool {
    candidate.end.saturating_sub(candidate.start) < current.end.saturating_sub(current.start)
}

fn rust_node_is_selectable(node: Node<'_>) -> bool {
    node.is_named() && !node.is_error() && !node.is_missing()
}

fn rust_tree_newline_indent_override(
    tree: &Tree,
    buffer: &TextBuffer,
    cursor: usize,
    indent_unit: &str,
) -> Option<String> {
    let root = tree.root_node();
    if root.end_byte() == 0 {
        return None;
    }

    let cursor = cursor.min(buffer.len_chars());
    if let Some(indent) = rust_match_arm_arrow_indent_override(tree, buffer, cursor, indent_unit) {
        return Some(indent);
    }

    let cursor_line = buffer.char_position(cursor).line;
    let lookup_byte = buffer.char_to_byte(cursor).min(root.end_byte());
    let lookup_start = if lookup_byte == root.end_byte() {
        lookup_byte.saturating_sub(1)
    } else {
        lookup_byte
    };
    let lookup_end = (lookup_start + 1).min(root.end_byte());
    let mut node = root.descendant_for_byte_range(lookup_start, lookup_end)?;

    loop {
        if rust_node_is_indent_scope(node) {
            let start_line = node.start_position().row;
            let end_line = node.end_position().row;
            if start_line < cursor_line && cursor_line <= end_line {
                let scope_indent = leading_indent_for_line(buffer, start_line);
                let current_indent = leading_indent_for_line(buffer, cursor_line);
                let desired_indent = format!("{scope_indent}{indent_unit}");
                if desired_indent.chars().count() > current_indent.chars().count() {
                    return Some(desired_indent);
                }
                return None;
            }
        }
        node = node.parent()?;
    }
}

fn rust_match_arm_arrow_indent_override(
    tree: &Tree,
    buffer: &TextBuffer,
    cursor: usize,
    indent_unit: &str,
) -> Option<String> {
    let position = buffer.char_position(cursor);
    let line_start = buffer.line_column_to_char(position.line, 0);
    let line_end = buffer.line_content_end_char(position.line);
    let cursor = cursor.min(line_end).max(line_start);
    let line_prefix = buffer.text_range(line_start..cursor)?;
    let trimmed = line_prefix.trim_end();
    if !trimmed.ends_with("=>") {
        return None;
    }

    let root = tree.root_node();
    let arrow_end_char = line_start + trimmed.chars().count();
    let lookup_char = arrow_end_char.saturating_sub(1);
    let lookup_byte = buffer.char_to_byte(lookup_char).min(root.end_byte());
    if !rust_lookup_is_in_match_block(root, lookup_byte, position.line) {
        return None;
    }

    let base_indent = leading_indent_for_line(buffer, position.line);
    Some(format!("{base_indent}{indent_unit}"))
}

fn rust_lookup_is_in_match_block(root: Node<'_>, lookup_byte: usize, line: usize) -> bool {
    if root.end_byte() == 0 {
        return false;
    }

    let lookup_start = if lookup_byte == root.end_byte() {
        lookup_byte.saturating_sub(1)
    } else {
        lookup_byte
    };
    let lookup_end = (lookup_start + 1).min(root.end_byte());
    if let Some(node) = root.descendant_for_byte_range(lookup_start, lookup_end)
        && rust_node_or_ancestor_is_match_block(node)
    {
        return true;
    }

    rust_tree_contains_match_block_on_line(root, line)
}

fn rust_node_or_ancestor_is_match_block(mut node: Node<'_>) -> bool {
    loop {
        if node.kind() == "match_block" {
            return true;
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        node = parent;
    }
}

fn rust_tree_contains_match_block_on_line(node: Node<'_>, line: usize) -> bool {
    let start = node.start_position().row;
    let end = node.end_position().row;
    if line < start || line > end {
        return false;
    }
    if node.kind() == "match_block" {
        return true;
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index)
            && rust_tree_contains_match_block_on_line(child, line)
        {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone)]
struct RustInjectionDirective {
    language: LanguageId,
    end_byte: usize,
    end_row: usize,
}

#[derive(Debug, Clone)]
struct RustInjectionCandidate {
    language: Option<LanguageId>,
    start_byte: usize,
    start_row: usize,
    content: Range<usize>,
}

#[derive(Debug, Clone)]
struct TreeSitterByteInjection {
    language: LanguageId,
    range: Range<usize>,
}

#[derive(Debug, Clone)]
struct RustDocCommentRange {
    range: Range<usize>,
    start_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RustDocFenceState {
    None,
    Unsupported,
    Language(LanguageId),
}

fn rust_tree_injections(tree: &Tree, text: &str) -> Vec<TreeSitterByteInjection> {
    let mut directives = Vec::new();
    let mut candidates = Vec::new();
    let mut comments = Vec::new();
    collect_rust_injection_candidates(
        tree.root_node(),
        text,
        &mut directives,
        &mut candidates,
        &mut comments,
    );
    directives.sort_by_key(|directive| directive.end_byte);
    candidates.sort_by_key(|candidate| candidate.start_byte);
    comments.sort_by_key(|comment| comment.range.start);

    let mut used_directives = vec![false; directives.len()];
    let mut injections = rust_doc_comment_fence_injections(&comments, text);
    for candidate in candidates {
        if injections.len() >= TREE_SITTER_INJECTION_LIMIT {
            break;
        }
        if candidate.content.is_empty() {
            continue;
        }

        let directive = directives
            .iter()
            .enumerate()
            .rev()
            .find(|(index, directive)| {
                !used_directives[*index]
                    && directive.end_byte <= candidate.start_byte
                    && rust_injection_directive_applies(directive, &candidate, text)
            });
        let language = if let Some((index, directive)) = directive {
            used_directives[index] = true;
            Some(directive.language)
        } else {
            candidate.language
        };
        if let Some(language) = language {
            injections.push(TreeSitterByteInjection {
                language,
                range: candidate.content,
            });
        }
    }

    injections.sort_by_key(|injection| injection.range.start);
    injections.truncate(TREE_SITTER_INJECTION_LIMIT);
    injections
}

fn collect_rust_injection_candidates(
    node: Node<'_>,
    text: &str,
    directives: &mut Vec<RustInjectionDirective>,
    candidates: &mut Vec<RustInjectionCandidate>,
    comments: &mut Vec<RustDocCommentRange>,
) {
    if directives
        .len()
        .saturating_add(candidates.len())
        .saturating_add(comments.len())
        >= TREE_SITTER_INJECTION_LIMIT * 4
    {
        return;
    }

    if rust_node_is_comment(node) {
        comments.push(RustDocCommentRange {
            range: node.byte_range(),
            start_row: node.start_position().row,
        });
        if let Some(language) = rust_comment_injection_language(node, text) {
            directives.push(RustInjectionDirective {
                language,
                end_byte: node.end_byte(),
                end_row: node.end_position().row,
            });
        }
    } else if rust_node_is_string_literal(node)
        && let Some(content) = rust_string_literal_content_range(text, node.byte_range())
    {
        candidates.push(RustInjectionCandidate {
            language: rust_macro_string_injection_language(node, text),
            start_byte: node.start_byte(),
            start_row: node.start_position().row,
            content,
        });
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_rust_injection_candidates(child, text, directives, candidates, comments);
        }
    }
}

fn rust_node_is_comment(node: Node<'_>) -> bool {
    matches!(node.kind(), "line_comment" | "block_comment")
}

fn rust_node_is_string_literal(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "string_literal" | "raw_string_literal" | "byte_string_literal" | "raw_byte_string_literal"
    )
}

fn rust_comment_injection_language(node: Node<'_>, text: &str) -> Option<LanguageId> {
    let comment = text.get(node.byte_range())?;
    injection_language_from_comment(comment)
}

fn injection_language_from_comment(comment: &str) -> Option<LanguageId> {
    let lower = comment.to_ascii_lowercase();
    ["language", "lang", "inject"]
        .iter()
        .find_map(|key| injection_label_after_key(&lower, key))
        .and_then(language_id_from_injection_label)
}

fn injection_label_after_key(text: &str, key: &str) -> Option<String> {
    let key_start = text.find(key)?;
    let key_end = key_start + key.len();
    if key_start > 0
        && text[..key_start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }

    let label = text[key_end..]
        .trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '=' | '-' | '>'))
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(*ch, '_' | '-'))
        .collect::<String>();
    (!label.is_empty()).then_some(label)
}

fn language_id_from_injection_label(label: String) -> Option<LanguageId> {
    match label.trim_matches(|ch: char| ch == '`' || ch == '"' || ch == '\'') {
        "rs" | "rust" => Some(LanguageId::Rust),
        "toml" => Some(LanguageId::Toml),
        "json" | "jsonc" => Some(LanguageId::Json),
        "sql" => Some(LanguageId::Sql),
        "md" | "markdown" => Some(LanguageId::Markdown),
        "ps1" | "powershell" | "pwsh" => Some(LanguageId::PowerShell),
        "py" | "python" => Some(LanguageId::Python),
        "ts" | "tsx" | "typescript" => Some(LanguageId::TypeScript),
        "js" | "jsx" | "mjs" | "cjs" | "javascript" => Some(LanguageId::JavaScript),
        "css" | "scss" | "sass" | "less" => Some(LanguageId::Css),
        "html" | "htm" | "xhtml" => Some(LanguageId::Html),
        "yaml" | "yml" => Some(LanguageId::Yaml),
        "go" | "golang" => Some(LanguageId::Go),
        "java" => Some(LanguageId::Java),
        "c" => Some(LanguageId::C),
        "cc" | "cpp" | "cxx" | "c++" => Some(LanguageId::Cpp),
        "cs" | "csharp" => Some(LanguageId::CSharp),
        "sh" | "bash" | "zsh" | "shell" | "shellscript" => Some(LanguageId::Shell),
        "diff" | "patch" => Some(LanguageId::Diff),
        "txt" | "text" | "plaintext" | "plain-text" => Some(LanguageId::PlainText),
        _ => None,
    }
}

fn rust_doc_comment_fence_injections(
    comments: &[RustDocCommentRange],
    text: &str,
) -> Vec<TreeSitterByteInjection> {
    let mut injections = Vec::new();
    let mut line_state = RustDocFenceState::None;
    let mut previous_doc_line = None;

    for comment in comments {
        let Some(comment_text) = text.get(comment.range.clone()) else {
            line_state = RustDocFenceState::None;
            previous_doc_line = None;
            continue;
        };

        if let Some(content_range) =
            rust_line_doc_comment_content_range(comment_text, comment.range.start)
        {
            if previous_doc_line.is_some_and(|row| comment.start_row != row + 1) {
                line_state = RustDocFenceState::None;
            }
            previous_doc_line = Some(comment.start_row);
            collect_rust_doc_comment_line_injection(
                text,
                content_range,
                &mut line_state,
                &mut injections,
            );
        } else {
            line_state = RustDocFenceState::None;
            previous_doc_line = None;
            if let Some(injection_count) = collect_rust_block_doc_comment_fence_injections(
                text,
                comment_text,
                comment.range.start,
                &mut injections,
            ) && injection_count == 0
            {
                continue;
            }
        }

        if injections.len() >= TREE_SITTER_INJECTION_LIMIT {
            break;
        }
    }

    injections
}

fn rust_line_doc_comment_content_range(comment: &str, byte_start: usize) -> Option<Range<usize>> {
    let is_doc_comment =
        comment.starts_with("//!") || (comment.starts_with("///") && !comment.starts_with("////"));
    if !is_doc_comment {
        return None;
    }
    let prefix_len = 3;
    let mut start = prefix_len;
    if comment[start..].starts_with(' ') {
        start += 1;
    }
    let end = comment.trim_end_matches(['\r', '\n']).len();
    Some(byte_start + start..byte_start + end)
}

fn collect_rust_block_doc_comment_fence_injections(
    text: &str,
    comment: &str,
    byte_start: usize,
    injections: &mut Vec<TreeSitterByteInjection>,
) -> Option<usize> {
    let is_doc_comment =
        (comment.starts_with("/**") && !comment.starts_with("/***")) || comment.starts_with("/*!");
    if !is_doc_comment {
        return None;
    }
    let body_start = 3;
    let body_end = comment.rfind("*/")?;
    if body_end < body_start {
        return Some(0);
    }

    let before = injections.len();
    let mut state = RustDocFenceState::None;
    let body = &comment[body_start..body_end];
    let mut offset = 0usize;
    for segment in body.split_inclusive('\n') {
        let line_len = segment.trim_end_matches(['\r', '\n']).len();
        let line = &segment[..line_len];
        let line_start = byte_start + body_start + offset;
        let content = rust_block_doc_comment_line_content(line, line_start);
        collect_rust_doc_comment_line_injection(text, content, &mut state, injections);
        offset += segment.len();
        if injections.len() >= TREE_SITTER_INJECTION_LIMIT {
            break;
        }
    }

    Some(injections.len().saturating_sub(before))
}

fn rust_block_doc_comment_line_content(line: &str, byte_start: usize) -> Range<usize> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let mut start = line.len().saturating_sub(trimmed.len());
    if trimmed.starts_with('*') {
        start += 1;
        if line[start..].starts_with(' ') {
            start += 1;
        }
    }
    byte_start + start..byte_start + line.len()
}

fn collect_rust_doc_comment_line_injection(
    text: &str,
    content_range: Range<usize>,
    state: &mut RustDocFenceState,
    injections: &mut Vec<TreeSitterByteInjection>,
) {
    let Some(content) = text.get(content_range.clone()) else {
        *state = RustDocFenceState::None;
        return;
    };

    if let Some(language) = rust_doc_code_fence_language(content) {
        *state = match *state {
            RustDocFenceState::None => language
                .map(RustDocFenceState::Language)
                .unwrap_or(RustDocFenceState::Unsupported),
            RustDocFenceState::Unsupported | RustDocFenceState::Language(_) => {
                RustDocFenceState::None
            }
        };
        return;
    }

    if let RustDocFenceState::Language(language) = *state {
        let visible = trim_rust_doc_injection_range(text, content_range);
        if !visible.is_empty() {
            injections.push(TreeSitterByteInjection {
                language,
                range: visible,
            });
        }
    }
}

fn rust_doc_code_fence_language(line: &str) -> Option<Option<LanguageId>> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("```")
        .or_else(|| trimmed.strip_prefix("~~~"))?;
    let label = rest.split_whitespace().next().unwrap_or_default();
    if label.is_empty() {
        return Some(None);
    }
    Some(language_id_from_injection_label(label.to_ascii_lowercase()))
}

fn trim_rust_doc_injection_range(text: &str, range: Range<usize>) -> Range<usize> {
    let Some(content) = text.get(range.clone()) else {
        return range.start..range.start;
    };
    let leading = content.len().saturating_sub(content.trim_start().len());
    let trailing = content.len().saturating_sub(content.trim_end().len());
    range.start + leading..range.end.saturating_sub(trailing)
}

fn rust_string_literal_content_range(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let literal = text.get(range.clone())?;
    let content_start = literal.find('"')? + 1;
    let content_end = literal.rfind('"')?;
    (content_start <= content_end).then(|| range.start + content_start..range.start + content_end)
}

fn rust_macro_string_injection_language(node: Node<'_>, text: &str) -> Option<LanguageId> {
    let mut parent = node.parent();
    while let Some(candidate) = parent {
        if candidate.kind() == "macro_invocation" {
            let invocation = text.get(candidate.start_byte()..node.start_byte())?;
            return rust_macro_injection_language(invocation);
        }
        parent = candidate.parent();
    }
    None
}

fn rust_macro_injection_language(invocation_prefix: &str) -> Option<LanguageId> {
    let compact = invocation_prefix
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if compact.ends_with("query!(")
        || compact.ends_with("query_as!(")
        || compact.ends_with("query_scalar!(")
        || compact.ends_with("query_file!(")
        || compact.ends_with("query_file_as!(")
    {
        return Some(LanguageId::Sql);
    }
    None
}

fn rust_injection_directive_applies(
    directive: &RustInjectionDirective,
    candidate: &RustInjectionCandidate,
    text: &str,
) -> bool {
    if directive.end_byte > candidate.start_byte {
        return false;
    }
    let between = text
        .get(directive.end_byte..candidate.start_byte)
        .unwrap_or_default();
    between.chars().all(char::is_whitespace) || directive.end_row + 1 == candidate.start_row
}

fn rust_node_is_indent_scope(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "block"
            | "match_block"
            | "declaration_list"
            | "field_declaration_list"
            | "ordered_field_declaration_list"
            | "enum_variant_list"
            | "parameters"
            | "arguments"
            | "array_expression"
            | "tuple_expression"
    )
}

fn leading_indent_for_line(buffer: &TextBuffer, line: usize) -> String {
    buffer
        .line(line)
        .unwrap_or_default()
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

fn range_strictly_expands(candidate: &Range<usize>, range: &Range<usize>) -> bool {
    candidate.start <= range.start
        && candidate.end >= range.end
        && (candidate.start < range.start || candidate.end > range.end)
}

#[cfg(test)]
#[path = "syntax_tree_cache/tests.rs"]
mod tests;
