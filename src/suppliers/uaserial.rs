use anyhow::{anyhow, Ok};
use indexmap::IndexMap;
use scraper::{ElementRef, Selector};
use std::sync::OnceLock;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        self,
        html::{self, DOMProcessor},
        playerjs,
    },
};

use super::ContentSupplier;

const URL: &str = "https://uaserial.tv";
const SEARCH_URL: &str = "https://uaserial.tv/search";

#[derive(Default)]
pub struct UAserialContentSupplier;

impl ContentSupplier for UAserialContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.into()).collect()
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

    async fn search(&self, query: String) -> anyhow::Result<Vec<ContentInfo>> {
        let request_builder = utils::create_client()
            .get(SEARCH_URL)
            .query(&[("query", &query)]);

        utils::scrap_page(request_builder, search_items_processor()).await
    }

    async fn load_channel(&self, channel: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = match get_channels_map().get(channel.as_str()) {
            Some(url) => format!("{url}{page}"),
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
        _langs: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        Err(anyhow!("unimplemented"))
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
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
        .filter_map(|(number, el)| {
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

async fn try_extract_iframe_options(
    url: String,
) -> anyhow::Result<Vec<(String, String)>, anyhow::Error> {
    const ALLOWED_VIDEO_HOSTS: &[&str] = &["ashdi", "tortuga"];
    static OPTIONS_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let selector =
        OPTIONS_SELECTOR.get_or_init(|| Selector::parse("option[data-type='link']").unwrap());

    let html = utils::create_client().get(url).send().await?.text().await?;

    let document = &scraper::Html::parse_document(&html);
    let root = document.root_element();
    let options: Vec<_> = root
        .select(selector)
        .filter_map(|el| {
            let link = el.attr("value")?;
            let description = el.text().next()?;

            Some((description.to_owned(), link.to_owned()))
        })
        .filter(|(_, link)| ALLOWED_VIDEO_HOSTS.iter().any(|&host| link.contains(host)))
        .collect();

    Ok(options)
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map(extract_id_from_url)
            .in_scope(".item > a")
            .unwrap_or_default()
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
                                .unwrap_or_default()
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
                            .map(|s| html::sanitize_text(&s))
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                    })
                    .into(),
                similar: html::default_value(),
                params: html::default_value(),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static IndexMap<&'static str, String> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, String>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("Фільми", format!("{URL}/movie/")),
            ("Серіали", format!("{URL}/serial/")),
            ("Аніме", format!("{URL}/animeukr/")),
            ("Мультфільми", format!("{URL}/cartoon-movie/")),
            ("Мультсеріали", format!("{URL}/cartoon-series/")),
        ])
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
    async fn should_load_channel() {
        let res = UAserialContentSupplier
            .load_channel("Серіали".into(), 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = UAserialContentSupplier
            .search("Термінатор".into())
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details_for_movie() {
        let res = UAserialContentSupplier
            .get_content_details("movie-the-terminator".into(), vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details_for_tv_show() {
        let res = UAserialContentSupplier
            .get_content_details("universal-basic-guys/season-1".into(), vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_item_sources() {
        let res = UAserialContentSupplier
            .load_media_item_sources(
                "blue-exorcist/season-1".into(),
                vec![],
                vec!["/embed/blue-exorcist/season-1/episode-1".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
