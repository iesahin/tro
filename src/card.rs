use super::client::Client;
use super::formatting::header;
use super::label::Label;
use super::trello_error::TrelloError;
use super::trello_object::TrelloObject;

use serde::Deserialize;

type Result<T> = std::result::Result<T, TrelloError>;

// https://developers.trello.com/reference/#card-object
#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub closed: bool,
    pub url: String,
    pub labels: Option<Vec<Label>>,
}

impl TrelloObject for Card {
    fn get_type() -> String {
        String::from("Card")
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_fields() -> &'static [&'static str] {
        &["id", "name", "desc", "labels", "closed", "url"]
    }

    fn render(&self) -> String {
        [header(&self.name, "=").as_str(), &self.desc].join("\n")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CardContents {
    pub name: String,
    pub desc: String,
}

impl Card {
    pub fn new(id: &str, name: &str, desc: &str, labels: Option<Vec<Label>>, url: &str) -> Card {
        Card {
            id: String::from(id),
            name: String::from(name),
            desc: String::from(desc),
            url: String::from(url),
            labels: labels,
            closed: false,
        }
    }

    /// Takes a buffer of contents that represent a Card render and parses
    /// it into a CardContents structure. This is similar to a deserialization process
    /// except this is quite unstructured and is not very strict in order to allow
    /// the user to more easily edit card contents.
    /// ```
    /// # fn main() -> Result<(), trello::TrelloError> {
    /// let buffer = "Hello World\n===\nThis is my card";
    /// let card_contents = trello::Card::parse(buffer)?;
    ///
    /// assert_eq!(
    ///     card_contents,
    ///     trello::CardContents {
    ///         name: String::from("Hello World"),
    ///         desc: String::from("This is my card"),
    ///     },
    /// );
    /// # Ok(())
    /// # }
    /// ```
    /// Invalid data will result in an appropriate error being returned.
    pub fn parse(buffer: &str) -> Result<CardContents> {
        // this is guaranteed to give at least one result
        let mut contents = buffer.split("\n").collect::<Vec<&str>>();
        trace!("{:?}", contents);

        // first line should *always* be the name of the card
        let mut name = vec![contents.remove(0)];

        // continue generating the name until we find a line entirely composed of '-'
        // we cannot calculate header() here because we allow the user the benefit of not
        // having to add or remove characters in case the name grows or shrinks in size
        let mut found = false;
        while contents.len() > 0 {
            let line = contents.remove(0);

            if &line.chars().take_while(|c| c == &'=').collect::<String>() != line {
                name.push(line);
            } else {
                found = true;
                break;
            }
        }

        if !found {
            return Err(TrelloError::CardParse(
                "Unable to find name delimiter '===='".to_owned(),
            ));
        }

        let name = name.join("\n");
        // The rest of the contents is assumed to be the description
        let desc = contents.join("\n");

        Ok(CardContents {
            name: String::from(name),
            desc: String::from(desc),
        })
    }

    pub fn create(client: &Client, list_id: &str, card: &Card) -> Result<Card> {
        let url = client.get_trello_url("/1/cards/", &[])?;

        let params: [(&str, &str); 3] = [
            ("name", &card.name),
            ("desc", &card.desc),
            ("idList", list_id),
        ];

        Ok(reqwest::Client::new()
            .post(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn open(client: &Client, card_id: &str) -> Result<Card> {
        let url = client.get_trello_url(&format!("/1/cards/{}", &card_id), &[])?;

        let params = [("closed", "false")];

        Ok(reqwest::Client::new()
            .put(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn update(client: &Client, card: &Card) -> Result<Card> {
        let url = client.get_trello_url(&format!("/1/cards/{}/", &card.id), &[])?;

        let params = [
            ("name", &card.name),
            ("desc", &card.desc),
            ("closed", &card.closed.to_string()),
        ];

        Ok(reqwest::Client::new()
            .put(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn remove_label(client: &Client, card_id: &str, label_id: &str) -> Result<()> {
        let url =
            client.get_trello_url(&format!("/1/cards/{}/idLabels/{}", card_id, label_id), &[])?;

        reqwest::Client::new()
            .delete(url)
            .send()?
            .error_for_status()?;

        Ok(())
    }

    pub fn apply_label(client: &Client, card_id: &str, label_id: &str) -> Result<()> {
        let url = client.get_trello_url(&format!("/1/cards/{}/idLabels", card_id), &[])?;

        let params = [("value", label_id)];

        reqwest::Client::new()
            .post(url)
            .form(&params)
            .send()?
            .error_for_status()?;

        Ok(())
    }
}
