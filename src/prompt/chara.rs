use std::{collections::HashMap, path::Path};

use regex::RegexBuilder;
use serde::Deserialize;
use serde_json::{Number, Value};
use snafu::{FromString, ResultExt, Whatever};

use crate::Result;

type ExtensionType = HashMap<String, Value>;

// Only used for reading from JSON. Since V2 is a strict superset of
// V1, we'll use the same struct.
#[derive(Debug, Deserialize)]
struct TavernCardV1 {
    name: String,
    description: String,
    personality: String,
    scenario: String,
    first_mes: String,
    mes_example: String,
}

#[expect(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct TavernCardV2 {
    pub name: String,
    description: String,
    personality: String,
    scenario: String,
    first_mes: String,
    mes_example: String,

    creator_notes: String,
    system_prompt: String,
    post_history_instructions: String,
    alternate_greetings: Vec<String>,
    character_book: Option<CharacterBook>,

    tags: Vec<String>,
    creator: String,
    character_version: String,
    extensions: ExtensionType,
}

impl TavernCardV2 {
    pub fn description(&self, username: &str) -> String {
        replace_placeholders(username, &self.name, &self.description)
    }

    #[expect(dead_code)]
    pub fn personality(&self, username: &str) -> String {
        replace_placeholders(username, &self.name, &self.personality)
    }

    #[expect(dead_code)]
    pub fn scenario(&self, username: &str) -> String {
        replace_placeholders(username, &self.name, &self.scenario)
    }

    #[expect(dead_code)]
    pub fn first_mes(&self, username: &str) -> String {
        replace_placeholders(username, &self.name, &self.first_mes)
    }

    #[expect(dead_code)]
    pub fn mes_example(&self, username: &str) -> String {
        replace_placeholders(username, &self.name, &self.mes_example)
    }
}

#[expect(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CharacterBook {
    name: Option<String>,
    description: Option<String>,
    scan_depth: Option<Number>,
    token_budget: Option<Number>,
    recursive_scanning: Option<bool>,
    extensions: ExtensionType,
    // NB Standalone world books/lore books seem to have the
    // following as object/dict instead of an array. If/when we
    // load those, we'll probably have to support both.
    entries: Vec<CharacterBookEntry>,
}

#[expect(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CharacterBookEntry {
    keys: Vec<String>,
    content: String,
    extensions: ExtensionType,
    enabled: bool,
    insertion_order: Number,
    case_sensitive: Option<bool>,

    name: Option<String>,
    priority: Option<Number>,

    id: Option<Number>,
    comment: Option<String>,
    selective: Option<bool>,
    secondary_keys: Option<Vec<String>>,
    constant: Option<bool>,
    position: Option<String>, // FIXME actually an enum
}

impl From<TavernCardV1> for TavernCardV2 {
    fn from(value: TavernCardV1) -> Self {
        Self {
            name: value.name,
            description: value.description,
            personality: value.personality,
            scenario: value.scenario,
            first_mes: value.first_mes,
            mes_example: value.mes_example,
            ..Default::default()
        }
    }
}

#[derive(Debug)]
pub enum TavernCard {
    V1(TavernCardV2), // Not a typo. Use same struct.
    V2(TavernCardV2),
}

impl TavernCard {
    pub fn data(&self) -> &TavernCardV2 {
        match self {
            TavernCard::V1(data) | TavernCard::V2(data) => data,
        }
    }
}

pub fn read_card_json<P>(card: P) -> Result<TavernCard>
where
    P: AsRef<Path>,
{
    let card_name = card.as_ref().display();

    let card_str = std::fs::read_to_string(&card)
        .with_whatever_context(|_| format!("reading card {card_name}"))?;
    let card_json: Value = serde_json::from_str(&card_str)
        .with_whatever_context(|_| format!("JSON decode {card_name}"))?;

    // FIXME should fail if spec/spec_version exist but are wrong
    if let Some(spec) = &card_json.pointer("/spec").and_then(Value::as_str)
        && *spec == "chara_card_v2"
    {
        if let Some(data) = card_json.pointer("/data") {
            let v2: TavernCardV2 = serde_json::from_value(data.clone())
                .with_whatever_context(|_| format!("deserializing V2 card {card_name}"))?;
            Ok(TavernCard::V2(v2))
        } else {
            Err(Whatever::without_source(format!(
                "malformed V2 card {card_name}"
            )))
        }
    } else {
        let v1: TavernCardV1 = serde_json::from_value(card_json.clone())
            .with_whatever_context(|_| format!("deserializing V1 card {card_name}"))?;
        Ok(TavernCard::V1(v1.into()))
    }
}

pub fn replace_placeholders(user_name: &str, char_name: &str, input: &str) -> String {
    // TODO move these to module
    let user_re = RegexBuilder::new(r"\{\{user\}\}|<user>")
        .case_insensitive(true)
        .build()
        .unwrap();
    let char_re = RegexBuilder::new(r"\{\{char\}\}|<bot>")
        .case_insensitive(true)
        .build()
        .unwrap();

    let input = user_re.replace_all(input, user_name);
    let input = char_re.replace_all(&input, char_name);
    input.to_string()
}

#[cfg(test)]
mod tests {
    use super::read_card_json;

    #[test]
    fn test_basic_v1() {
        let v1 = read_card_json("test/chara-V1.json").unwrap();
        let data = v1.data();
        assert_eq!(data.name, "Sirocco");
        assert_eq!(
            data.description,
            "You are {{char}}, a parrot that likes to party."
        );
    }

    #[test]
    fn test_replace_v1() {
        let v1 = read_card_json("test/chara-V1.json").unwrap();
        let data = v1.data();
        assert_eq!(
            data.description("User"),
            "You are Sirocco, a parrot that likes to party."
        );
    }

    #[test]
    fn test_basic_v2() {
        let v2 = read_card_json("test/chara-V2.json").unwrap();
        let data = v2.data();
        assert_eq!(data.name, "Jack");
        assert_eq!(
            data.description,
            "You are {{char}}, a sparrow that likes to party."
        );
        assert_eq!(data.creator, "Me");
        assert_eq!(data.character_version, "1.0");
    }
}
