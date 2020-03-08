use super::client::Client;
use super::formatting::header;
use super::trello_error::TrelloError;
use super::trello_object::TrelloObject;

use serde::Deserialize;

type Result<T> = std::result::Result<T, TrelloError>;

#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub url: String,
}

impl Attachment {
    pub fn get_all(client: &Client, card_id: &str) -> Result<Vec<Attachment>> {
        let url = client.get_trello_url(
            &format!("/1/cards/{}/attachments", card_id),
            &[("fields", &Attachment::get_fields().join(","))],
        )?;

        Ok(reqwest::get(url)?.error_for_status()?.json()?)
    }
}

impl TrelloObject for Attachment {
    fn get_type() -> String {
        String::from("Attachment")
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_fields() -> &'static [&'static str] {
        &["id", "name", "url"]
    }

    fn render(&self) -> String {
        [header(&self.name, "-").as_str(), &self.url].join("\n")
    }
}
