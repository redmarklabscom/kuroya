use super::*;

#[test]
fn diff_algorithm_accepts_current_vs_code_variants() {
    let external: EditorSettings = toml::from_str("diff_algorithm = \"advanced-external\"\n")
        .expect("advanced-external diff algorithm should load");
    assert_eq!(external.diff_algorithm, DiffAlgorithm::AdvancedExternal);
    assert!(external.diff_algorithm.uses_advanced_diff());

    let wasm: EditorSettings = toml::from_str("diff_algorithm = \"advanced-wasm\"\n")
        .expect("advanced-wasm diff algorithm should load");
    assert_eq!(wasm.diff_algorithm, DiffAlgorithm::AdvancedWasm);
    assert!(wasm.diff_algorithm.uses_advanced_diff());

    let legacy: EditorSettings =
        toml::from_str("diff_algorithm = \"legacy\"\n").expect("legacy diff should load");
    assert!(!legacy.diff_algorithm.uses_advanced_diff());
}

#[test]
fn scm_diff_decorations_modes_map_to_visible_surfaces() {
    assert!(ScmDiffDecorations::All.show_gutter());
    assert!(ScmDiffDecorations::All.show_overview());
    assert!(ScmDiffDecorations::All.show_minimap());

    assert!(ScmDiffDecorations::Gutter.show_gutter());
    assert!(!ScmDiffDecorations::Gutter.show_overview());
    assert!(!ScmDiffDecorations::Gutter.show_minimap());

    assert!(!ScmDiffDecorations::Overview.show_gutter());
    assert!(ScmDiffDecorations::Overview.show_overview());
    assert!(!ScmDiffDecorations::Overview.show_minimap());

    assert!(!ScmDiffDecorations::Minimap.show_gutter());
    assert!(!ScmDiffDecorations::Minimap.show_overview());
    assert!(ScmDiffDecorations::Minimap.show_minimap());

    assert!(!ScmDiffDecorations::None.show_gutter());
    assert!(!ScmDiffDecorations::None.show_overview());
    assert!(!ScmDiffDecorations::None.show_minimap());
}

#[test]
fn scm_diff_decorations_gutter_width_is_clamped_to_glyph_margin_range() {
    assert_eq!(
        clamp_scm_diff_decorations_gutter_width(0),
        MIN_SCM_DIFF_DECORATIONS_GUTTER_WIDTH
    );
    assert_eq!(clamp_scm_diff_decorations_gutter_width(3), 3);
    assert_eq!(clamp_scm_diff_decorations_gutter_width(5), 5);
    assert_eq!(
        clamp_scm_diff_decorations_gutter_width(usize::MAX),
        MAX_SCM_DIFF_DECORATIONS_GUTTER_WIDTH
    );
}

#[test]
fn scm_diff_decorations_ignore_trim_whitespace_matches_vs_code_union() {
    assert!(ScmDiffDecorationsIgnoreTrimWhitespace::True.resolve(false));
    assert!(!ScmDiffDecorationsIgnoreTrimWhitespace::False.resolve(true));
    assert!(ScmDiffDecorationsIgnoreTrimWhitespace::Inherit.resolve(true));
    assert!(!ScmDiffDecorationsIgnoreTrimWhitespace::Inherit.resolve(false));

    let legacy: EditorSettings =
        toml::from_str("scm_diff_decorations_ignore_trim_whitespace = true")
            .expect("legacy bool setting should load");
    assert_eq!(
        legacy.scm_diff_decorations_ignore_trim_whitespace,
        ScmDiffDecorationsIgnoreTrimWhitespace::True
    );
}

#[test]
fn scm_graph_page_size_is_clamped_to_vs_code_range() {
    assert_eq!(clamp_scm_graph_page_size(0), MIN_SCM_GRAPH_PAGE_SIZE);
    assert_eq!(clamp_scm_graph_page_size(125), 125);
    assert_eq!(
        clamp_scm_graph_page_size(usize::MAX),
        MAX_SCM_GRAPH_PAGE_SIZE
    );
}

#[test]
fn scm_repositories_visible_is_clamped_to_supported_range() {
    assert_eq!(
        clamp_scm_repositories_visible(0),
        MIN_SCM_REPOSITORIES_VISIBLE
    );
    assert_eq!(clamp_scm_repositories_visible(10), 10);
    assert_eq!(
        clamp_scm_repositories_visible(usize::MAX),
        MAX_SCM_REPOSITORIES_VISIBLE
    );
}

#[test]
fn git_input_validation_length_is_clamped_to_supported_range() {
    assert_eq!(
        clamp_git_input_validation_length(0),
        MIN_GIT_INPUT_VALIDATION_LENGTH
    );
    assert_eq!(clamp_git_input_validation_length(72), 72);
    assert_eq!(
        clamp_git_input_validation_length(usize::MAX),
        MAX_GIT_INPUT_VALIDATION_LENGTH
    );

    let inherited: EditorSettings =
        toml::from_str("git_input_validation_subject_length = \"inherit\"\n")
            .expect("inherit subject length should load");
    assert_eq!(
        inherited.git_input_validation_subject_length,
        GitInputValidationSubjectLength::Inherit
    );

    let numeric: EditorSettings = toml::from_str("git_input_validation_subject_length = 60\n")
        .expect("numeric subject length should load");
    assert_eq!(
        numeric.git_input_validation_subject_length,
        GitInputValidationSubjectLength::Chars(60)
    );
}

#[test]
fn git_commit_short_hash_length_is_clamped_to_vs_code_range() {
    assert_eq!(
        clamp_git_commit_short_hash_length(0),
        MIN_GIT_COMMIT_SHORT_HASH_LENGTH
    );
    assert_eq!(clamp_git_commit_short_hash_length(12), 12);
    assert_eq!(
        clamp_git_commit_short_hash_length(usize::MAX),
        MAX_GIT_COMMIT_SHORT_HASH_LENGTH
    );
}

#[test]
fn git_status_limit_is_clamped_to_supported_range() {
    assert_eq!(clamp_git_status_limit(0), MIN_GIT_STATUS_LIMIT);
    assert_eq!(clamp_git_status_limit(10_000), 10_000);
    assert_eq!(clamp_git_status_limit(usize::MAX), MAX_GIT_STATUS_LIMIT);
}

#[test]
fn git_detect_submodules_limit_is_clamped_to_supported_range() {
    assert_eq!(
        clamp_git_detect_submodules_limit(0),
        MIN_GIT_DETECT_SUBMODULES_LIMIT
    );
    assert_eq!(clamp_git_detect_submodules_limit(10), 10);
    assert_eq!(
        clamp_git_detect_submodules_limit(usize::MAX),
        MAX_GIT_DETECT_SUBMODULES_LIMIT
    );
}

#[test]
fn git_auto_repository_detection_accepts_vs_code_union_values() {
    let disabled: EditorSettings = toml::from_str("git_auto_repository_detection = false\n")
        .expect("disabled auto repository detection should load");
    assert_eq!(
        disabled.git_auto_repository_detection,
        GitAutoRepositoryDetection::False
    );

    let enabled: EditorSettings = toml::from_str("git_auto_repository_detection = true\n")
        .expect("enabled auto repository detection should load");
    assert_eq!(
        enabled.git_auto_repository_detection,
        GitAutoRepositoryDetection::True
    );

    let open_editors: EditorSettings =
        toml::from_str("git_auto_repository_detection = \"openEditors\"\n")
            .expect("openEditors auto repository detection should load");
    assert_eq!(
        open_editors.git_auto_repository_detection,
        GitAutoRepositoryDetection::OpenEditors
    );
}

#[test]
fn git_autofetch_accepts_vs_code_union_values() {
    let disabled: EditorSettings =
        toml::from_str("git_autofetch = false\n").expect("disabled autofetch should load");
    assert_eq!(disabled.git_autofetch, GitAutoFetch::False);

    let enabled: EditorSettings =
        toml::from_str("git_autofetch = true\n").expect("enabled autofetch should load");
    assert_eq!(enabled.git_autofetch, GitAutoFetch::True);

    let all: EditorSettings =
        toml::from_str("git_autofetch = \"all\"\n").expect("all autofetch should load");
    assert_eq!(all.git_autofetch, GitAutoFetch::All);
}

#[test]
fn git_path_accepts_vs_code_union_values() {
    let empty: EditorSettings =
        toml::from_str("git_path = []\n").expect("empty git path list should load");
    assert!(empty.git_path.is_empty());

    let single: EditorSettings =
        toml::from_str("git_path = \"C:/Git/bin/git.exe\"\n").expect("git path should load");
    assert_eq!(single.git_path, ["C:/Git/bin/git.exe"]);

    let multiple: EditorSettings =
        toml::from_str("git_path = [\"C:/Git/bin/git.exe\", \"D:/Git/bin/git.exe\"]\n")
            .expect("git path list should load");
    assert_eq!(
        multiple.git_path,
        ["C:/Git/bin/git.exe", "D:/Git/bin/git.exe"]
    );
}

#[test]
fn git_repository_scan_max_depth_is_clamped_to_supported_range() {
    assert_eq!(
        clamp_git_repository_scan_max_depth(0),
        MIN_GIT_REPOSITORY_SCAN_MAX_DEPTH
    );
    assert_eq!(clamp_git_repository_scan_max_depth(4), 4);
    assert_eq!(
        clamp_git_repository_scan_max_depth(usize::MAX),
        MAX_GIT_REPOSITORY_SCAN_MAX_DEPTH
    );
}

#[test]
fn git_autofetch_and_worktree_limits_are_clamped_to_supported_ranges() {
    assert_eq!(clamp_git_autofetch_period(0), MIN_GIT_AUTOFETCH_PERIOD);
    assert_eq!(clamp_git_autofetch_period(90), 90);
    assert_eq!(
        clamp_git_autofetch_period(usize::MAX),
        MAX_GIT_AUTOFETCH_PERIOD
    );

    assert_eq!(
        clamp_git_detect_worktrees_limit(0),
        MIN_GIT_DETECT_WORKTREES_LIMIT
    );
    assert_eq!(clamp_git_detect_worktrees_limit(7), 7);
    assert_eq!(
        clamp_git_detect_worktrees_limit(usize::MAX),
        MAX_GIT_DETECT_WORKTREES_LIMIT
    );
}

#[test]
fn git_similarity_threshold_is_clamped_to_vs_code_range() {
    assert_eq!(
        clamp_git_similarity_threshold(0),
        MIN_GIT_SIMILARITY_THRESHOLD
    );
    assert_eq!(clamp_git_similarity_threshold(50), 50);
    assert_eq!(
        clamp_git_similarity_threshold(usize::MAX),
        MAX_GIT_SIMILARITY_THRESHOLD
    );
}

#[test]
fn git_branch_validation_error_matches_vs_code_regex_setting() {
    assert_eq!(git_branch_validation_error("bugfix/search", ""), None);
    assert_eq!(
        git_branch_validation_error("feature/search", "^feature/"),
        None
    );
    assert_eq!(
        git_branch_validation_error("bugfix/search", "^feature/"),
        Some("Branch name does not match git.branchValidationRegex".to_owned())
    );
    assert!(
        git_branch_validation_error("feature/search", "[")
            .is_some_and(|error| error.starts_with("Invalid git.branchValidationRegex:"))
    );
}

#[test]
fn git_branch_validation_regex_error_is_single_line_and_bounded() {
    let pattern = format!(
        "[\n{}\u{202e}",
        "x".repeat(MAX_GIT_BRANCH_VALIDATION_ERROR_CHARS * 3)
    );
    let error = git_branch_validation_error("feature/search", &pattern)
        .expect("invalid regex should report a validation error");

    assert!(error.starts_with("Invalid git.branchValidationRegex:"));
    assert!(!error.contains('\n'));
    assert!(!error.contains('\u{202e}'));
    assert!(error.contains("..."));
    assert!(
        error.chars().count()
            <= "Invalid git.branchValidationRegex: ".chars().count()
                + MAX_GIT_BRANCH_VALIDATION_ERROR_CHARS
    );
}
