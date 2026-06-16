use std::{
    collections::HashSet,
    env,
    path::{Component, Path, PathBuf},
};

const MAX_FONT_FAMILY_STACK_CHARS: usize = 4096;
const MAX_FONT_FAMILY_STACK_NAMES: usize = 16;
const MAX_FONT_FAMILY_NAME_CHARS: usize = 128;

pub(crate) fn editor_font_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        let fonts = PathBuf::from(local_app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts");
        candidates.push(fonts.join("JetBrainsMono-Regular.ttf"));
        candidates.push(fonts.join("CascadiaCode.ttf"));
    }
    if let Some(windir) = env::var_os("WINDIR") {
        let fonts = PathBuf::from(windir).join("Fonts");
        candidates.push(fonts.join("CascadiaCode.ttf"));
        candidates.push(fonts.join("CascadiaMono.ttf"));
        candidates.push(fonts.join("consola.ttf"));
    }
    candidates.extend([
        PathBuf::from("/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/cascadia-code/CascadiaCode.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
        PathBuf::from("/Library/Fonts/JetBrainsMono-Regular.ttf"),
        PathBuf::from("/Library/Fonts/Menlo.ttf"),
    ]);
    dedupe_paths(candidates)
}

pub(crate) fn editor_font_candidates_for_family_stack(family_stack: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for family in font_family_stack_names(family_stack) {
        candidates.extend(editor_font_candidates_for_family(&family));
    }
    candidates.extend(editor_font_candidates());
    dedupe_paths(candidates)
}

pub(crate) fn font_family_stack_names(family_stack: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(4);
    let mut keys: Vec<String> = Vec::with_capacity(4);
    let mut current = String::with_capacity(family_stack.len().min(MAX_FONT_FAMILY_NAME_CHARS));
    let mut current_chars = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for ch in family_stack.chars().take(MAX_FONT_FAMILY_STACK_CHARS) {
        if escaped {
            push_font_family_char(&mut current, &mut current_chars, ch);
            escaped = false;
            continue;
        }

        if let Some(quote_ch) = quote {
            match ch {
                '\\' => escaped = true,
                ch if ch == quote_ch => quote = None,
                _ => push_font_family_char(&mut current, &mut current_chars, ch),
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            ',' => {
                push_font_family_name(&mut names, &mut keys, &current);
                if names.len() >= MAX_FONT_FAMILY_STACK_NAMES {
                    return names;
                }
                current.clear();
                current_chars = 0;
            }
            _ => push_font_family_char(&mut current, &mut current_chars, ch),
        }
    }

    if escaped {
        push_font_family_char(&mut current, &mut current_chars, '\\');
    }

    push_font_family_name(&mut names, &mut keys, &current);
    names
}

pub(crate) fn ui_font_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        let fonts = PathBuf::from(local_app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts");
        candidates.push(fonts.join("Inter-Regular.ttf"));
    }
    if let Some(windir) = env::var_os("WINDIR") {
        let fonts = PathBuf::from(windir).join("Fonts");
        candidates.push(fonts.join("segoeui.ttf"));
    }
    candidates.extend([
        PathBuf::from("/usr/share/fonts/truetype/inter/Inter-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        PathBuf::from("/Library/Fonts/Inter-Regular.ttf"),
        PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"),
    ]);
    dedupe_paths(candidates)
}

fn editor_font_candidates_for_family(family: &str) -> Vec<PathBuf> {
    match font_family_key(family).as_str() {
        "jetbrainsmono" => jetbrains_mono_candidates(),
        "cascadiacode" => cascadia_code_candidates(),
        "cascadiamono" => cascadia_mono_candidates(),
        "consolas" => consolas_candidates(),
        "dejavusansmono" => dejavu_sans_mono_candidates(),
        "menlo" => menlo_candidates(),
        _ => Vec::new(),
    }
}

fn jetbrains_mono_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts")
                .join("JetBrainsMono-Regular.ttf"),
        );
    }
    candidates.extend([
        PathBuf::from("/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf"),
        PathBuf::from("/Library/Fonts/JetBrainsMono-Regular.ttf"),
    ]);
    candidates
}

fn cascadia_code_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts")
                .join("CascadiaCode.ttf"),
        );
    }
    if let Some(windir) = env::var_os("WINDIR") {
        candidates.push(PathBuf::from(windir).join("Fonts").join("CascadiaCode.ttf"));
    }
    candidates.push(PathBuf::from(
        "/usr/share/fonts/truetype/cascadia-code/CascadiaCode.ttf",
    ));
    candidates
}

fn cascadia_mono_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(windir) = env::var_os("WINDIR") {
        candidates.push(PathBuf::from(windir).join("Fonts").join("CascadiaMono.ttf"));
    }
    candidates
}

fn consolas_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(windir) = env::var_os("WINDIR") {
        candidates.push(PathBuf::from(windir).join("Fonts").join("consola.ttf"));
    }
    candidates
}

fn dejavu_sans_mono_candidates() -> Vec<PathBuf> {
    vec![PathBuf::from(
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    )]
}

fn menlo_candidates() -> Vec<PathBuf> {
    vec![PathBuf::from("/Library/Fonts/Menlo.ttf")]
}

fn is_generic_font_family_key(key: &str) -> bool {
    matches!(
        key,
        "monospace"
            | "serif"
            | "sansserif"
            | "cursive"
            | "fantasy"
            | "systemui"
            | "uimonospace"
            | "uiserif"
            | "uisansserif"
    )
}

fn push_font_family_char(current: &mut String, current_chars: &mut usize, ch: char) {
    if *current_chars >= MAX_FONT_FAMILY_NAME_CHARS {
        return;
    }

    if is_hidden_font_family_control(ch) {
        return;
    }

    current.push(
        if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
            ' '
        } else {
            ch
        },
    );
    *current_chars += 1;
}

fn push_font_family_name(names: &mut Vec<String>, keys: &mut Vec<String>, raw: &str) {
    if names.len() >= MAX_FONT_FAMILY_STACK_NAMES {
        return;
    }

    let Some(name) = normalize_font_family_name(raw) else {
        return;
    };
    let key = font_family_key(&name);
    if key.is_empty() || is_generic_font_family_key(&key) || keys.contains(&key) {
        return;
    }

    keys.push(key);
    names.push(name);
}

fn normalize_font_family_name(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_matches(|ch| ch == '"' || ch == '\'').trim();
    let mut normalized = String::with_capacity(raw.len().min(MAX_FONT_FAMILY_NAME_CHARS));
    let mut previous_was_space = false;
    for ch in raw.chars().take(MAX_FONT_FAMILY_NAME_CHARS) {
        if is_hidden_font_family_control(ch) {
            continue;
        }
        let ch = if ch.is_control() || matches!(ch, '\u{2028}' | '\u{2029}') {
            ' '
        } else {
            ch
        };
        if ch.is_whitespace() {
            if !previous_was_space && !normalized.is_empty() {
                normalized.push(' ');
                previous_was_space = true;
            }
        } else {
            normalized.push(ch);
            previous_was_space = false;
        }
    }

    if normalized.ends_with(' ') {
        normalized.pop();
    }
    (!normalized.is_empty()).then_some(normalized)
}

fn font_family_key(family: &str) -> String {
    let mut key = String::with_capacity(family.len());
    for ch in family.chars().filter(|ch| ch.is_alphanumeric()) {
        if ch.is_ascii() {
            key.push(ch.to_ascii_lowercase());
        } else {
            key.extend(ch.to_lowercase());
        }
    }
    key
}

fn is_hidden_font_family_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::with_capacity(paths.len());
    let mut keys = HashSet::with_capacity(paths.len());
    for path in paths {
        let key = lexical_normalize_path(&path);
        if keys.insert(key) {
            deduped.push(path);
        }
    }
    deduped
}

fn lexical_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut has_root = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                has_root = true;
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop_normal = normalized
                    .components()
                    .next_back()
                    .is_some_and(|component| matches!(component, Component::Normal(_)));
                if can_pop_normal {
                    normalized.pop();
                } else if !has_root {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_FONT_FAMILY_NAME_CHARS, dedupe_paths, font_family_stack_names, lexical_normalize_path,
    };
    use std::path::PathBuf;

    #[test]
    fn font_family_stack_names_keep_commas_inside_quotes() {
        assert_eq!(
            font_family_stack_names("\"Mono, Patched\", 'JetBrains Mono', monospace"),
            vec!["Mono, Patched".to_owned(), "JetBrains Mono".to_owned()]
        );
    }

    #[test]
    fn font_family_stack_names_unescape_quotes_and_collapse_controls() {
        assert_eq!(
            font_family_stack_names("'Mono\\' Special', \"Bad\nName\", serif"),
            vec!["Mono' Special".to_owned(), "Bad Name".to_owned()]
        );
    }

    #[test]
    fn font_family_stack_names_strip_hidden_controls_and_line_separators() {
        assert_eq!(
            font_family_stack_names("Jet\u{202e}Brains\u{200b} Mono, Bad\u{2028}Name"),
            vec!["JetBrains Mono".to_owned(), "Bad Name".to_owned()]
        );
    }

    #[test]
    fn font_family_stack_names_caps_pathological_input() {
        let huge_name = "A".repeat(MAX_FONT_FAMILY_NAME_CHARS + 64);
        let stack = format!("{huge_name}, Menlo");
        let names = font_family_stack_names(&stack);

        assert_eq!(names.len(), 2);
        assert_eq!(names[0].chars().count(), MAX_FONT_FAMILY_NAME_CHARS);
        assert_eq!(names[1], "Menlo");
    }

    #[test]
    fn lexical_normalize_path_preserves_stacked_relative_parents() {
        let stacked = PathBuf::from("..").join("..");
        assert_eq!(lexical_normalize_path(&stacked), stacked);

        let stacked_file = PathBuf::from("..").join("..").join("x");
        assert_eq!(lexical_normalize_path(&stacked_file), stacked_file);

        assert_eq!(
            lexical_normalize_path(
                &PathBuf::from("a")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("b")
            ),
            PathBuf::from("..").join("..").join("b")
        );
    }

    #[test]
    fn dedupe_paths_keeps_stacked_parent_candidate_distinct() {
        let escaped = PathBuf::from("..")
            .join("..")
            .join("Fonts")
            .join("Editor.ttf");
        let local = PathBuf::from("Fonts").join("Editor.ttf");

        assert_eq!(
            dedupe_paths(vec![escaped.clone(), local.clone()]),
            vec![escaped, local]
        );
    }
}
