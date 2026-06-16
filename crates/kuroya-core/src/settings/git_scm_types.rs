use serde::{Deserialize, Serialize};

use super::{DEFAULT_GIT_INPUT_VALIDATION_SUBJECT_LENGTH, clamp_git_input_validation_length};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmDiffDecorations {
    #[default]
    All,
    Gutter,
    Overview,
    Minimap,
    None,
}

impl ScmDiffDecorations {
    pub fn show_gutter(self) -> bool {
        matches!(self, Self::All | Self::Gutter)
    }

    pub fn show_overview(self) -> bool {
        matches!(self, Self::All | Self::Overview)
    }

    pub fn show_minimap(self) -> bool {
        matches!(self, Self::All | Self::Minimap)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmDefaultViewMode {
    #[default]
    List,
    Tree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmDefaultViewSortKey {
    #[default]
    Path,
    Name,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmCountBadge {
    #[default]
    All,
    Focused,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmProviderCountBadge {
    #[default]
    Hidden,
    Auto,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitCountBadge {
    #[default]
    All,
    Tracked,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitUntrackedChanges {
    #[default]
    Mixed,
    Separate,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitOpenRepositoryInParentFolders {
    Always,
    Never,
    #[default]
    Prompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GitAddAiCoAuthor {
    #[default]
    Off,
    ChatAndAgent,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitAutoRepositoryDetection {
    #[default]
    True,
    False,
    SubFolders,
    OpenEditors,
}

impl Serialize for GitAutoRepositoryDetection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::True => serializer.serialize_bool(true),
            Self::False => serializer.serialize_bool(false),
            Self::SubFolders => serializer.serialize_str("subFolders"),
            Self::OpenEditors => serializer.serialize_str("openEditors"),
        }
    }
}

impl<'de> Deserialize<'de> for GitAutoRepositoryDetection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AutoRepositoryDetectionVisitor;

        impl<'de> serde::de::Visitor<'de> for AutoRepositoryDetectionVisitor {
            type Value = GitAutoRepositoryDetection;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of subFolders or openEditors")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    GitAutoRepositoryDetection::True
                } else {
                    GitAutoRepositoryDetection::False
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim() {
                    "true" => Ok(GitAutoRepositoryDetection::True),
                    "false" => Ok(GitAutoRepositoryDetection::False),
                    "subFolders" => Ok(GitAutoRepositoryDetection::SubFolders),
                    "openEditors" => Ok(GitAutoRepositoryDetection::OpenEditors),
                    _ => Err(E::invalid_value(serde::de::Unexpected::Str(value), &self)),
                }
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(AutoRepositoryDetectionVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitAutoFetch {
    True,
    #[default]
    False,
    All,
}

impl Serialize for GitAutoFetch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::True => serializer.serialize_bool(true),
            Self::False => serializer.serialize_bool(false),
            Self::All => serializer.serialize_str("all"),
        }
    }
}

impl<'de> Deserialize<'de> for GitAutoFetch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AutoFetchVisitor;

        impl<'de> serde::de::Visitor<'de> for AutoFetchVisitor {
            type Value = GitAutoFetch;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or all")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    GitAutoFetch::True
                } else {
                    GitAutoFetch::False
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim() {
                    "true" => Ok(GitAutoFetch::True),
                    "false" => Ok(GitAutoFetch::False),
                    "all" => Ok(GitAutoFetch::All),
                    _ => Err(E::invalid_value(serde::de::Unexpected::Str(value), &self)),
                }
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(AutoFetchVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GitOpenAfterClone {
    Always,
    AlwaysNewWindow,
    WhenNoFolderOpen,
    #[default]
    Prompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitPostCommitCommand {
    #[default]
    None,
    Push,
    Sync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitPromptToSaveFilesBeforeCommit {
    #[default]
    Always,
    Staged,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GitBranchProtectionPrompt {
    #[serde(rename = "alwaysCommit")]
    AlwaysCommit,
    #[serde(rename = "alwaysCommitToNewBranch")]
    AlwaysCommitToNewBranch,
    #[default]
    #[serde(rename = "alwaysPrompt")]
    AlwaysPrompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitBranchSortOrder {
    #[default]
    CommitterDate,
    Alphabetically,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmGraphBadges {
    All,
    #[default]
    Filter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmDiffDecorationsGutterAction {
    #[default]
    Diff,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScmDiffDecorationsGutterVisibility {
    #[default]
    Always,
    Hover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScmDiffDecorationsIgnoreTrimWhitespace {
    True,
    #[default]
    False,
    Inherit,
}

impl ScmDiffDecorationsIgnoreTrimWhitespace {
    pub fn resolve(self, diff_ignore_trim_whitespace: bool) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
            Self::Inherit => diff_ignore_trim_whitespace,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitInputValidationSubjectLength {
    Inherit,
    Chars(usize),
}

impl Default for GitInputValidationSubjectLength {
    fn default() -> Self {
        Self::Chars(DEFAULT_GIT_INPUT_VALIDATION_SUBJECT_LENGTH)
    }
}

impl GitInputValidationSubjectLength {
    pub fn resolve(self, line_length: usize) -> usize {
        match self {
            Self::Inherit => clamp_git_input_validation_length(line_length),
            Self::Chars(length) => clamp_git_input_validation_length(length),
        }
    }
}

impl Serialize for GitInputValidationSubjectLength {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Inherit => serializer.serialize_str("inherit"),
            Self::Chars(length) => serializer.serialize_u64(*length as u64),
        }
    }
}

impl<'de> Deserialize<'de> for GitInputValidationSubjectLength {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SubjectLengthVisitor;

        impl<'de> serde::de::Visitor<'de> for SubjectLengthVisitor {
            type Value = GitInputValidationSubjectLength;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a positive number, null, or inherit")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(GitInputValidationSubjectLength::Chars(
                    value.min(usize::MAX as u64) as usize,
                ))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(GitInputValidationSubjectLength::Chars(value.max(0) as usize))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(GitInputValidationSubjectLength::Inherit)
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(GitInputValidationSubjectLength::Inherit)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "inherit" | "null" => Ok(GitInputValidationSubjectLength::Inherit),
                    _ => value
                        .trim()
                        .parse::<usize>()
                        .map(GitInputValidationSubjectLength::Chars)
                        .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(value), &self)),
                }
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(SubjectLengthVisitor)
    }
}

impl Serialize for ScmDiffDecorationsIgnoreTrimWhitespace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::True => serializer.serialize_str("true"),
            Self::False => serializer.serialize_str("false"),
            Self::Inherit => serializer.serialize_str("inherit"),
        }
    }
}

impl<'de> Deserialize<'de> for ScmDiffDecorationsIgnoreTrimWhitespace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IgnoreTrimWhitespaceVisitor;

        impl<'de> serde::de::Visitor<'de> for IgnoreTrimWhitespaceVisitor {
            type Value = ScmDiffDecorationsIgnoreTrimWhitespace;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of true, false, or inherit")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    ScmDiffDecorationsIgnoreTrimWhitespace::True
                } else {
                    ScmDiffDecorationsIgnoreTrimWhitespace::False
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "true" => Ok(ScmDiffDecorationsIgnoreTrimWhitespace::True),
                    "false" => Ok(ScmDiffDecorationsIgnoreTrimWhitespace::False),
                    "inherit" => Ok(ScmDiffDecorationsIgnoreTrimWhitespace::Inherit),
                    _ => Err(E::invalid_value(serde::de::Unexpected::Str(value), &self)),
                }
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(IgnoreTrimWhitespaceVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScmDiffDecorationsGutterPattern {
    pub added: bool,
    pub modified: bool,
}

impl Default for ScmDiffDecorationsGutterPattern {
    fn default() -> Self {
        Self {
            added: false,
            modified: true,
        }
    }
}
