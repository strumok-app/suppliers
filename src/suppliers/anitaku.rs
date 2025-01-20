use std::{sync::OnceLock, vec};

use anyhow::anyhow;
use log::error;
use scraper::{selectable::Selectable, Selector};

use crate::{
    extractors::{doodstream, gogostream, mp4upload, streamwish},
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{self, html},
};

use super::ContentSupplier;

const URL: &str = "https://anitaku.bz";
const SEARCH_URL: &str = "https://anitaku.bz/search.html";
const GOGO_AJAX_URL: &str = "https://ajax.gogocdn.net";

#[derive(Default)]
pub struct AnitakuContentSupplier;

impl ContentSupplier for AnitakuContentSupplier {
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
        vec!["en".into()]
    }

    async fn search(&self, query: String) -> anyhow::Result<Vec<ContentInfo>> {
        utils::scrap_page(
            utils::create_client()
                .get(SEARCH_URL)
                .query(&[("keyword", query)]),
            search_result_items_processor(),
        )
        .await
    }

    async fn load_channel(&self, _channel: String, _page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_content_details(
        &self,
        id: String,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        utils::scrap_page(
            utils::create_client().get(format!("{URL}/{id}")),
            content_details_processor(),
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: String,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        static LINKS_SELECTOR: OnceLock<Selector> = OnceLock::new();
        let links_selector = LINKS_SELECTOR.get_or_init(|| Selector::parse("li").unwrap());

        if params.is_empty() {
            return Err(anyhow!("Expected episode id"));
        }

        let anime_name = id.rsplit_once("/").map_or(id.as_str(), |(_, name)| name);

        let content = utils::create_client()
            .get(format!("{GOGO_AJAX_URL}/ajax/load-list-episode"))
            .query(&[("ep_start", "0"), ("ep_end", "9999"), ("id", &params[0])])
            .send()
            .await?
            .text()
            .await?;

        let document = scraper::Html::parse_fragment(&content);
        let result: Vec<_> = document
            .select(links_selector)
            .enumerate()
            .map(|(idx, _el)| {
                let ep_num = idx as u32 + 1;
                ContentMediaItem {
                    number: ep_num,
                    title: format!("{ep_num}  episode"),
                    section: None,
                    image: None,
                    sources: None,
                    params: vec![format!("{anime_name}-episode-{ep_num}")],
                }
            })
            .collect();

        Ok(result)
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.is_empty() {
            return Err(anyhow::anyhow!("episode number expected"));
        }
        let ep_link = &params[0];
        let episode_url = format!("{URL}/{ep_link}");

        let servers = extract_servers(&episode_url).await?;

        //println!("{servers:#?}");

        let sources_futures = servers
            .into_iter()
            .map(|server| load_server_sources(&episode_url, server));

        let sources = futures::future::join_all(sources_futures)
            .await
            .into_iter()
            .flatten()
            .flatten()
            .collect();

        Ok(sources)
    }
    // add code here
}

#[derive(Debug)]
struct AnitakuServer {
    name: String,
    url: String,
}

async fn extract_servers(episode_url: &str) -> anyhow::Result<Vec<AnitakuServer>> {
    static SERVERS_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let servers_selector =
        SERVERS_SELECTOR.get_or_init(|| Selector::parse("div.anime_muti_link > ul li").unwrap());
    static LINK_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let link_selector = LINK_SELECTOR.get_or_init(|| Selector::parse("a").unwrap());

    // println!("{episode_url}");
    let episode_page = utils::create_client()
        .get(episode_url)
        .send()
        .await?
        .text()
        .await?;

    let document = scraper::Html::parse_document(&episode_page);

    let results: Vec<_> = document
        .select(servers_selector)
        .filter_map(|el| {
            let name = el.attr("class")?;
            let url = el.select(link_selector).next()?.attr("data-video")?;

            Some(AnitakuServer {
                name: name.into(),
                url: url.into(),
            })
        })
        .collect();

    Ok(results)
}

async fn load_server_sources(
    episode_url: &str,
    server: AnitakuServer,
) -> Option<Vec<ContentMediaItemSource>> {
    let server_url = &server.url;
    let server_name = &server.name;

    let res = match server.name.as_str() {
        "doodstream" => doodstream::extract(server_url, server_name).await,
        "vidcdn" => gogostream::extract(server_url, server_name).await,
        "streamwish" | "vidhide" => streamwish::extract(server_url, episode_url, server_name).await,
        "mp4upload" => mp4upload::extract(server_url, episode_url, server_name).await,
        _ => return None,
    };

    match res {
        Ok(sources) => Some(sources),
        Err(err) => {
            error!("[anitaku] {server_name} fail to load source link (episode_url: {episode_url}, server: {server_url}): {err}");
            None
        }
    }
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#wrapper",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value(".anime_info_body h1"),
                original_title: html::optional_text_value(".anime_info_body p.other-name a"),
                image: html::attr_value(".anime_info_body .anime_info_body_bg img", "src"),
                description: html::text_value(".anime_info_body .description"),
                additional_info: html::ItemsProcessor::new(
                    ".anime_info_body p.type",
                    html::TextValue::new()
                        .all_nodes()
                        .map(|s| html::sanitize_text(&s))
                        .into(),
                )
                .filter(|s| !s.starts_with("Plot Summary") && !s.starts_with("Other name"))
                .into(),
                similar: html::default_value(),
                params: html::AttrValue::new("value")
                    .map(|s| vec![s])
                    .in_scope("#movie_id")
                    .unwrap_or_default()
                    .into(),
            }
            .into(),
        )
    })
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map(extract_id_from_url)
            .in_scope(".name a")
            .unwrap_or_default()
            .into(),
        title: html::text_value(".name a"),
        secondary_title: html::default_value(),
        image: html::attr_value(".img img", "src"),
    }
    .into()
}

fn search_result_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new(".last_episodes .items li", content_info_processor())
    })
}

fn extract_id_from_url(mut id: String) -> String {
    if !id.is_empty() {
        id.remove(0);
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_search() {
        let res = AnitakuContentSupplier
            .search("Dr Stone".into())
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = AnitakuContentSupplier
            .get_content_details("category/dr-stone-ryuusui".into(), vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = AnitakuContentSupplier
            .load_media_items("category/dr-stone".into(), vec![], vec!["8205".into()])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_for_movie() {
        let res = AnitakuContentSupplier
            .load_media_items(
                "category/dr-stone-ryuusui".into(),
                vec![],
                vec!["12734".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_item_sources() {
        let res = AnitakuContentSupplier
            .load_media_item_sources(
                "category/fairy-tail-100-years-quest".into(),
                vec![],
                vec!["fairy-tail-100-years-quest-episode-20".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
