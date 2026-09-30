use std::path::Path;

use crate::{
    Result,
    config::Config,
    prompt::chara::{TavernCard, read_card_json, replace_placeholders},
};

mod chara;

pub trait SystemPromptSource {
    fn generate_system_prompt(&self, username: &str) -> Result<String>;
}

/// No-nonsense prompt source. Comes directly from the config file.
pub struct BasicSystemPrompt {
    system_prompt: Option<String>,
}

impl BasicSystemPrompt {
    pub fn new(config: &Config) -> Self {
        Self {
            system_prompt: config.system_prompt.clone(),
        }
    }
}

impl SystemPromptSource for BasicSystemPrompt {
    fn generate_system_prompt(&self, _username: &str) -> Result<String> {
        Ok(self.system_prompt.clone().unwrap_or_default())
    }
}

#[derive(Debug, Default)]
pub struct CharaSystemPrompt {
    // TODO I'm kind of envisioning a chara card per sub-agent
    // but as we don't even have sub-agents implemented yet, this will
    // do for now.
    card: Option<TavernCard>,
}

impl CharaSystemPrompt {
    pub fn new(_config: &Config) -> Self {
        Self::default()
    }

    pub fn load_card<P>(&mut self, card: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let taverncard = read_card_json(card)?;
        self.card = Some(taverncard);
        Ok(())
    }
}

impl SystemPromptSource for CharaSystemPrompt {
    fn generate_system_prompt(&self, username: &str) -> Result<String> {
        if let Some(card) = &self.card {
            // TODO Need to do substitution as required by chara card specs.
            match card {
                // FIXME We don't care if it's V1 or V2... hmmm.
                // Our data representation is pretty much broken.
                // Though I suppose it influences whether or not we
                // can access V2 fields.
                TavernCard::V1(data) | TavernCard::V2(data) => {
                    let desc = replace_placeholders(username, &data.name, &data.description);
                    Ok(desc)
                }
            }
        } else {
            Ok(String::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::Config,
        prompt::{CharaSystemPrompt, SystemPromptSource},
    };

    #[test]
    fn test_chara_prompt() {
        let config = Config::load("test/pirate.toml").unwrap();
        let mut source = CharaSystemPrompt::new(&config);
        source.load_card("test/chara-V2.json").unwrap();
        let prompt = source.generate_system_prompt("User").unwrap();
        assert_eq!(prompt, "You are Jack, a sparrow that likes to party.");
    }
}
