use super::{
    DEFAULT_EDITOR_FONT_LIGATURES, DEFAULT_EDITOR_FONT_VARIATIONS, EDITOR_FONT_LIGATURES_ON,
    EDITOR_FONT_VARIATIONS_TRANSLATE, SETTINGS_DESERIALIZE_LIST_HARD_CAP,
};

pub(super) fn deserialize_optional_string_list<'de, D>(
    deserializer: D,
) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringListVisitor;

    impl<'de> serde::de::Visitor<'de> for StringListVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("null, a string, or a list of strings")
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(if value.trim().is_empty() {
                Vec::new()
            } else {
                vec![value.to_owned()]
            })
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(if value.trim().is_empty() {
                Vec::new()
            } else {
                vec![value]
            })
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut values = Vec::with_capacity(
                seq.size_hint()
                    .unwrap_or_default()
                    .min(SETTINGS_DESERIALIZE_LIST_HARD_CAP),
            );
            while let Some(value) = seq.next_element::<String>()? {
                if values.len() < SETTINGS_DESERIALIZE_LIST_HARD_CAP && !value.trim().is_empty() {
                    values.push(value);
                }
            }
            Ok(values)
        }
    }

    deserializer.deserialize_any(StringListVisitor)
}

pub(super) fn default_editor_font_ligatures() -> String {
    DEFAULT_EDITOR_FONT_LIGATURES.to_owned()
}

pub(super) fn default_editor_font_variations() -> String {
    DEFAULT_EDITOR_FONT_VARIATIONS.to_owned()
}

pub fn normalize_editor_font_ligatures(value: &str) -> String {
    match value.trim() {
        "" | "false" => DEFAULT_EDITOR_FONT_LIGATURES.to_owned(),
        "true" => EDITOR_FONT_LIGATURES_ON.to_owned(),
        value => value.to_owned(),
    }
}

pub fn normalize_editor_font_variations(value: &str) -> String {
    match value.trim() {
        "false" => DEFAULT_EDITOR_FONT_VARIATIONS.to_owned(),
        "true" => EDITOR_FONT_VARIATIONS_TRANSLATE.to_owned(),
        value => value.to_owned(),
    }
}

pub(super) fn deserialize_editor_font_ligatures<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct FontLigaturesVisitor;

    impl<'de> serde::de::Visitor<'de> for FontLigaturesVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a boolean or font feature settings string")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(if value {
                EDITOR_FONT_LIGATURES_ON.to_owned()
            } else {
                DEFAULT_EDITOR_FONT_LIGATURES.to_owned()
            })
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(normalize_editor_font_ligatures(value))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(FontLigaturesVisitor)
}

pub(super) fn deserialize_editor_font_variations<'de, D>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct FontVariationsVisitor;

    impl<'de> serde::de::Visitor<'de> for FontVariationsVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a boolean or font variation settings string")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(if value {
                EDITOR_FONT_VARIATIONS_TRANSLATE.to_owned()
            } else {
                DEFAULT_EDITOR_FONT_VARIATIONS.to_owned()
            })
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(normalize_editor_font_variations(value))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(FontVariationsVisitor)
}
