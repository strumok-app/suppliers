use std::sync::OnceLock;

use anyhow::anyhow;
use log::error;

use indexmap::IndexMap;
use reqwest::header;
use scraper::Selector;
use serde::Deserialize;

use crate::extractors::megacloud2 as megacloud;
use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType, MediaType,
};

use crate::utils;
use crate::utils::html::{self, DOMProcessor};

use super::ContentSupplier;

const URL: &str = "https://hianimez.to";
const SEARCH_URL: &str = "https://hianimez.to/search";
// static HIANIME_API: &str = env!("HIANIME_API");

#[derive(Default)]
pub struct HianimeContentSupplier;

impl ContentSupplier for HianimeContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.into()).collect()
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

    async fn search(&self, query: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        utils::scrap_page(
            utils::create_client()
                .get(SEARCH_URL)
                .query(&[("keyword", query), ("page", page.to_string())]),
            content_channel_items_processor(),
        )
        .await
    }

    async fn load_channel(&self, channel: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = match get_channels_map().get(channel.as_str()) {
            Some(url) => format!("{url}={page}"),
            None => return Err(anyhow!("unknown channel")),
        };

        utils::scrap_page(
            utils::create_client().get(&url),
            content_channel_items_processor(),
        )
        .await
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
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        static SEASONS_SELECTOR: OnceLock<Selector> = OnceLock::new();
        let selector =
            SEASONS_SELECTOR.get_or_init(|| Selector::parse(".seasons-block .ep-item").unwrap());

        let data_id = match id.rsplit_once("-") {
            Some((_, data_id)) => data_id,
            None => return Err(anyhow!("not valid id")),
        };

        #[derive(Deserialize)]
        struct ListResponse {
            html: String,
        }

        let list_response: ListResponse = utils::create_client()
            .get(format!("{URL}/ajax/v2/episode/list/{data_id}"))
            .header(header::ACCEPT, "application/json")
            .header(header::REFERER, format!("{URL}/watch/{id}"))
            .send()
            .await?
            .json()
            .await?;

        let document = scraper::Html::parse_fragment(&list_response.html);

        let params: Vec<_> = document
            .select(selector)
            .enumerate()
            .filter_map(|(idx, el)| {
                let data_id = el.attr("data-id")?;
                let title = el.attr("title")?;
                let num = idx + 1;

                Some(ContentMediaItem {
                    title: format!("{num}. {title}"),
                    section: None,
                    image: None,
                    sources: None,
                    params: vec![data_id.to_owned()],
                })
            })
            .collect();

        Ok(params)
    }

    async fn load_media_item_sources(
        &self,
        id: String,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.is_empty() {
            return Err(anyhow!("episode id expected"));
        }

        let episode_id = &params[0];
        let servers = extract_servers(&id, episode_id, langs).await?;

        let mut sources = vec![];

        for server in servers {
            let mut server_sources = load_server_sources(&id, episode_id, &server).await;
            sources.append(&mut server_sources);
        }

        Ok(sources)
    }
}

#[derive(Debug)]
struct HianimeServer {
    id: String,
    title: String,
    dub: bool,
}

async fn extract_servers(
    id: &str,
    episode_id: &str,
    langs: Vec<String>,
) -> anyhow::Result<Vec<HianimeServer>> {
    static SUBS_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let subs_selector =
        SUBS_SELECTOR.get_or_init(|| Selector::parse(".servers-sub .item").unwrap());

    static DUBS_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let dubs_selector =
        DUBS_SELECTOR.get_or_init(|| Selector::parse(".servers-dub .item").unwrap());

    #[derive(Deserialize)]
    struct ServersResponse {
        html: String,
    }

    let servers_response: ServersResponse = utils::create_client()
        .get(format!("{URL}/ajax/v2/episode/servers"))
        .query(&[("episodeId", episode_id)])
        .header("Referer", format!("{URL}/watch/{id}"))
        .send()
        .await?
        .json()
        .await?;

    let document = scraper::Html::parse_fragment(&servers_response.html);

    let mut servers: Vec<HianimeServer> = vec![];

    if langs.contains(&"en".to_owned()) {
        servers.extend(document.select(dubs_selector).filter_map(|el| {
            let data_id = el.attr("data-id")?;
            let title = el
                .text()
                .map(html::sanitize_text)
                .collect::<Vec<_>>()
                .join("");

            Some(HianimeServer {
                id: data_id.to_owned(),
                title: title.to_owned(),
                dub: true,
            })
        }));
    }

    if langs.contains(&"ja".to_owned()) {
        servers.extend(document.select(subs_selector).filter_map(|el| {
            let data_id = el.attr("data-id")?;
            let title = el
                .text()
                .map(html::sanitize_text)
                .collect::<Vec<_>>()
                .join("");

            Some(HianimeServer {
                id: data_id.to_owned(),
                title: html::sanitize_text(&title),
                dub: false,
            })
        }));
    }

    // print!("HianimeServers: {servers:#?}");

    Ok(servers)
}

async fn load_server_sources(
    id: &str,
    episode_id: &str,
    server: &HianimeServer,
) -> Vec<ContentMediaItemSource> {
    let server_id = &server.id;
    let server_name = &server.title.to_lowercase();
    let dub_or_sub = if server.dub { "dub" } else { "sub" };
    let prefix = format!("[{dub_or_sub}] {server_name}");

    let link = match load_server_source_link(id, server_id).await {
        Ok(link) => link,
        Err(err) => {
            error!("[hianime] fail to load source link (id: {id}, server_id: {id}, episode_id: {episode_id}): {err}");
            return vec![];
        }
    };

    let res = match server.title.as_str() {
        "HD-1" | "HD-2" => megacloud::extract(&link, URL, &prefix).await,
        _ => return vec![],
    };

    // println!("{server:#?}: {res:#?}");
    match res {
        Ok(sources) => sources,
        Err(err) => {
            error!("[hianime] fail to load source (id: {id}, server_id: {id}, episode_id: {episode_id}): {err}");
            vec![]
        }
    }
}

async fn load_server_source_link(id: &str, server_id: &str) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct SourcesResponse {
        link: String,
    }

    let sources_resposne: SourcesResponse = utils::create_client()
        .get(format!("{URL}/ajax/v2/episode/sources"))
        .query(&[("id", server_id)])
        .header("Referer", format!("{URL}/{id}"))
        .send()
        .await?
        .json()
        .await?;

    Ok(sources_resposne.link)
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    const ASIDE_SEL: &str = "#ani_detail .ani_detail-stage .anis-content";
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#wrapper",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::TextValue::new()
                    .all_nodes()
                    .map(|s| html::sanitize_text(&s))
                    .in_scope(format!("{ASIDE_SEL} .anisc-detail .film-name").as_str())
                    .unwrap_or_default()
                    .boxed(),
                original_title: html::default_value(),
                image: html::attr_value(format!("{ASIDE_SEL} .anisc-poster img").as_str(), "src"),
                description: html::TextValue::new()
                    .all_nodes()
                    .map(|s| html::sanitize_text(&s))
                    .in_scope(format!("{ASIDE_SEL} .anisc-detail .film-description").as_str())
                    .unwrap_or_default()
                    .boxed(),
                additional_info: html::items_processor(
                    format!("{ASIDE_SEL} .anisc-info-wrap .anisc-info .item:not(.w-hide)").as_str(),
                    html::TextValue::new()
                        .all_nodes()
                        .map(|s| html::sanitize_text(&s))
                        .boxed(),
                ),
                similar: html::items_processor(
                    "#main-sidebar .block_area-content ul.ulclear li",
                    content_info_processor(),
                ),
                params: html::default_value(),
            }
            .boxed(),
        )
    })
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(extract_id_from_url)
            .in_scope(".film-detail .film-name a")
            .flatten()
            .unwrap_or_default()
            .boxed(),
        title: html::TextValue::new()
            .all_nodes()
            .map(|s| html::sanitize_text(&s))
            .in_scope(".film-detail .film-name")
            .unwrap_or_default()
            .boxed(),
        secondary_title: html::default_value(),
        image: html::attr_value(".film-poster > img", "data-src"),
    }
    .into()
}

fn content_channel_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new(
            ".tab-content .film_list-wrap .flw-item",
            content_info_processor(),
        )
    })
}

fn get_channels_map() -> &'static IndexMap<&'static str, String> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, String>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("New", format!("{URL}/recently-added?page")),
            ("Most Popular", format!("{URL}/most-popular?page")),
            ("Recently Updated", format!("{URL}/recently-updated?page")),
            ("Top Airing", format!("{URL}/top-airing?page")),
            ("Movies", format!("{URL}/movie?page")),
            ("TV Series", format!("{URL}/tv?page")),
        ])
    })
}

fn extract_id_from_url(mut id: String) -> String {
    if !id.is_empty() {
        id.remove(0);
        return match id.split_once("?") {
            Some((extracted_id, _)) => extracted_id.to_owned(),
            None => id,
        };
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = HianimeContentSupplier
            .load_channel("Most Popular".into(), 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = HianimeContentSupplier
            .search("Dr Stone".into(), 0)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = HianimeContentSupplier
            .get_content_details("dr-stone-ryuusui-18114".into(), vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = HianimeContentSupplier
            .load_media_items("dr-stone-ryuusui-18114".into(), vec![], vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_item_sources() {
        let res = HianimeContentSupplier
            .load_media_item_sources(
                "dr-stone-ryuusui-18114".into(),
                vec!["en".to_owned(), "ja".to_owned()],
                vec!["92705".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
