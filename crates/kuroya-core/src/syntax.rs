use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageBracketPair {
    pub open: char,
    pub close: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageConfiguration {
    line_comment_prefix: Option<&'static str>,
    brackets: &'static [LanguageBracketPair],
    auto_closing_pairs: &'static [LanguageBracketPair],
    increase_indent_line_suffixes: &'static [&'static str],
}

const C_STYLE_BRACKETS: &[LanguageBracketPair] = &[
    LanguageBracketPair {
        open: '(',
        close: ')',
    },
    LanguageBracketPair {
        open: '[',
        close: ']',
    },
    LanguageBracketPair {
        open: '{',
        close: '}',
    },
];

const C_STYLE_AUTO_CLOSING_PAIRS: &[LanguageBracketPair] = &[
    LanguageBracketPair {
        open: '(',
        close: ')',
    },
    LanguageBracketPair {
        open: '[',
        close: ']',
    },
    LanguageBracketPair {
        open: '{',
        close: '}',
    },
    LanguageBracketPair {
        open: '"',
        close: '"',
    },
    LanguageBracketPair {
        open: '\'',
        close: '\'',
    },
    LanguageBracketPair {
        open: '`',
        close: '`',
    },
];

const NO_AUTO_CLOSING_PAIRS: &[LanguageBracketPair] = &[];

const RUST_INDENT_SUFFIXES: &[&str] = &["{", "(", "["];
const PYTHON_INDENT_SUFFIXES: &[&str] = &[":", "{", "(", "["];
const YAML_INDENT_SUFFIXES: &[&str] = &[":"];
const SHELL_INDENT_SUFFIXES: &[&str] = &["then", "do", "case", "{"];
const DEFAULT_INDENT_SUFFIXES: &[&str] = &["{", "(", "["];
const NO_INDENT_SUFFIXES: &[&str] = &[];

fn matches_ignore_ascii_case(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.len() == candidate.len() && value.eq_ignore_ascii_case(candidate))
}

impl LanguageConfiguration {
    pub fn line_comment_prefix(self) -> Option<&'static str> {
        self.line_comment_prefix
    }

    pub fn brackets(self) -> &'static [LanguageBracketPair] {
        self.brackets
    }

    pub fn auto_closing_pairs(self) -> &'static [LanguageBracketPair] {
        self.auto_closing_pairs
    }

    pub fn increase_indent_after_line(self, line_prefix: &str) -> bool {
        let trimmed = line_prefix.trim_end();
        !trimmed.is_empty()
            && self
                .increase_indent_line_suffixes
                .iter()
                .any(|suffix| trimmed.ends_with(suffix))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LanguageId {
    Rust,
    Toml,
    Json,
    Sql,
    Markdown,
    PowerShell,
    Python,
    TypeScript,
    JavaScript,
    Css,
    Html,
    Yaml,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Shell,
    Diff,
    PlainText,
}

impl LanguageId {
    pub fn from_path(path: &Path) -> Self {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(Self::from_file_name)
            .or_else(|| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(Self::from_extension)
            })
            .unwrap_or(Self::PlainText)
    }

    fn from_file_name(file_name: &str) -> Option<Self> {
        if matches_ignore_ascii_case(file_name, &["README"]) {
            Some(Self::Markdown)
        } else if matches_ignore_ascii_case(file_name, &["Cargo.lock", "Pipfile"]) {
            Some(Self::Toml)
        } else if matches_ignore_ascii_case(file_name, &["go.mod", "go.work"]) {
            Some(Self::Go)
        } else if matches_ignore_ascii_case(
            file_name,
            &[
                ".bashrc",
                ".bash_profile",
                ".bash_login",
                ".profile",
                ".zshrc",
                ".zprofile",
            ],
        ) {
            Some(Self::Shell)
        } else {
            None
        }
    }

    fn from_extension(ext: &str) -> Self {
        if ext.eq_ignore_ascii_case("rs") {
            Self::Rust
        } else if ext.eq_ignore_ascii_case("toml") {
            Self::Toml
        } else if ext.eq_ignore_ascii_case("json") {
            Self::Json
        } else if ext.eq_ignore_ascii_case("sql") {
            Self::Sql
        } else if matches_ignore_ascii_case(ext, &["md", "markdown"]) {
            Self::Markdown
        } else if matches_ignore_ascii_case(ext, &["ps1", "psm1"]) {
            Self::PowerShell
        } else if ext.eq_ignore_ascii_case("py") {
            Self::Python
        } else if matches_ignore_ascii_case(ext, &["ts", "tsx"]) {
            Self::TypeScript
        } else if matches_ignore_ascii_case(ext, &["js", "jsx", "mjs", "cjs"]) {
            Self::JavaScript
        } else if matches_ignore_ascii_case(ext, &["css", "scss", "sass", "less"]) {
            Self::Css
        } else if matches_ignore_ascii_case(ext, &["html", "htm", "xhtml"]) {
            Self::Html
        } else if matches_ignore_ascii_case(ext, &["yaml", "yml"]) {
            Self::Yaml
        } else if ext.eq_ignore_ascii_case("go") {
            Self::Go
        } else if ext.eq_ignore_ascii_case("java") {
            Self::Java
        } else if matches_ignore_ascii_case(ext, &["c", "h"]) {
            Self::C
        } else if matches_ignore_ascii_case(ext, &["cc", "cpp", "cxx", "hh", "hpp", "hxx"]) {
            Self::Cpp
        } else if ext.eq_ignore_ascii_case("cs") {
            Self::CSharp
        } else if matches_ignore_ascii_case(ext, &["sh", "bash", "zsh"]) {
            Self::Shell
        } else if matches_ignore_ascii_case(ext, &["diff", "patch"]) {
            Self::Diff
        } else {
            Self::PlainText
        }
    }

    pub fn syntect_extension(self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Sql => "sql",
            Self::Markdown => "md",
            Self::PowerShell => "ps1",
            Self::Python => "py",
            Self::TypeScript => "ts",
            Self::JavaScript => "js",
            Self::Css => "css",
            Self::Html => "html",
            Self::Yaml => "yaml",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "cs",
            Self::Shell => "sh",
            Self::Diff => "diff",
            Self::PlainText => "txt",
        }
    }

    pub fn activation_id(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Toml => "toml",
            Self::Json => "json",
            Self::Sql => "sql",
            Self::Markdown => "markdown",
            Self::PowerShell => "powershell",
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Css => "css",
            Self::Html => "html",
            Self::Yaml => "yaml",
            Self::Go => "go",
            Self::Java => "java",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Shell => "shellscript",
            Self::Diff => "diff",
            Self::PlainText => "plaintext",
        }
    }

    pub fn line_comment_prefix(self) -> Option<&'static str> {
        self.configuration().line_comment_prefix()
    }

    pub fn configuration(self) -> LanguageConfiguration {
        match self {
            Self::Rust => LanguageConfiguration {
                line_comment_prefix: Some("//"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: RUST_INDENT_SUFFIXES,
            },
            Self::TypeScript | Self::JavaScript => LanguageConfiguration {
                line_comment_prefix: Some("//"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: DEFAULT_INDENT_SUFFIXES,
            },
            Self::Go | Self::Java | Self::C | Self::Cpp | Self::CSharp => LanguageConfiguration {
                line_comment_prefix: Some("//"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: DEFAULT_INDENT_SUFFIXES,
            },
            Self::Toml | Self::PowerShell => LanguageConfiguration {
                line_comment_prefix: Some("#"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: DEFAULT_INDENT_SUFFIXES,
            },
            Self::Shell => LanguageConfiguration {
                line_comment_prefix: Some("#"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: SHELL_INDENT_SUFFIXES,
            },
            Self::Python => LanguageConfiguration {
                line_comment_prefix: Some("#"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: PYTHON_INDENT_SUFFIXES,
            },
            Self::Yaml => LanguageConfiguration {
                line_comment_prefix: Some("#"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: YAML_INDENT_SUFFIXES,
            },
            Self::Json => LanguageConfiguration {
                line_comment_prefix: None,
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: DEFAULT_INDENT_SUFFIXES,
            },
            Self::Sql => LanguageConfiguration {
                line_comment_prefix: Some("--"),
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: DEFAULT_INDENT_SUFFIXES,
            },
            Self::Markdown => LanguageConfiguration {
                line_comment_prefix: None,
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: NO_INDENT_SUFFIXES,
            },
            Self::Css | Self::Html => LanguageConfiguration {
                line_comment_prefix: None,
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: DEFAULT_INDENT_SUFFIXES,
            },
            Self::Diff => LanguageConfiguration {
                line_comment_prefix: None,
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: NO_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: NO_INDENT_SUFFIXES,
            },
            Self::PlainText => LanguageConfiguration {
                line_comment_prefix: None,
                brackets: C_STYLE_BRACKETS,
                auto_closing_pairs: C_STYLE_AUTO_CLOSING_PAIRS,
                increase_indent_line_suffixes: DEFAULT_INDENT_SUFFIXES,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_comment_prefixes_cover_line_comment_languages() {
        assert_eq!(LanguageId::Rust.line_comment_prefix(), Some("//"));
        assert_eq!(LanguageId::JavaScript.line_comment_prefix(), Some("//"));
        assert_eq!(LanguageId::Python.line_comment_prefix(), Some("#"));
        assert_eq!(LanguageId::Toml.line_comment_prefix(), Some("#"));
        assert_eq!(LanguageId::Sql.line_comment_prefix(), Some("--"));
        assert_eq!(LanguageId::Go.line_comment_prefix(), Some("//"));
        assert_eq!(LanguageId::Java.line_comment_prefix(), Some("//"));
        assert_eq!(LanguageId::Cpp.line_comment_prefix(), Some("//"));
        assert_eq!(LanguageId::Shell.line_comment_prefix(), Some("#"));
        assert_eq!(LanguageId::Yaml.line_comment_prefix(), Some("#"));
        assert_eq!(LanguageId::Json.line_comment_prefix(), None);
        assert_eq!(LanguageId::Css.line_comment_prefix(), None);
        assert_eq!(LanguageId::Html.line_comment_prefix(), None);
        assert_eq!(LanguageId::Diff.line_comment_prefix(), None);
        assert_eq!(LanguageId::PlainText.line_comment_prefix(), None);
    }

    #[test]
    fn language_detection_covers_common_daily_driver_file_types() {
        assert_eq!(LanguageId::from_path(Path::new("app.css")), LanguageId::Css);
        assert_eq!(
            LanguageId::from_path(Path::new("index.html")),
            LanguageId::Html
        );
        assert_eq!(
            LanguageId::from_path(Path::new("compose.yml")),
            LanguageId::Yaml
        );
        assert_eq!(LanguageId::from_path(Path::new("main.go")), LanguageId::Go);
        assert_eq!(
            LanguageId::from_path(Path::new("App.java")),
            LanguageId::Java
        );
        assert_eq!(LanguageId::from_path(Path::new("lib.cpp")), LanguageId::Cpp);
        assert_eq!(
            LanguageId::from_path(Path::new("include/app.h")),
            LanguageId::C
        );
        assert_eq!(
            LanguageId::from_path(Path::new("Program.cs")),
            LanguageId::CSharp
        );
        assert_eq!(
            LanguageId::from_path(Path::new("scripts/build.sh")),
            LanguageId::Shell
        );
    }

    #[test]
    fn language_detection_uses_known_filenames_without_standard_extensions() {
        assert_eq!(
            LanguageId::from_path(Path::new("README")),
            LanguageId::Markdown
        );
        assert_eq!(
            LanguageId::from_path(Path::new("Cargo.lock")),
            LanguageId::Toml
        );
        assert_eq!(
            LanguageId::from_path(Path::new("services/api/Pipfile")),
            LanguageId::Toml
        );
        assert_eq!(LanguageId::from_path(Path::new("go.mod")), LanguageId::Go);
        assert_eq!(
            LanguageId::from_path(Path::new("workspace/go.work")),
            LanguageId::Go
        );
    }

    #[test]
    fn language_detection_handles_extensionless_shell_profiles() {
        assert_eq!(
            LanguageId::from_path(Path::new(".bashrc")),
            LanguageId::Shell
        );
        assert_eq!(
            LanguageId::from_path(Path::new(".bash_profile")),
            LanguageId::Shell
        );
        assert_eq!(
            LanguageId::from_path(Path::new("home/.profile")),
            LanguageId::Shell
        );
        assert_eq!(
            LanguageId::from_path(Path::new(".zshrc")),
            LanguageId::Shell
        );
    }

    #[test]
    fn language_detection_is_case_insensitive_for_extensions() {
        assert_eq!(
            LanguageId::from_path(Path::new("MAIN.RS")),
            LanguageId::Rust
        );
        assert_eq!(
            LanguageId::from_path(Path::new("README.MarkDown")),
            LanguageId::Markdown
        );
        assert_eq!(
            LanguageId::from_path(Path::new("SCRIPT.PS1")),
            LanguageId::PowerShell
        );
        assert_eq!(
            LanguageId::from_path(Path::new("component.TSX")),
            LanguageId::TypeScript
        );
    }

    #[test]
    fn language_configurations_expose_indentation_boundaries() {
        assert!(
            LanguageId::Rust
                .configuration()
                .increase_indent_after_line("fn main() {")
        );
        assert!(
            LanguageId::Python
                .configuration()
                .increase_indent_after_line("if ready:")
        );
        assert!(
            !LanguageId::PlainText
                .configuration()
                .increase_indent_after_line("label:")
        );
        assert!(
            LanguageId::Yaml
                .configuration()
                .increase_indent_after_line("services:")
        );
        assert!(
            LanguageId::Shell
                .configuration()
                .increase_indent_after_line("if ready; then")
        );
        assert_eq!(
            LanguageId::Rust.configuration().brackets()[0],
            LanguageBracketPair {
                open: '(',
                close: ')'
            }
        );
        assert!(LanguageId::Rust.configuration().auto_closing_pairs().len() > 3);
        assert!(
            LanguageId::Diff
                .configuration()
                .auto_closing_pairs()
                .is_empty()
        );
    }

    #[test]
    fn language_activation_ids_are_stable_plugin_keys() {
        assert_eq!(LanguageId::Rust.activation_id(), "rust");
        assert_eq!(LanguageId::Sql.activation_id(), "sql");
        assert_eq!(LanguageId::TypeScript.activation_id(), "typescript");
        assert_eq!(LanguageId::Go.activation_id(), "go");
        assert_eq!(LanguageId::Cpp.activation_id(), "cpp");
        assert_eq!(LanguageId::CSharp.activation_id(), "csharp");
        assert_eq!(LanguageId::Shell.activation_id(), "shellscript");
        assert_eq!(LanguageId::PlainText.activation_id(), "plaintext");
    }
}
