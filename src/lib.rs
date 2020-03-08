#[macro_use]
extern crate log;

mod attachment;
mod client;
mod formatting;
mod label;
mod trello_error;
mod trello_object;

#[cfg(test)]
mod test_lib;

pub use attachment::Attachment;
pub use client::Client;
use formatting::{header, title};
pub use label::Label;
pub use trello_error::TrelloError;
pub use trello_object::TrelloObject;

use colored::*;
use regex::RegexBuilder;
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

// https://developers.trello.com/reference/#list-object
#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub id: String,
    pub name: String,
    pub closed: bool,
    pub cards: Option<Vec<Card>>,
}

impl TrelloObject for List {
    fn get_type() -> String {
        String::from("List")
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_fields() -> &'static [&'static str] {
        &["id", "name", "closed"]
    }

    fn render(&self) -> String {
        let title = header(&self.name, "-").bold().to_string();
        let mut result: Vec<String> = vec![title];
        if let Some(cards) = &self.cards {
            for c in cards {
                trace!("{:?}", c);
                let mut lformat: Vec<String> = vec![];

                if c.desc != "" {
                    lformat.push("[...]".dimmed().to_string());
                }

                if let Some(labels) = &c.labels {
                    for l in labels {
                        lformat.push(l.render());
                    }
                }

                let s = format!("* {} {}", &c.name, lformat.join(" "));

                // trim end in case there is no data presented by lformat
                result.push(s.trim_end().to_string());
            }
        }
        result.join("\n")
    }
}

// https://developers.trello.com/reference/#board-object
#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: String,
    pub name: String,
    pub closed: bool,
    pub url: String,
    pub lists: Option<Vec<List>>,
}

impl TrelloObject for Board {
    fn get_type() -> String {
        String::from("Board")
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_fields() -> &'static [&'static str] {
        &["id", "name", "closed", "url"]
    }

    fn render(&self) -> String {
        let mut result = vec![title(&self.name).bold().to_string()];
        if let Some(lists) = &self.lists {
            for list in lists {
                result.push(String::from(""));
                result.push(list.render());
            }
        }
        result.join("\n")
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

    pub fn apply_attachment(client: &Client, card_id: &str, file: &str) -> Result<Attachment> {
        let url = client.get_trello_url(&format!("/1/cards/{}/attachments", card_id), &[])?;

        let form = reqwest::multipart::Form::new().file("file", file)?;

        Ok(reqwest::Client::new()
            .post(url)
            .multipart(form)
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

impl List {
    pub fn new(id: &str, name: &str, cards: Option<Vec<Card>>) -> List {
        List {
            id: String::from(id),
            name: String::from(name),
            cards: cards,
            closed: false,
        }
    }

    /// Filters cards that match the given label_filter (As a regular expression).
    /// Returns a copy of the original List, with the correct filtering applied.
    ///
    /// ```
    /// use trello::{Card, Label, List};
    ///
    /// let list = List::new(
    ///     "123",
    ///     "TODO",
    ///     Some(vec![
    ///         Card::new("1", "Orange", "", Some(vec![Label::new("", "fruit", "")]), ""),
    ///         Card::new("2", "Green", "", None, ""),
    ///     ])
    /// );
    ///
    /// assert_eq!(
    ///     list.filter("idontexist"),
    ///     List::new(
    ///         "123",
    ///         "TODO",
    ///         Some(vec![]),
    ///     )
    /// );
    ///
    /// assert_eq!(
    ///     list.filter("fruit"),
    ///     List::new(
    ///         "123",
    ///         "TODO",
    ///         Some(vec![
    ///             Card::new("1", "Orange", "", Some(vec![Label::new("", "fruit", "")]), "")
    ///         ])
    ///     )
    /// );
    /// ```
    pub fn filter(&self, label_filter: &str) -> List {
        let re = RegexBuilder::new(label_filter)
            .case_insensitive(true)
            .build()
            .expect("Invalid regex for label filter");

        let closure = |c: &Card| -> bool {
            if let Some(labels) = &c.labels {
                for label in labels {
                    if re.is_match(&label.name) {
                        return true;
                    }
                }
            }
            false
        };

        let mut result = self.clone();
        result.cards = if let Some(cards) = result.cards {
            Some(cards.into_iter().filter(closure).collect())
        } else {
            None
        };
        result
    }

    pub fn create(client: &Client, board_id: &str, name: &str) -> Result<List> {
        let url = client.get_trello_url("/1/lists/", &[])?;

        let params = [("name", name), ("idBoard", board_id)];

        Ok(reqwest::Client::new()
            .post(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn open(client: &Client, list_id: &str) -> Result<List> {
        let url = client.get_trello_url(&format!("/1/lists/{}", &list_id), &[])?;

        let params = [("closed", "false")];

        Ok(reqwest::Client::new()
            .put(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn update(client: &Client, list: &List) -> Result<List> {
        let url = client.get_trello_url(&format!("/1/lists/{}/", &list.id), &[])?;

        let params = [("name", &list.name), ("closed", &list.closed.to_string())];

        Ok(reqwest::Client::new()
            .put(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn get_all_cards(client: &Client, list_id: &str) -> Result<Vec<Card>> {
        let url = client.get_trello_url(
            &format!("/1/lists/{}/cards/", list_id),
            &[("fields", &Card::get_fields().join(","))],
        )?;
        Ok(reqwest::get(url)?.error_for_status()?.json()?)
    }
}

impl Board {
    pub fn new(id: &str, name: &str, lists: Option<Vec<List>>, url: &str) -> Board {
        Board {
            id: String::from(id),
            name: String::from(name),
            url: String::from(url),
            lists: lists,
            closed: false,
        }
    }

    pub fn filter(&self, filter_name: &str) -> Board {
        let mut result = self.clone();

        result.lists = if let Some(lists) = result.lists {
            Some(lists.into_iter().map(|l| l.filter(filter_name)).collect())
        } else {
            None
        };
        result
    }

    /// Retrieves any missing nested content for the given board. This potentially
    /// means one or more network requests in order to retrieve the data. The Board
    /// will be mutated to include all its associated lists. The lists will also in turn
    /// contain the associated card resources.
    pub fn retrieve_nested(&mut self, client: &Client) -> Result<()> {
        self.lists = Some(Board::get_all_lists(client, &self.id, true)?);

        Ok(())
    }

    pub fn create(client: &Client, name: &str) -> Result<Board> {
        let url = client.get_trello_url("/1/boards/", &[])?;

        let params = [("name", name)];

        Ok(reqwest::Client::new()
            .post(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn open(client: &Client, board_id: &str) -> Result<Board> {
        let url = client.get_trello_url(&format!("/1/boards/{}", &board_id), &[])?;

        let params = [("closed", "false")];

        Ok(reqwest::Client::new()
            .put(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn update(client: &Client, board: &Board) -> Result<Board> {
        let url = client.get_trello_url(&format!("/1/boards/{}/", &board.id), &[])?;

        let params = [("name", &board.name), ("closed", &board.closed.to_string())];

        Ok(reqwest::Client::new()
            .put(url)
            .form(&params)
            .send()?
            .error_for_status()?
            .json()?)
    }

    pub fn get_all(client: &Client) -> Result<Vec<Board>> {
        let url = client.get_trello_url(
            "/1/members/me/boards/",
            &[
                ("filter", "open"),
                ("fields", &Board::get_fields().join(",")),
            ],
        )?;

        Ok(reqwest::get(url)?.error_for_status()?.json()?)
    }

    pub fn get(client: &Client, board_id: &str) -> Result<Board> {
        let url = client.get_trello_url(
            &format!("/1/boards/{}", board_id),
            &[("fields", &Board::get_fields().join(","))],
        )?;

        Ok(reqwest::get(url)?.error_for_status()?.json()?)
    }

    pub fn get_all_labels(client: &Client, board_id: &str) -> Result<Vec<Label>> {
        let fields = Label::get_fields().join(",");

        let url = client.get_trello_url(
            &format!("/1/boards/{}/labels", board_id),
            &[("fields", &fields)],
        )?;

        Ok(reqwest::get(url)?.error_for_status()?.json()?)
    }

    pub fn get_all_lists(client: &Client, board_id: &str, cards: bool) -> Result<Vec<List>> {
        let fields = List::get_fields().join(",");
        let mut params = vec![("fields", fields.as_str())];

        if cards {
            params.push(("cards", "open"));
        }

        let url = client.get_trello_url(&format!("/1/boards/{}/lists", board_id), &params)?;

        Ok(reqwest::get(url)?.error_for_status()?.json()?)
    }
}
