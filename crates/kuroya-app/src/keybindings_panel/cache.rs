use eframe::egui::{Context, Id};
use kuroya_core::keymap::KeyBinding;
use std::sync::Arc;

use super::{
    KeybindingPanelItem,
    item::sanitized_keybinding_items,
    query::{keybinding_query_terms, keybinding_search_text_matches_terms},
};

const KEYBINDINGS_PANEL_CACHE_ID: &str = "kuroya.keybindings_panel.items_cache";

#[derive(Clone, Default)]
pub(in crate::keybindings_panel) struct KeybindingsPanelItemsCache {
    pub(in crate::keybindings_panel) bindings_valid: bool,
    pub(in crate::keybindings_panel) filtered_valid: bool,
    pub(in crate::keybindings_panel) query: String,
    pub(in crate::keybindings_panel) bindings: Vec<KeyBinding>,
    pub(in crate::keybindings_panel) sanitized_items: Arc<Vec<KeybindingPanelItem>>,
    pub(in crate::keybindings_panel) items: Arc<Vec<KeybindingPanelItem>>,
}

impl KeybindingsPanelItemsCache {
    pub(in crate::keybindings_panel) fn items_for(
        &mut self,
        bindings: &[KeyBinding],
        query: &str,
    ) -> Arc<Vec<KeybindingPanelItem>> {
        if !self.bindings_match(bindings) {
            self.bindings_valid = true;
            self.filtered_valid = false;
            self.bindings.clear();
            self.bindings.extend_from_slice(bindings);
            self.sanitized_items = Arc::new(sanitized_keybinding_items(bindings));
        }

        if !self.filtered_match(query) {
            let next_items = if query.is_empty() {
                Arc::clone(&self.sanitized_items)
            } else {
                let source_items = if self.can_refine_previous_filter(query) {
                    Arc::clone(&self.items)
                } else {
                    Arc::clone(&self.sanitized_items)
                };
                match filter_keybinding_items_for_cache(source_items.as_slice(), query) {
                    FilteredKeybindingItems::All => source_items,
                    FilteredKeybindingItems::Filtered(filtered_items) => Arc::new(filtered_items),
                }
            };
            self.filtered_valid = true;
            self.query.clear();
            self.query.push_str(query);
            self.items = next_items;
        }
        Arc::clone(&self.items)
    }

    #[cfg(test)]
    pub(in crate::keybindings_panel) fn matches(
        &self,
        bindings: &[KeyBinding],
        query: &str,
    ) -> bool {
        self.bindings_match(bindings) && self.filtered_match(query)
    }

    fn bindings_match(&self, bindings: &[KeyBinding]) -> bool {
        self.bindings_valid && self.bindings == bindings
    }

    fn filtered_match(&self, query: &str) -> bool {
        self.filtered_valid && self.query == query
    }

    pub(in crate::keybindings_panel) fn can_refine_previous_filter(&self, query: &str) -> bool {
        self.filtered_valid && !self.query.is_empty() && query.starts_with(self.query.as_str())
    }

    #[cfg(test)]
    pub(in crate::keybindings_panel) fn filter_source_for(
        &self,
        query: &str,
    ) -> &[KeybindingPanelItem] {
        if self.can_refine_previous_filter(query) {
            &self.items
        } else {
            &self.sanitized_items
        }
    }
}

pub(in crate::keybindings_panel) fn cached_keybinding_items(
    ctx: &Context,
    bindings: &[KeyBinding],
    query: &str,
) -> Arc<Vec<KeybindingPanelItem>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<KeybindingsPanelItemsCache>(Id::new(
            KEYBINDINGS_PANEL_CACHE_ID,
        ))
        .items_for(bindings, query)
    })
}

#[cfg(test)]
pub(in crate::keybindings_panel) fn filter_keybinding_items(
    items: &[KeybindingPanelItem],
    query: &str,
) -> Vec<KeybindingPanelItem> {
    match filter_keybinding_items_for_cache(items, query) {
        FilteredKeybindingItems::All => items.to_vec(),
        FilteredKeybindingItems::Filtered(filtered) => filtered,
    }
}

enum FilteredKeybindingItems {
    All,
    Filtered(Vec<KeybindingPanelItem>),
}

fn filter_keybinding_items_for_cache(
    items: &[KeybindingPanelItem],
    query: &str,
) -> FilteredKeybindingItems {
    let terms = keybinding_query_terms(query);
    if terms.is_empty() {
        return FilteredKeybindingItems::All;
    }

    let mut filtered: Option<Vec<KeybindingPanelItem>> = None;
    for (index, item) in items.iter().enumerate() {
        if keybinding_search_text_matches_terms(&item.search_text, terms.as_slice()) {
            if let Some(filtered) = filtered.as_mut() {
                filtered.push(item.clone());
            }
        } else if filtered.is_none() {
            let mut kept = Vec::with_capacity(items.len());
            kept.extend(items[..index].iter().cloned());
            filtered = Some(kept);
        }
    }

    match filtered {
        Some(filtered) => FilteredKeybindingItems::Filtered(filtered),
        None => FilteredKeybindingItems::All,
    }
}
