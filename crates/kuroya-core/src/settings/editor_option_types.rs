use serde::{Deserialize, Serialize};

use super::{
    DEFAULT_EDITOR_LINE_DECORATIONS_WIDTH, clamp_editor_line_decorations_width,
    clamp_editor_line_decorations_width_ch,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorLineNumbers {
    #[default]
    On,
    Off,
    Relative,
    Interval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorRenderWhitespace {
    #[default]
    None,
    Boundary,
    Selection,
    Trailing,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorRenderFinalNewline {
    Off,
    On,
    Dimmed,
}

impl Default for EditorRenderFinalNewline {
    fn default() -> Self {
        if cfg!(target_os = "linux") {
            Self::Dimmed
        } else {
            Self::On
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorRenderLineHighlight {
    None,
    Gutter,
    #[default]
    Line,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorAutoClosingStrategy {
    Always,
    #[default]
    LanguageDefined,
    BeforeWhitespace,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorAutoClosingEditStrategy {
    Always,
    #[default]
    Auto,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorExperimentalGpuAcceleration {
    On,
    #[default]
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorExperimentalWhitespaceRendering {
    #[default]
    Svg,
    Font,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorLightbulbMode {
    Off,
    On,
    #[default]
    OnCode,
}

impl EditorLightbulbMode {
    pub fn enabled(self) -> bool {
        !matches!(self, Self::Off)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorSuggestSelection {
    #[default]
    First,
    RecentlyUsed,
    RecentlyUsedByPrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorSuggestInsertMode {
    #[default]
    Insert,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorSuggestSelectionMode {
    #[default]
    Always,
    Never,
    WhenTriggerCharacter,
    WhenQuickSuggestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorSuggestPreviewMode {
    Prefix,
    Subword,
    #[default]
    SubwordSmart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorTabCompletion {
    On,
    #[default]
    Off,
    OnlySnippets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorSnippetSuggestions {
    Top,
    Bottom,
    #[default]
    Inline,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorInlineSuggestMode {
    Prefix,
    Subword,
    #[default]
    SubwordSmart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorInlineSuggestShowToolbar {
    Always,
    #[default]
    OnHover,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorInlineSuggestEditsAllowCodeShifting {
    #[default]
    Always,
    Horizontal,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorInlineSuggestEditsRenderSideBySide {
    #[default]
    Auto,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorInlineSuggestShowOnSuggestConflict {
    Always,
    #[default]
    Never,
    WhenSuggestListIsIncomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorFindSeedSearchStringFromSelection {
    Never,
    #[default]
    Always,
    Selection,
}

impl EditorFindSeedSearchStringFromSelection {
    pub fn seeds_selection(self) -> bool {
        !matches!(self, Self::Never)
    }

    pub fn seeds_word_at_cursor(self) -> bool {
        matches!(self, Self::Always)
    }
}

impl Serialize for EditorFindSeedSearchStringFromSelection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Never => serializer.serialize_str("never"),
            Self::Always => serializer.serialize_str("always"),
            Self::Selection => serializer.serialize_str("selection"),
        }
    }
}

impl<'de> Deserialize<'de> for EditorFindSeedSearchStringFromSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FindSeedVisitor;

        impl<'de> serde::de::Visitor<'de> for FindSeedVisitor {
            type Value = EditorFindSeedSearchStringFromSelection;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of always, selection, or never")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorFindSeedSearchStringFromSelection::Always
                } else {
                    EditorFindSeedSearchStringFromSelection::Never
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "always" | "on" | "true" => Ok(EditorFindSeedSearchStringFromSelection::Always),
                    "selection" => Ok(EditorFindSeedSearchStringFromSelection::Selection),
                    "never" | "off" | "false" => Ok(EditorFindSeedSearchStringFromSelection::Never),
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

        deserializer.deserialize_any(FindSeedVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorFindAutoFindInSelection {
    #[default]
    Never,
    Always,
    Multiline,
}

impl Serialize for EditorFindAutoFindInSelection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Never => serializer.serialize_str("never"),
            Self::Always => serializer.serialize_str("always"),
            Self::Multiline => serializer.serialize_str("multiline"),
        }
    }
}

impl<'de> Deserialize<'de> for EditorFindAutoFindInSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AutoFindInSelectionVisitor;

        impl<'de> serde::de::Visitor<'de> for AutoFindInSelectionVisitor {
            type Value = EditorFindAutoFindInSelection;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of always, multiline, or never")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorFindAutoFindInSelection::Always
                } else {
                    EditorFindAutoFindInSelection::Never
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "always" | "on" | "true" => Ok(EditorFindAutoFindInSelection::Always),
                    "multiline" => Ok(EditorFindAutoFindInSelection::Multiline),
                    "never" | "off" | "false" => Ok(EditorFindAutoFindInSelection::Never),
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

        deserializer.deserialize_any(AutoFindInSelectionVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorFindHistory {
    Never,
    #[default]
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditorLineDecorationsWidth {
    Pixels(f32),
    Ch(f32),
}

impl Default for EditorLineDecorationsWidth {
    fn default() -> Self {
        Self::Pixels(DEFAULT_EDITOR_LINE_DECORATIONS_WIDTH)
    }
}

impl EditorLineDecorationsWidth {
    pub fn clamped(self) -> Self {
        match self {
            Self::Pixels(width) => Self::Pixels(clamp_editor_line_decorations_width(width)),
            Self::Ch(chars) => Self::Ch(clamp_editor_line_decorations_width_ch(chars)),
        }
    }

    pub fn pixels(self, char_width: f32) -> f32 {
        let char_width = if char_width.is_finite() && char_width > 0.0 {
            char_width
        } else {
            8.0
        };
        match self {
            Self::Pixels(width) => clamp_editor_line_decorations_width(width),
            Self::Ch(chars) => clamp_editor_line_decorations_width(chars * char_width),
        }
    }
}

impl Serialize for EditorLineDecorationsWidth {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            Self::Pixels(width) => {
                serializer.serialize_f32(clamp_editor_line_decorations_width(width))
            }
            Self::Ch(chars) => serializer.serialize_str(&format!(
                "{}ch",
                clamp_editor_line_decorations_width_ch(chars)
            )),
        }
    }
}

impl<'de> Deserialize<'de> for EditorLineDecorationsWidth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct LineDecorationsWidthVisitor;

        impl<'de> serde::de::Visitor<'de> for LineDecorationsWidthVisitor {
            type Value = EditorLineDecorationsWidth;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a pixel number or a string like 1.3ch")
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(EditorLineDecorationsWidth::Pixels(
                    clamp_editor_line_decorations_width(value as f32),
                ))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(EditorLineDecorationsWidth::Pixels(
                    clamp_editor_line_decorations_width(value as f32),
                ))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(EditorLineDecorationsWidth::Pixels(
                    clamp_editor_line_decorations_width(value as f32),
                ))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let trimmed = value.trim();
                if let Some(chars) = trimmed.strip_suffix("ch") {
                    let chars = chars
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(value), &self))?;
                    return Ok(EditorLineDecorationsWidth::Ch(
                        clamp_editor_line_decorations_width_ch(chars),
                    ));
                }
                let width = trimmed
                    .parse::<f32>()
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(value), &self))?;
                Ok(EditorLineDecorationsWidth::Pixels(
                    clamp_editor_line_decorations_width(width),
                ))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(LineDecorationsWidthVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorRenderValidationDecorations {
    Off,
    #[default]
    Editable,
    On,
}

impl EditorRenderValidationDecorations {
    pub fn visible(self, read_only: bool) -> bool {
        match self {
            Self::Off => false,
            Self::Editable => !read_only,
            Self::On => true,
        }
    }
}

impl<'de> Deserialize<'de> for EditorRenderValidationDecorations {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RenderValidationDecorationsVisitor;

        impl<'de> serde::de::Visitor<'de> for RenderValidationDecorationsVisitor {
            type Value = EditorRenderValidationDecorations;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of off, editable, or on")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorRenderValidationDecorations::On
                } else {
                    EditorRenderValidationDecorations::Off
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "editable" => Ok(EditorRenderValidationDecorations::Editable),
                    "on" | "true" => Ok(EditorRenderValidationDecorations::On),
                    "off" | "false" | "none" => Ok(EditorRenderValidationDecorations::Off),
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

        deserializer.deserialize_any(RenderValidationDecorationsVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorUnicodeHighlightNonBasicAscii {
    Off,
    On,
    #[default]
    InUntrustedWorkspace,
}

impl EditorUnicodeHighlightNonBasicAscii {
    pub fn enabled(self, workspace_trusted: bool) -> bool {
        match self {
            Self::Off => false,
            Self::On => true,
            Self::InUntrustedWorkspace => !workspace_trusted,
        }
    }
}

impl Serialize for EditorUnicodeHighlightNonBasicAscii {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Off => serializer.serialize_bool(false),
            Self::On => serializer.serialize_bool(true),
            Self::InUntrustedWorkspace => serializer.serialize_str("inUntrustedWorkspace"),
        }
    }
}

impl<'de> Deserialize<'de> for EditorUnicodeHighlightNonBasicAscii {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NonBasicAsciiVisitor;

        impl<'de> serde::de::Visitor<'de> for NonBasicAsciiVisitor {
            type Value = EditorUnicodeHighlightNonBasicAscii;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of off, on, or inUntrustedWorkspace")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorUnicodeHighlightNonBasicAscii::On
                } else {
                    EditorUnicodeHighlightNonBasicAscii::Off
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "on" | "true" => Ok(EditorUnicodeHighlightNonBasicAscii::On),
                    "off" | "false" | "none" => Ok(EditorUnicodeHighlightNonBasicAscii::Off),
                    "inuntrustedworkspace" | "untrusted" | "untrustedworkspace" => {
                        Ok(EditorUnicodeHighlightNonBasicAscii::InUntrustedWorkspace)
                    }
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

        deserializer.deserialize_any(NonBasicAsciiVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorUnicodeHighlightScope {
    Off,
    #[default]
    On,
    InUntrustedWorkspace,
}

impl EditorUnicodeHighlightScope {
    pub fn enabled(self, workspace_trusted: bool) -> bool {
        match self {
            Self::Off => false,
            Self::On => true,
            Self::InUntrustedWorkspace => !workspace_trusted,
        }
    }
}

impl Serialize for EditorUnicodeHighlightScope {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Off => serializer.serialize_bool(false),
            Self::On => serializer.serialize_bool(true),
            Self::InUntrustedWorkspace => serializer.serialize_str("inUntrustedWorkspace"),
        }
    }
}

impl<'de> Deserialize<'de> for EditorUnicodeHighlightScope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct UnicodeHighlightScopeVisitor;

        impl<'de> serde::de::Visitor<'de> for UnicodeHighlightScopeVisitor {
            type Value = EditorUnicodeHighlightScope;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of off, on, or inUntrustedWorkspace")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorUnicodeHighlightScope::On
                } else {
                    EditorUnicodeHighlightScope::Off
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "on" | "true" => Ok(EditorUnicodeHighlightScope::On),
                    "off" | "false" | "none" => Ok(EditorUnicodeHighlightScope::Off),
                    "inuntrustedworkspace" | "untrusted" | "untrustedworkspace" => {
                        Ok(EditorUnicodeHighlightScope::InUntrustedWorkspace)
                    }
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

        deserializer.deserialize_any(UnicodeHighlightScopeVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorHighlightActiveIndentation {
    Off,
    #[default]
    Focused,
    Always,
}

impl EditorHighlightActiveIndentation {
    pub fn visible(self, focused: bool) -> bool {
        match self {
            Self::Off => false,
            Self::Focused => focused,
            Self::Always => true,
        }
    }
}

impl Serialize for EditorHighlightActiveIndentation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Off => serializer.serialize_bool(false),
            Self::Focused => serializer.serialize_bool(true),
            Self::Always => serializer.serialize_str("always"),
        }
    }
}

impl<'de> Deserialize<'de> for EditorHighlightActiveIndentation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HighlightActiveIndentationVisitor;

        impl<'de> serde::de::Visitor<'de> for HighlightActiveIndentationVisitor {
            type Value = EditorHighlightActiveIndentation;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of off, focused, on, or always")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorHighlightActiveIndentation::Focused
                } else {
                    EditorHighlightActiveIndentation::Off
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "always" => Ok(EditorHighlightActiveIndentation::Always),
                    "focused" | "focus" | "on" | "true" => {
                        Ok(EditorHighlightActiveIndentation::Focused)
                    }
                    "off" | "false" | "none" => Ok(EditorHighlightActiveIndentation::Off),
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

        deserializer.deserialize_any(HighlightActiveIndentationVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorBracketPairGuideMode {
    #[default]
    Off,
    Active,
    On,
}

impl EditorBracketPairGuideMode {
    pub fn enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn active_only(self) -> bool {
        matches!(self, Self::Active)
    }
}

impl Serialize for EditorBracketPairGuideMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Off => serializer.serialize_bool(false),
            Self::Active => serializer.serialize_str("active"),
            Self::On => serializer.serialize_bool(true),
        }
    }
}

impl<'de> Deserialize<'de> for EditorBracketPairGuideMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BracketPairGuideModeVisitor;

        impl<'de> serde::de::Visitor<'de> for BracketPairGuideModeVisitor {
            type Value = EditorBracketPairGuideMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of off, active, or on")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorBracketPairGuideMode::On
                } else {
                    EditorBracketPairGuideMode::Off
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "active" => Ok(EditorBracketPairGuideMode::Active),
                    "on" | "true" => Ok(EditorBracketPairGuideMode::On),
                    "off" | "false" | "none" => Ok(EditorBracketPairGuideMode::Off),
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

        deserializer.deserialize_any(BracketPairGuideModeVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMatchBrackets {
    #[default]
    Always,
    Never,
    Near,
}

impl EditorMatchBrackets {
    pub fn enabled(self) -> bool {
        !matches!(self, Self::Never)
    }
}

impl Serialize for EditorMatchBrackets {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Always => serializer.serialize_str("always"),
            Self::Never => serializer.serialize_str("never"),
            Self::Near => serializer.serialize_str("near"),
        }
    }
}

impl<'de> Deserialize<'de> for EditorMatchBrackets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MatchBracketsVisitor;

        impl<'de> serde::de::Visitor<'de> for MatchBracketsVisitor {
            type Value = EditorMatchBrackets;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or one of always, near, or never")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(if value {
                    EditorMatchBrackets::Always
                } else {
                    EditorMatchBrackets::Never
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "always" | "on" | "true" => Ok(EditorMatchBrackets::Always),
                    "near" => Ok(EditorMatchBrackets::Near),
                    "never" | "off" | "false" => Ok(EditorMatchBrackets::Never),
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

        deserializer.deserialize_any(MatchBracketsVisitor)
    }
}
