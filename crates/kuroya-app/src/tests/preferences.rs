use crate::{
    fonts::{
        configured_font_paths, editor_font_candidates_for_family_stack, font_data_name,
        font_family_stack_names, load_font_bytes, load_font_stack_bytes,
    },
    settings_form::{optional_setting_path_from_input, optional_setting_path_to_input},
    status_bar::items::{
        git_status_count_badge_label, git_status_counts_label,
        source_control_provider_count_badge_label,
    },
    theme::{available_theme_labels, selected_theme_index, selected_theme_index_with_plugins},
};

use kuroya_core::{
    GitCountBadge, GitStatusCounts, PLUGIN_API_VERSION, PluginCapabilities, PluginContributions,
    PluginDescriptor, PluginManifest, PluginThemeContribution, PluginThemeRegistry, ScmCountBadge,
    ScmProviderCountBadge, ThemeSettings,
};
use std::{env, path::PathBuf};

#[test]
fn configured_font_paths_prefer_workspace_relative_setting() {
    let root = PathBuf::from("workspace");
    let candidates = vec![PathBuf::from("system-font.ttf")];
    let paths = configured_font_paths(&root, Some("fonts/Editor.ttf"), &candidates);

    assert_eq!(paths[0], root.join("fonts").join("Editor.ttf"));
    assert_eq!(paths[1], PathBuf::from("system-font.ttf"));
}

#[test]
fn font_data_name_is_stable_and_ascii() {
    assert_eq!(
        font_data_name(std::path::Path::new("JetBrains Mono-Regular.ttf")),
        "kuroya_jetbrains_mono_regular"
    );
}

#[test]
fn font_family_stack_names_parse_css_like_lists() {
    assert_eq!(
        font_family_stack_names("\"Cascadia Code\", 'Consolas', monospace, Consolas, system-ui"),
        vec!["Cascadia Code".to_owned(), "Consolas".to_owned()]
    );
}

#[test]
fn editor_font_candidates_follow_configured_family_stack() {
    let candidates =
        editor_font_candidates_for_family_stack("'DejaVu Sans Mono', 'JetBrains Mono', monospace");
    let file_names = candidates
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>();

    assert_eq!(file_names.first().copied(), Some("DejaVuSansMono.ttf"));
    let dejavu_index = file_names
        .iter()
        .position(|name| *name == "DejaVuSansMono.ttf")
        .expect("DejaVu Sans Mono candidate exists");
    let jetbrains_index = file_names
        .iter()
        .position(|name| *name == "JetBrainsMono-Regular.ttf")
        .expect("JetBrains Mono candidate exists");

    assert!(dejavu_index < jetbrains_index);
    assert_eq!(
        file_names
            .iter()
            .filter(|name| **name == "DejaVuSansMono.ttf")
            .count(),
        1
    );
}

#[test]
fn selected_theme_index_matches_builtin_names() {
    let themes = ThemeSettings::built_in_presets();
    let graphite = themes
        .iter()
        .find(|theme| theme.name == "Graphite")
        .expect("graphite preset exists");

    assert_eq!(
        ThemeSettings::built_in_presets()[selected_theme_index(graphite)].name,
        "Graphite"
    );
    assert_eq!(
        selected_theme_index(&ThemeSettings {
            name: "Custom".to_owned(),
            ..ThemeSettings::default()
        }),
        0
    );
}

#[test]
fn plugin_theme_labels_and_selection_follow_discovered_registry() {
    let plugin = PluginDescriptor {
        root: PathBuf::from("workspace/.kuroya/plugins/solar"),
        manifest: PluginManifest {
            api_version: PLUGIN_API_VERSION.to_owned(),
            id: "solar.plugin".to_owned(),
            name: "Solar".to_owned(),
            version: "0.1.0".to_owned(),
            entry: None,
            activation_events: Vec::new(),
            capabilities: PluginCapabilities {
                themes: true,
                ..PluginCapabilities::default()
            },
            contributes: PluginContributions {
                themes: vec![PluginThemeContribution {
                    id: "solar-dark".to_owned(),
                    label: "Solar Dark".to_owned(),
                    path: PathBuf::from("workspace/.kuroya/plugins/solar/themes/dark.toml"),
                }],
                ..PluginContributions::default()
            },
        },
    };
    let registry = PluginThemeRegistry::from_plugins(&[plugin]);
    let labels = available_theme_labels(&registry);

    assert_eq!(labels.last().map(String::as_str), Some("Solar Dark"));
    assert_eq!(
        selected_theme_index_with_plugins(
            &ThemeSettings {
                name: "Solar Dark".to_owned(),
                ..ThemeSettings::default()
            },
            &registry,
        ),
        ThemeSettings::built_in_presets().len()
    );
    assert_eq!(
        selected_theme_index_with_plugins(
            &ThemeSettings {
                name: "Unknown".to_owned(),
                ..ThemeSettings::default()
            },
            &registry,
        ),
        0
    );
}

#[test]
fn git_status_counts_label_is_compact_and_ordered() {
    let label = git_status_counts_label(GitStatusCounts {
        modified: 2,
        added: 1,
        deleted: 0,
        renamed: 1,
        untracked: 3,
        conflicted: 1,
    });

    assert_eq!(label, "M2 A1 R1 ?3 !1");
    assert_eq!(git_status_counts_label(GitStatusCounts::default()), "");
}

#[test]
fn git_status_count_badge_label_follows_scm_count_badge_setting() {
    let counts = GitStatusCounts {
        modified: 2,
        added: 1,
        deleted: 0,
        renamed: 1,
        untracked: 3,
        conflicted: 1,
    };

    assert_eq!(
        git_status_count_badge_label(counts, ScmCountBadge::All, GitCountBadge::All).as_deref(),
        Some("8")
    );
    assert_eq!(
        git_status_count_badge_label(counts, ScmCountBadge::Focused, GitCountBadge::All).as_deref(),
        Some("8")
    );
    assert_eq!(
        git_status_count_badge_label(counts, ScmCountBadge::Off, GitCountBadge::All),
        None
    );
    assert_eq!(
        git_status_count_badge_label(
            GitStatusCounts::default(),
            ScmCountBadge::All,
            GitCountBadge::All
        ),
        None
    );
    assert_eq!(
        git_status_count_badge_label(counts, ScmCountBadge::All, GitCountBadge::Tracked).as_deref(),
        Some("5")
    );
    assert_eq!(
        git_status_count_badge_label(counts, ScmCountBadge::All, GitCountBadge::Off),
        None
    );
}

#[test]
fn source_control_provider_count_badge_label_follows_provider_badge_setting() {
    let counts = GitStatusCounts {
        modified: 2,
        added: 1,
        deleted: 0,
        renamed: 1,
        untracked: 3,
        conflicted: 1,
    };

    assert_eq!(
        source_control_provider_count_badge_label(counts, ScmProviderCountBadge::Hidden),
        None
    );
    assert_eq!(
        source_control_provider_count_badge_label(counts, ScmProviderCountBadge::Auto).as_deref(),
        Some("8")
    );
    assert_eq!(
        source_control_provider_count_badge_label(counts, ScmProviderCountBadge::Visible)
            .as_deref(),
        Some("8")
    );
    assert_eq!(
        source_control_provider_count_badge_label(
            GitStatusCounts::default(),
            ScmProviderCountBadge::Auto
        ),
        None
    );
    assert_eq!(
        source_control_provider_count_badge_label(
            GitStatusCounts::default(),
            ScmProviderCountBadge::Visible
        )
        .as_deref(),
        Some("0")
    );
}

#[test]
fn optional_setting_path_trims_empty_values() {
    assert_eq!(optional_setting_path_from_input(""), None);
    assert_eq!(optional_setting_path_from_input("   "), None);
    assert_eq!(
        optional_setting_path_from_input(" fonts/Inter-Regular.ttf "),
        Some("fonts/Inter-Regular.ttf".to_owned())
    );
    assert_eq!(
        optional_setting_path_to_input(&Some("fonts/Editor.ttf".to_owned())),
        "fonts/Editor.ttf"
    );
    assert_eq!(optional_setting_path_to_input(&None), "");
}

#[test]
fn load_font_bytes_skips_invalid_configured_file() {
    let path = env::temp_dir().join(format!("kuroya-invalid-font-{}.ttf", std::process::id()));
    std::fs::write(&path, b"not a font").expect("write invalid font fixture");

    let loaded = load_font_bytes(std::path::Path::new("."), path.to_str(), &[]);

    let _ = std::fs::remove_file(path);
    assert!(loaded.is_none());
}

#[test]
fn load_font_stack_bytes_skips_invalid_candidates_without_losing_order() {
    let configured = env::temp_dir().join(format!(
        "kuroya-invalid-configured-font-{}.ttf",
        std::process::id()
    ));
    let candidate = env::temp_dir().join(format!(
        "kuroya-invalid-candidate-font-{}.ttf",
        std::process::id()
    ));
    std::fs::write(&configured, b"not a font").expect("write invalid configured font fixture");
    std::fs::write(&candidate, b"also not a font").expect("write invalid candidate font fixture");

    let loaded = load_font_stack_bytes(
        std::path::Path::new("."),
        configured.to_str(),
        std::slice::from_ref(&candidate),
        4,
    );

    let _ = std::fs::remove_file(configured);
    let _ = std::fs::remove_file(candidate);
    assert!(loaded.is_empty());
}
