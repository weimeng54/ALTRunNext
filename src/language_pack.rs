use serde::{Deserialize, Deserializer, de};
use std::{collections::BTreeMap, fmt};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguagePack {
    pub locale: String,
    pub native_name: String,
    pub format_version: u32,
    #[serde(deserialize_with = "unique_strings")]
    pub strings: BTreeMap<String, String>,
}

fn unique_strings<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct Strings;
    impl<'de> de::Visitor<'de> for Strings {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an object with unique resource keys and string values")
        }

        fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut strings = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                if strings.contains_key(&key) {
                    return Err(de::Error::custom(format!("duplicate resource key {key}")));
                }
                strings.insert(key, value);
            }
            Ok(strings)
        }
    }
    deserializer.deserialize_map(Strings)
}

pub fn language_tag(value: &str) -> Option<String> {
    let mut parts = value.split('-');
    let base = parts.next()?;
    if value.len() > 63
        || !(2..=8).contains(&base.len())
        || !base.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return None;
    }
    let mut tag = base.to_ascii_lowercase();
    for part in parts {
        if !(2..=8).contains(&part.len()) || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return None;
        }
        tag.push('-');
        if part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            tag.push_str(&part.to_ascii_uppercase());
        } else if part.len() == 4 && part.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            tag.push_str(&part[..1].to_ascii_uppercase());
            tag.push_str(&part[1..].to_ascii_lowercase());
        } else {
            tag.push_str(&part.to_ascii_lowercase());
        }
    }
    Some(tag)
}

pub fn placeholders(message: &str) -> Vec<&str> {
    let mut names: Vec<_> = message
        .split('{')
        .skip(1)
        .filter_map(|part| {
            let (name, _) = part.split_once('}')?;
            (name.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            .then_some(name)
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

impl LanguagePack {
    pub fn parse(input: &str) -> Result<Self, String> {
        let mut pack: Self = serde_json::from_str(input.trim_start_matches('\u{feff}'))
            .map_err(|error| error.to_string())?;
        pack.locale = language_tag(&pack.locale)
            .filter(|locale| locale != "system")
            .ok_or("invalid locale")?;
        if pack.format_version != 1 {
            return Err("unsupported formatVersion".into());
        }
        if pack.native_name.trim().is_empty()
            || pack.native_name.chars().count() > 64
            || pack.native_name.chars().any(char::is_control)
        {
            return Err("invalid nativeName".into());
        }
        if pack.strings.values().any(|value| {
            value.trim().is_empty()
                || value.chars().count() > 4096
                || value
                    .chars()
                    .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\t'))
        }) {
            return Err("invalid translation text".into());
        }
        Ok(pack)
    }

    pub fn check<'a>(
        &self,
        base: impl IntoIterator<Item = (&'a str, &'a str)>,
        complete: bool,
    ) -> Result<(), String> {
        let base: BTreeMap<_, _> = base.into_iter().collect();
        for (key, value) in &self.strings {
            let Some(original) = base.get(key.as_str()) else {
                if complete {
                    return Err(format!("unknown key {key}"));
                }
                continue;
            };
            if placeholders(original) != placeholders(value)
                || original.matches('\t').count() != value.matches('\t').count()
            {
                return Err(format!("mismatched placeholders in {key}"));
            }
        }
        if complete && let Some(key) = base.keys().find(|key| !self.strings.contains_key(**key)) {
            return Err(format!("missing key {key}"));
        }
        Ok(())
    }
}
