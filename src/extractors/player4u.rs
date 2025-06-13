use std::sync::OnceLock;

use regex::Regex;
use scraper::Selector;

use crate::{extractors::streamwish, models::ContentMediaItemSource, utils};

#[derive(Debug)]
struct Item {
    url: String,
    title: String,
}

pub async fn extract(
    url: &str,
    referer: &str,
    prefix: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let client = utils::create_client();

    let res = client
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    let items = lookup_items(&res, prefix);

    // println!("{items:#?}");

    let sub_extractors = items.iter().map(|item| {
        let url = &item.url;
        let prefix = &item.title;

        streamwish::extract(url, referer, prefix)
    });

    let sources: Vec<_> = futures::future::join_all(sub_extractors)
        .await
        .into_iter()
        .flatten()
        .flatten()
        .collect();

    Ok(sources)
}

fn lookup_items(res: &str, prefix: &str) -> Vec<Item> {
    let document = scraper::Html::parse_document(res);

    let sel = Selector::parse("li.slide-toggle a.playbtnx").unwrap();

    static ID_RE: OnceLock<Regex> = OnceLock::new();
    let id_re = ID_RE.get_or_init(|| Regex::new(r"id=(?<id>[\w\d]+)").unwrap());

    let player_url = streamwish::PLAYER_URL;
    let items: Vec<_> = document
        .select(&sel)
        .filter_map(|el| {
            let onclick = el.attr("onclick")?;
            let title = el.text().collect::<Vec<_>>().join("");

            let id = id_re
                .captures(onclick)
                .and_then(|c| c.name("id"))
                .map(|m| m.as_str())?;

            Some(Item {
                url: format!("{player_url}/e/{id}"),
                title: format!("[{prefix}] {title}"),
            })
        })
        .collect();
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_extract() {
        let res = extract(
            "https://player4u.xyz/embed?key=The Flash s01e01",
            "https://player4u.xyz",
            "prefix",
        )
        .await;

        println!("{res:#?}")
    }
}
