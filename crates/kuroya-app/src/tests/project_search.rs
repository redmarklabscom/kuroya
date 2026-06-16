use crate::project_search_state::{
    MAX_PROJECT_SEARCH_GLOB_CHARS, MAX_PROJECT_SEARCH_GLOBS, MAX_PROJECT_SEARCH_RECENT_LABEL_CHARS,
    MAX_PROJECT_SEARCH_RECENT_QUERIES, MAX_PROJECT_SEARCH_RECENT_QUERY_CHARS, ProjectSearchQuery,
    next_project_search_request_id, normalize_recent_project_searches, parse_project_globs,
    project_search_query_record, project_search_recent_label, project_search_request_is_current,
    project_search_result_is_current, record_recent_project_search,
    record_recent_project_search_from_parsed_globs,
};
use std::collections::VecDeque;

#[test]
fn project_search_result_currency_tracks_query_and_options() {
    let include = vec!["src/**/*.rs".to_owned()];
    let exclude = vec!["target/**".to_owned()];

    assert!(project_search_result_is_current(
        "needle", 3, 3, true, false, &include, &exclude, "needle", true, false, &include, &exclude,
    ));
    assert!(!project_search_result_is_current(
        "needle", 2, 3, true, false, &include, &exclude, "needle", true, false, &include, &exclude,
    ));
    assert!(!project_search_result_is_current(
        "needle", 3, 3, true, false, &include, &exclude, "needle", false, false, &include,
        &exclude,
    ));
    assert!(!project_search_result_is_current(
        "needle", 3, 3, true, false, &include, &exclude, "", true, false, &include, &exclude,
    ));
}

#[test]
fn project_search_request_currency_rejects_stale_async_results() {
    let include = vec!["src/**/*.rs".to_owned()];
    let exclude = vec!["target/**".to_owned()];

    assert!(project_search_request_is_current(
        7, 7, 4, 4, "needle", false, true, &include, &exclude, "needle", false, true, &include,
        &exclude,
    ));
    assert!(!project_search_request_is_current(
        6, 7, 4, 4, "needle", false, true, &include, &exclude, "needle", false, true, &include,
        &exclude,
    ));
    assert!(!project_search_request_is_current(
        7, 7, 3, 4, "needle", false, true, &include, &exclude, "needle", false, true, &include,
        &exclude,
    ));
    assert!(!project_search_request_is_current(
        7,
        7,
        4,
        4,
        "needle",
        false,
        true,
        &include,
        &exclude,
        "needle",
        false,
        true,
        &include,
        &[]
    ));
}

#[test]
fn project_search_request_ids_wrap_without_reusing_saturated_active_id() {
    assert_eq!(next_project_search_request_id(0), 1);
    assert_eq!(next_project_search_request_id(41), 42);
    assert_eq!(next_project_search_request_id(u64::MAX), 1);

    assert!(!project_search_request_is_current(
        u64::MAX,
        next_project_search_request_id(u64::MAX),
        4,
        4,
        "needle",
        false,
        false,
        &[],
        &[],
        "needle",
        false,
        false,
        &[],
        &[],
    ));
}

#[test]
fn parses_project_glob_lists() {
    assert_eq!(
        parse_project_globs("src/**/*.rs, tests/** ; *.md\nREADME*"),
        vec!["src/**/*.rs", "tests/**", "*.md", "README*"]
    );
    assert_eq!(
        parse_project_globs("src/**, tests/**, src/**\ntests/**"),
        vec!["src/**", "tests/**"]
    );
    assert!(parse_project_globs(" , ; \n").is_empty());
}

#[test]
fn project_search_globs_are_single_line_bounded_and_limited() {
    let long = "a".repeat(MAX_PROJECT_SEARCH_GLOB_CHARS + 20);
    let mut input = format!("src/**/*.rs, {long}\0tail, src/**/*.rs");
    for index in 0..(MAX_PROJECT_SEARCH_GLOBS + 10) {
        input.push_str(&format!(", generated/{index}/**"));
    }

    let globs = parse_project_globs(&input);

    assert_eq!(globs.len(), MAX_PROJECT_SEARCH_GLOBS);
    assert_eq!(globs[0], "src/**/*.rs");
    assert!(globs[1].chars().count() <= MAX_PROJECT_SEARCH_GLOB_CHARS);
    assert!(!globs.iter().any(|glob| glob.chars().any(char::is_control)));
    assert_eq!(
        globs
            .iter()
            .filter(|glob| glob.as_str() == "src/**/*.rs")
            .count(),
        1
    );
}

#[test]
fn project_search_recent_records_are_trimmed_deduped_and_bounded() {
    let mut recent = VecDeque::new();
    let first = project_search_query_record(" needle ", true, false, " src/**/*.rs ", "").unwrap();
    let second = project_search_query_record(" other ", false, true, "", " target/** ").unwrap();

    record_recent_project_search(&mut recent, first.clone(), 2);
    record_recent_project_search(&mut recent, second.clone(), 2);
    record_recent_project_search(&mut recent, first.clone(), 2);
    record_recent_project_search(
        &mut recent,
        project_search_query_record("third", false, false, "", "").unwrap(),
        2,
    );

    assert_eq!(
        recent.into_iter().collect::<Vec<_>>(),
        vec![
            ProjectSearchQuery {
                query: "third".to_owned(),
                case_sensitive: false,
                whole_word: false,
                include: String::new(),
                exclude: String::new(),
            },
            first
        ]
    );
    assert!(project_search_query_record("  ", false, false, "", "").is_none());
    assert_eq!(
        project_search_recent_label(&second),
        "other (word, exclude)"
    );
}

#[test]
fn project_search_recent_queries_are_single_line_and_bounded() {
    let query = format!(
        "{}\nignored\tstill-visible",
        "needle".repeat(MAX_PROJECT_SEARCH_RECENT_QUERY_CHARS)
    );
    let entry = project_search_query_record(&query, false, false, "", "").unwrap();

    assert!(entry.query.chars().count() <= MAX_PROJECT_SEARCH_RECENT_QUERY_CHARS);
    assert!(!entry.query.chars().any(char::is_control));

    let label = project_search_recent_label(&ProjectSearchQuery {
        query: "needle\nsecond".to_owned(),
        case_sensitive: true,
        whole_word: false,
        include: String::new(),
        exclude: String::new(),
    });
    assert!(!label.contains('\n'));
}

#[test]
fn project_search_recent_label_sanitizes_bidi_and_bounds_display_text() {
    let label = project_search_recent_label(&ProjectSearchQuery {
        query: format!(
            "needle\n{}\u{202e}",
            "very-long-query".repeat(MAX_PROJECT_SEARCH_RECENT_LABEL_CHARS)
        ),
        case_sensitive: true,
        whole_word: true,
        include: "src/**".to_owned(),
        exclude: "target/**".to_owned(),
    });

    assert!(!label.contains('\n'));
    assert!(!label.contains('\u{202e}'));
    assert!(label.contains("..."));
    assert!(label.chars().count() <= MAX_PROJECT_SEARCH_RECENT_LABEL_CHARS);
}

#[test]
fn project_search_recent_normalizes_persisted_entries() {
    let entries = vec![
        ProjectSearchQuery {
            query: " first ".to_owned(),
            case_sensitive: false,
            whole_word: false,
            include: String::new(),
            exclude: String::new(),
        },
        ProjectSearchQuery {
            query: "first".to_owned(),
            case_sensitive: false,
            whole_word: false,
            include: String::new(),
            exclude: String::new(),
        },
        ProjectSearchQuery {
            query: "second".to_owned(),
            case_sensitive: true,
            whole_word: true,
            include: "src/**".to_owned(),
            exclude: "target/**".to_owned(),
        },
    ];

    let recent = normalize_recent_project_searches(entries, MAX_PROJECT_SEARCH_RECENT_QUERIES);

    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].query, "first");
    assert_eq!(recent[1].query, "second");
    assert!(recent[1].case_sensitive);
    assert!(recent[1].whole_word);
}

#[test]
fn project_search_recent_canonicalizes_effective_glob_drafts() {
    let mut recent = VecDeque::new();
    let no_effective_globs =
        project_search_query_record("needle", false, false, " , ; \n ", "").unwrap();
    let empty_globs = project_search_query_record("needle", false, false, "", "").unwrap();

    assert!(no_effective_globs.include.is_empty());
    record_recent_project_search(
        &mut recent,
        no_effective_globs,
        MAX_PROJECT_SEARCH_RECENT_QUERIES,
    );
    record_recent_project_search(&mut recent, empty_globs, MAX_PROJECT_SEARCH_RECENT_QUERIES);
    assert_eq!(recent.len(), 1);

    let mut persisted = normalize_recent_project_searches(
        [
            ProjectSearchQuery {
                query: "needle".to_owned(),
                case_sensitive: false,
                whole_word: false,
                include: "src/**/*.rs; tests/**/*.rs; src/**/*.rs".to_owned(),
                exclude: " target/**\n*.snap\ntarget/** ".to_owned(),
            },
            ProjectSearchQuery {
                query: "needle".to_owned(),
                case_sensitive: false,
                whole_word: false,
                include: "src/**/*.rs,tests/**/*.rs".to_owned(),
                exclude: "target/**, *.snap".to_owned(),
            },
        ],
        MAX_PROJECT_SEARCH_RECENT_QUERIES,
    );

    let entry = persisted.pop_front().expect("canonical persisted entry");
    assert!(persisted.is_empty());
    assert_eq!(entry.include, "src/**/*.rs, tests/**/*.rs");
    assert_eq!(entry.exclude, "target/**, *.snap");
}

#[test]
fn project_search_recent_records_from_already_parsed_globs() {
    let include = parse_project_globs("src/**/*.rs, src/**/*.rs; tests/**/*.rs");
    let exclude = parse_project_globs("target/**\n*.snap\ntarget/**");
    let expected = project_search_query_record(
        " needle\nvalue ",
        true,
        false,
        "src/**/*.rs, src/**/*.rs; tests/**/*.rs",
        "target/**\n*.snap\ntarget/**",
    )
    .expect("canonical project search query");
    let mut recent = VecDeque::new();

    record_recent_project_search_from_parsed_globs(
        &mut recent,
        " needle\nvalue ",
        true,
        false,
        &include,
        &exclude,
        MAX_PROJECT_SEARCH_RECENT_QUERIES,
    );
    record_recent_project_search_from_parsed_globs(
        &mut recent,
        "needle value",
        true,
        false,
        &include,
        &exclude,
        MAX_PROJECT_SEARCH_RECENT_QUERIES,
    );

    assert_eq!(recent.into_iter().collect::<Vec<_>>(), vec![expected]);
}
