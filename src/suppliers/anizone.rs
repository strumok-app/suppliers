use std::sync::OnceLock;

use anyhow::anyhow;

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

#[derive(Default)]
pub struct AnizoneContentSupplier;

impl ContentSupplier for AnizoneContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec![]
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

        utils::scrap_page(
            utils::create_client()
                .get(format!("{SITE_URL}/anime"))
                .query(&[("search", query)]),
            search_items_processor(),
        )
        .await
    }

    async fn load_channel(&self, _channel: &str, _page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        utils::scrap_page(
            utils::create_client().get(format!("{SITE_URL}/anime/{id}")),
            content_details_processor(),
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: &str,
        _langs: Vec<String>,
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
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() != 1 {
            return Err(anyhow!("expected ep_num in params"));
        }

        let ep_num = &params[0];

        let anime_page_res = load_anime_page(id, ep_num, langs).await?;
        let mut results: Vec<ContentMediaItemSource> = vec![];

        results.push(ContentMediaItemSource::Video {
            link: anime_page_res.hls_src,
            description: "Default".to_string(),
            headers: None,
        });

        for sub in anime_page_res.subtitles {
            results.push(ContentMediaItemSource::Subtitle {
                link: sub.src,
                description: sub.label,
                headers: None,
            });
        }

        Ok(results)
    }
}

#[derive(Debug)]
struct Subtitle {
    src: String,
    label: String,
}

#[derive(Debug)]
struct AnimePageResult {
    hls_src: String,
    subtitles: Vec<Subtitle>,
}

async fn load_anime_page(
    id: &str,
    ep_num: &str,
    langs: Vec<String>,
) -> anyhow::Result<AnimePageResult> {
    let url = format!("{SITE_URL}/anime/{id}/{ep_num}");

    let page_content = create_client().get(url).send().await?.text().await?;

    let document = scraper::Html::parse_document(&page_content);
    let player_selector = scraper::Selector::parse("main media-player").unwrap();
    let tracks_selector = scraper::Selector::parse("track[kind='subtitles']").unwrap();

    let player_el = document
        .select(&player_selector)
        .next()
        .ok_or_else(|| anyhow!("player not found for anime {} ep_num {}", id, ep_num))?;

    let hls_src = player_el
        .attr("src")
        .ok_or_else(|| anyhow!("player src not found for anime {} ep_num {}", id, ep_num))?;

    let subtitles: Vec<_> = player_el
        .select(&tracks_selector)
        .filter_map(|track_el| {
            let src = track_el.attr("src")?;
            let label = track_el.attr("label")?;

            if !utils::lang::is_allowed(&langs, label) {
                return None;
            }

            Some(Subtitle {
                src: src.to_string(),
                label: label.to_string(),
            })
        })
        .collect();

    Ok(AnimePageResult {
        hls_src: hls_src.to_string(),
        subtitles,
    })
}

fn search_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new(
            "main > div > div > div.grid > div",
            content_info_processor(),
        )
    })
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(extract_id_from_url)
            .in_scope_flatten("div.h-6.inline > a")
            .unwrap_or_default()
            .boxed(),
        title: html::TextValue::new()
            .map(|s| utils::text::sanitize_text(&s))
            .in_scope("div.h-6.inline > a")
            .unwrap_or_default()
            .boxed(),
        secondary_title: html::items_processor("div.h-4 > span", html::TextValue::new().boxed())
            .map(|s| Some(s.join(", ")))
            .boxed(),
        image: html::attr_value("img", "src"),
    }
    .into()
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "main",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::TextValue::new()
                    .in_scope("h1")
                    .unwrap_or_default()
                    .boxed(),
                original_title: html::default_value(),
                image: html::attr_value("div.mx-auto img", "src"),
                description: html::TextValue::new()
                    .all_nodes()
                    .map(|s| utils::text::sanitize_text(&s))
                    .in_scope("div.text-slate-100 div")
                    .unwrap_or_default()
                    .boxed(),
                additional_info: html::items_processor(
                    "div.text-slate-100 span span",
                    html::TextValue::new().boxed(),
                ),
                similar: html::default_value(),
                params: html::default_value(),
            }
            .boxed(),
        )
    })
}

fn extract_id_from_url(id: String) -> String {
    if !id.is_empty() {
        let offset = SITE_URL.len() + 7;
        return id[offset..].to_string();
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = AnizoneContentSupplier.search("Naruto", 1).await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = AnizoneContentSupplier
            .get_content_details("uyyyn4kf", vec![])
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items() {
        let res = AnizoneContentSupplier
            .load_media_items("47tr68c3", vec![], vec![])
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_item_sources() {
        let res = AnizoneContentSupplier
            .load_media_item_sources(
                "47tr68c3",
                vec!["en".to_string(), "jp".to_string()],
                vec!["2".to_string()],
            )
            .await;
        println!("{res:#?}")
    }
}
