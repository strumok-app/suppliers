use anyhow::{anyhow, Ok};
use scraper::{ElementRef, Selector};
use std::{collections::HashMap, sync::OnceLock};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::utils::{html, playerjs},
};

use super::{
    utils::{self, html::DOMProcessor},
    ContentSupplier,
};

const URL: &str = "https://uaserial.tv";
const SEARCH_URL: &str = "https://uaserial.tv/search";

#[derive(Default)]
pub struct UAserialContentSupplier;

impl ContentSupplier for UAserialContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![
            ContentType::Movie,
            ContentType::Cartoon,
            ContentType::Series,
            ContentType::Anime,
        ]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".into()]
    }

    async fn search(
        &self,
        query: String,
        _types: Vec<String>,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        let requedt_builder = utils::create_client()
            .get(SEARCH_URL)
            .query(&[("query", &query)]);

        utils::scrap_page(requedt_builder, search_items_processor()).await
    }

    async fn load_channel(
        &self,
        channel: String,
        page: u16,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        let url = match get_channels_map().get(&channel) {
            Some(url) => {
                format!("{url}{page}")
            }
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
    ) -> Result<Option<ContentDetails>, anyhow::Error> {
        let url = format!("{URL}/{id}");

        let html = utils::create_client().get(url).send().await?.text().await?;
        let document = scraper::Html::parse_document(&html);
        let root = document.root_element();

        let mut maybe_details = content_details_processor().process(&root);

        if let Some(&mut ref mut details) = maybe_details.as_mut() {
            details.media_items = try_extract_episodes(&root);

            if details.media_items.is_none() {
                details.media_items = try_extract_movie(&root);
            }
        }

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        _id: String,
        _params: Vec<String>,
    ) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
        Err(anyhow!("unimplemented"))
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {

        if params.is_empty() {
            return Err(anyhow!("iframe url expected"));
        }

        let iframe_path = &params[0];
        let url = format!("{URL}{iframe_path}");

        let options = try_extract_iframe_options(url).await?;

        if options.is_empty() {
            return Ok(vec![]);
        }

        let sources_futures = options.iter().map(|(description, link)| {
            playerjs::load_and_parse_playerjs_sources(description, link)
        });

        let results: Vec<ContentMediaItemSource> = futures::future::try_join_all(sources_futures)
            .await?
            .into_iter()
            .flatten()
            .collect();

        Ok(results)
    }
}

fn try_extract_episodes(root: &ElementRef) -> Option<Vec<ContentMediaItem>> {
    static SERIES_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let selector =
        SERIES_SELECTOR.get_or_init(|| Selector::parse("#select-series option").unwrap());

    let items: Vec<_> = root
        .select(selector)
        .enumerate()
        .map(|(number, el)| {
            let url = el.attr("value")?;
            let title = el.text().next()?;

            Some(ContentMediaItem {
                number: number as u32,
                title: title.into(),
                section: None,
                image: None,
                sources: None,
                params: vec![url.into()],
            })
        })
        .flatten()
        .collect();

    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn try_extract_movie(root: &ElementRef) -> Option<Vec<ContentMediaItem>> {
    static MOVIES_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let selector = MOVIES_SELECTOR.get_or_init(|| Selector::parse("#embed").unwrap());

    root.select(selector)
        .map(|el| {
            let url = el.attr("src")?;

            Some(ContentMediaItem {
                number: 0,
                title: "Default".into(),
                section: None,
                image: None,
                sources: None,
                params: vec![url.into()],
            })
        })
        .take(1)
        .collect()
}

async fn try_extract_iframe_options(url: String) -> Result<Vec<(String, String)>, anyhow::Error> {
    const ALLOWED_VIDEO_HOSTS: &'static [&'static str] = &["ashdi", "tortuga"];
    static OPTIONS_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let selector =
        OPTIONS_SELECTOR.get_or_init(|| Selector::parse("option[data-type='link']").unwrap());


    let html = utils::create_client().get(url).send()
        .await?
        .text()
        .await?;
    
    let ref document = scraper::Html::parse_document(&html);
    let root = document.root_element();
    let options: Vec<_> = root
        .select(selector)
        .map(|el| {
            let link = el.attr("value")?;
            let description = el.text().next()?;

            Some((description.to_owned(), link.to_owned()))
        })
        .flatten()
        .filter(|(_, link)| ALLOWED_VIDEO_HOSTS.iter().any(|&host| link.contains(host)))
        .collect();

    Ok(options)
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new(".item > a")
            .in_scope("href")
            .map_optional(|id| extract_id_from_url(id))
            .unwrap()
            .into(),
        title: html::text_value(".item__data > a .name"),
        secondary_title: html::ItemsProcessor::new(
            ".item__data .info__item",
            html::TextValue::new().into(),
        )
        .map(|infos| Some(infos.join(",")))
        .into(),
        image: html::self_hosted_image(URL, ".item > a > .img-wrap > img", "src"),
    }
    .into()
}

fn content_channel_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new("#filters-grid-content .row .col", content_info_processor())
    })
}

fn search_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new(
            "#block-search-page > .block__search__actors .row .col",
            content_info_processor(),
        )
    })
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#container",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value(".header--title  .title"),
                original_title: html::optional_text_value(".header--title .original"),
                image: html::self_hosted_image(URL, ".poster img", "src"),
                description: html::text_value(".player__info .player__description .text"),
                additional_info: html::FlattenProcessor::default()
                    .add_processor(html::items_processor(
                        ".movie-data .movie-data-item",
                        html::JoinProcessors::new(vec![
                            html::text_value(".type"),
                            html::TextValue::new()
                                .all_nodes()
                                .in_scope(".value")
                                .unwrap()
                                .into(),
                        ])
                        .map(|v| v.join(" "))
                        .into(),
                    ))
                    .add_processor(html::items_processor(
                        ".movie__genres__container .selection__badge",
                        html::TextValue::new().into(),
                    ))
                    .map(|v| {
                        v.into_iter()
                            .map(html::sanitize_text)
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .into(),
                similar: html::default_value::<Vec<ContentInfo>>(),
                params: html::default_value::<Vec<String>>(),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static HashMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<HashMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        HashMap::from([
            ("Фільми".into(), format!("{URL}/movie/")),
            ("Серіали".into(), format!("{URL}/serial/")),
            ("Аніме".into(), format!("{URL}/animeukr/")),
            ("Мультфільми".into(), format!("{URL}/cartoon-movie/")),
            ("Мультсеріали".into(), format!("{URL}/cartoon-series/")),
        ])
    })
}

fn extract_id_from_url(mut id: String) -> String {
    id.remove(0);
    id
}
