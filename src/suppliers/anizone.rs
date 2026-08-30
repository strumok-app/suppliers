use anyhow::anyhow;
use serde::Deserialize;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        self, create_client,
        html::{self, DOMProcessor},
    },
};

use super::ContentSupplier;

const SITE_URL: &str = "https://anizone.to";

pub struct AnizoneContentSupplier {
    processor_content_details: html::ScopeProcessor<ContentDetails>,
}

impl Default for AnizoneContentSupplier {
    fn default() -> Self {
        Self {
            processor_content_details: html::ScopeProcessor::new(
                "main",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::attr_value_map("[x-data]", "x-data", extract_title_from_xdata),
                    original_title: html::default_value(),
                    image: html::attr_value("div.mx-auto img", "src"),
                    description: html::text_value_map("div.text-slate-100 div", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    additional_info: html::items_processor(
                        "div.text-slate-100 span span",
                        html::TextValue::new().boxed(),
                    ),
                    similar: html::default_value(),
                    params: html::default_value(),
                }
                .boxed(),
            ),
        }
    }
}

impl ContentSupplier for AnizoneContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec!["Latest Anime".to_string()]
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Anime]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["en".to_string(), "ja".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        if page > 1 {
            return Ok(vec![]);
        }

        let page_content = create_client()
            .get(format!("{SITE_URL}/anime"))
            .query(&[("search", query)])
            .send()
            .await?
            .text()
            .await?;

        parse_anime_items(&page_content)
    }

    async fn load_channel(&self, _channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        if page > 1 {
            return Ok(vec![]);
        }

        let page_content = create_client().get(SITE_URL).send().await?.text().await?;

        parse_anime_items(&page_content)
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        utils::scrap_page(
            utils::create_client().get(format!("{SITE_URL}/anime/{id}")),
            &self.processor_content_details,
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let url = format!("{SITE_URL}/anime/{id}/1");

        let page_content = create_client().get(url).send().await?.text().await?;

        let document = scraper::Html::parse_document(&page_content);
        let selector = scraper::Selector::parse("main div.order-2 div a").unwrap();

        let results: Vec<_> = document
            .select(&selector)
            .enumerate()
            .map(|(i, el)| {
                let ep_num = i + 1;
                let text: String = el.text().collect();

                ContentMediaItem {
                    title: utils::text::sanitize_text(&text),
                    section: None,
                    sources: None,
                    image: None,
                    params: vec![ep_num.to_string()],
                }
            })
            .collect();

        Ok(results)
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() != 1 {
            return Err(anyhow!("expected ep_num in params"));
        }

        let ep_num = &params[0];

        let player_data = self.load_player_data(id, ep_num).await?;
        let mut results: Vec<ContentMediaItemSource> = vec![];

        results.push(ContentMediaItemSource::Video {
            link: player_data.src,
            description: "Default".to_string(),
            headers: None,
            hls_proxy: false,
        });

        for sub in player_data.subtitles {
            results.push(ContentMediaItemSource::Subtitle {
                link: sub.file,
                description: sub.title,
                headers: None,
            });
        }

        Ok(results)
    }
}

/// Player payload embedded in the episode page as
/// `x-data="vidstackPlayer(JSON.parse('...'))"`.
#[derive(Debug, Deserialize)]
struct PlayerData {
    src: String,
    #[serde(default)]
    subtitles: Vec<PlayerSubtitle>,
}

#[derive(Debug, Deserialize)]
struct PlayerSubtitle {
    title: String,
    file: String,
}

impl AnizoneContentSupplier {
    async fn load_player_data(&self, id: &str, ep_num: &str) -> anyhow::Result<PlayerData> {
        let url = format!("{SITE_URL}/anime/{id}/{ep_num}");

        let page_content = create_client().get(url).send().await?.text().await?;

        let json = extract_json_parse_arg(&page_content, "vidstackPlayer(")
            .ok_or_else(|| anyhow!("player data not found for anime {id} ep_num {ep_num}"))?;

        let player_data: PlayerData = serde_json::from_str(&json)?;

        Ok(player_data)
    }
}

/// Anime entry embedded in the page as `items: JSON.parse('...')` inside an
/// Alpine.js `x-data` attribute (used by both the home page swiper and the
/// anime index/search page).
#[derive(Debug, Deserialize)]
struct AnimeItem {
    slug: String,
    main_title: String,
    cover: String,
    #[serde(default)]
    title_list: serde_json::Value,
}

impl From<AnimeItem> for ContentInfo {
    fn from(item: AnimeItem) -> Self {
        let secondary_title = item
            .title_list
            .get("1")
            .and_then(|v| v.as_str())
            .filter(|t| *t != item.main_title)
            .map(|s| s.to_string());

        ContentInfo {
            id: item.slug,
            title: utils::text::sanitize_text(&item.main_title),
            secondary_title,
            image: item.cover,
        }
    }
}

/// Extracts and parses the `items: JSON.parse('...')` payload found in the
/// page's Alpine.js `x-data` attribute.
fn parse_anime_items(page_content: &str) -> anyhow::Result<Vec<ContentInfo>> {
    let json = extract_json_parse_arg(page_content, "items: ")
        .ok_or_else(|| anyhow!("anime items json not found in page"))?;

    let items: Vec<AnimeItem> = serde_json::from_str(&json)?;

    Ok(items.into_iter().map(ContentInfo::from).collect())
}

/// Extracts the string argument of a `JSON.parse('...')` call that immediately
/// follows `prefix` in `content`, and returns it decoded (i.e. ready-to-parse
/// JSON). All quotes/apostrophes inside such payloads are `\uXXXX`-encoded, so
/// the next raw single quote after the opening one is the closing delimiter.
fn extract_json_parse_arg(content: &str, prefix: &str) -> Option<String> {
    let marker = format!("{prefix}JSON.parse('");
    let start = content.find(&marker)? + marker.len();
    let end = content[start..].find('\'')?;
    Some(utils::text::unescape_js_string(
        &content[start..start + end],
    ))
}

fn extract_title_from_xdata(xdata: String) -> String {
    const MARKER: &str = "getTitle(this.anmTitles, '";
    if let Some(start) = xdata.find(MARKER) {
        let title_start = start + MARKER.len();
        if let Some(end) = xdata[title_start..].find('\'') {
            return xdata[title_start..title_start + end].to_string();
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn anizone_should_search() {
        let res = AnizoneContentSupplier::default().search("Naruto", 1).await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn anizone_should_load_channel() {
        let res = AnizoneContentSupplier::default()
            .load_channel("Latest Anime", 1)
            .await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn anizone_should_get_content_details() {
        let res = AnizoneContentSupplier::default()
            .get_content_details("uyyyn4kf")
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn anizone_should_load_media_items() {
        let res = AnizoneContentSupplier::default()
            .load_media_items("47tr68c3", vec![])
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn anizone_should_load_media_item_sources() {
        let res = AnizoneContentSupplier::default()
            .load_media_item_sources("47tr68c3", vec!["2".to_string()])
            .await;
        println!("{res:#?}")
    }
}
