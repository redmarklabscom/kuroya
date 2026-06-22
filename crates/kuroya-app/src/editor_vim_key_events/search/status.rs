use super::state::VIM_SEARCH_INPUT;
use crate::editor_vim_key_events::EditorVimPendingKey;

const VIM_SEARCH_STATUS_QUERY_MAX_CHARS: usize = 96;

pub(crate) fn vim_pending_search_status_label(
    pending: Option<EditorVimPendingKey>,
) -> Option<String> {
    let EditorVimPendingKey::SearchInput { forward, .. } = pending? else {
        return None;
    };

    Some(VIM_SEARCH_INPUT.with(|input| {
        let query = input.borrow();
        let mut label = String::with_capacity(
            1 + query
                .len()
                .min(VIM_SEARCH_STATUS_QUERY_MAX_CHARS * 4 + "...".len()),
        );
        label.push(if forward { '/' } else { '?' });
        push_vim_search_status_query(&mut label, &query);
        label
    }))
}

fn push_vim_search_status_query(label: &mut String, query: &str) {
    let mut chars = query.chars();
    for _ in 0..VIM_SEARCH_STATUS_QUERY_MAX_CHARS {
        let Some(ch) = chars.next() else {
            return;
        };
        label.push(ch);
    }
    if chars.next().is_some() {
        label.push_str("...");
    }
}
