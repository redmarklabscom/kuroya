use super::*;

#[test]
fn vim_search_highlight_ranges_follow_stored_search() {
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
    let buffer = TextBuffer::from_text(11, None, "alpha alphabet alpha beta".to_owned());

    vim_set_last_search(&buffer, "alpha", true, true);
    assert_eq!(
        vim_search_highlight_ranges_for_buffer(&buffer),
        vec![0..5, 15..20]
    );

    vim_set_last_search(&buffer, "alpha", true, false);
    assert_eq!(
        vim_search_highlight_ranges_for_buffer(&buffer),
        vec![0..5, 6..11, 15..20]
    );

    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
}

#[test]
fn vim_search_highlight_ranges_keep_paint_ranges_non_overlapping() {
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
    let buffer = TextBuffer::from_text(12, None, "aaa".to_owned());

    vim_set_last_search(&buffer, "aa", true, false);

    assert_eq!(vim_search_highlight_ranges_for_buffer(&buffer), vec![0..2]);
    VIM_SEARCHES.with(|searches| searches.borrow_mut().clear());
}

#[test]
fn vim_pending_search_status_label_names_direction_and_bounds_query() {
    VIM_SEARCH_INPUT.with(|input| *input.borrow_mut() = "needle".to_owned());
    assert_eq!(
        vim_pending_search_status_label(Some(EditorVimPendingKey::SearchInput {
            count: 1,
            forward: true,
        })),
        Some("/needle".to_owned())
    );
    assert_eq!(
        vim_pending_search_status_label(Some(EditorVimPendingKey::SearchInput {
            count: 1,
            forward: false,
        })),
        Some("?needle".to_owned())
    );
    assert_eq!(
        vim_pending_search_status_label(Some(EditorVimPendingKey::Go(None))),
        None
    );

    VIM_SEARCH_INPUT.with(|input| *input.borrow_mut() = "x".repeat(140));
    let label = vim_pending_search_status_label(Some(EditorVimPendingKey::SearchInput {
        count: 1,
        forward: true,
    }))
    .expect("pending search status label");

    assert!(label.starts_with('/'));
    assert!(label.ends_with("..."));
    assert!(label.chars().count() < 120);
    VIM_SEARCH_INPUT.with(|input| input.borrow_mut().clear());
}
