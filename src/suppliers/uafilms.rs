use std::collections::HashMap;
use std::sync::OnceLock;

use chrono::Datelike;

use super::utils::{self, html};
use super::ContentSupplier;
use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType, MediaType,
};
use crate::suppliers::utils::datalife;

const URL: &str = "https://uafilm.pro";

pub struct UAFilmsContentSupplier;

impl Default for UAFilmsContentSupplier {
    fn default() -> Self {
        Self {}
    }
}

impl ContentSupplier for UAFilmsContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![
            ContentType::Movie,
            ContentType::Series,
            ContentType::Cartoon,
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
        utils::scrap_page(
            datalife::search_request(URL, &query),
            content_info_items_processor(),
        )
        .await
    }

    async fn load_channel(
        &self,
        channel: String,
        page: u16,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        let url = datalife::get_channel_url(get_channels_map(), &channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: String,
    ) -> Result<Option<ContentDetails>, anyhow::Error> {
        let url = datalife::format_id_from_url(URL, &id);

        utils::scrap_page(
            utils::create_client().get(&url),
            content_details_processor(),
        )
        .await
    }

    async fn load_media_items(
        &self,
        _id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
        utils::playerjs::load_and_parse_playerjs(&params[0]).await
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        unimplemented!()
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .scoped("a.movie-title")
            .map_optional(|id| datalife::extract_id_from_url(URL, id))
            .unwrap()
            .into(),
        title: html::text_value("a.movie-title"),
        secondary_title: html::MapValue::new(
            |v| {
                Some(
                    v.into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(","),
                )
            },
            html::join_processors(vec![
                html::text_value(".movie-img>span"),
                html::text_value(".movie-img>.movie-series"),
            ]),
        )
        .into(),
        image: html::self_hosted_image(URL, ".movie-img img", "data-src"),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::ItemsProcessor::new(".movie-item", content_info_processor()))
}

fn content_details_processor() -> &'static html::ScopedProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopedProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopedProcessor::new(
            "#dle-content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value("h1[itemprop='name']"),
                original_title: html::optional_text_value("span[itemprop='alternativeHeadline']"),
                image: html::self_hosted_image(URL, ".m-img>img", "src"),
                description: html::TextValue::new()
                    .scoped(".m-desc")
                    .map_optional(html::sanitize_text)
                    .unwrap()
                    .into(),
                additional_info: html::flatten(vec![
                    html::join_processors(vec![html::text_value(".m-ratings > .mr-item-rate > b")]),
                    html::item_processor(
                        ".m-desc>.m-info>.m-info>.mi-item",
                        html::JoinProcessors::new(vec![
                            html::text_value(".mi-label-desc"),
                            html::text_value(".mi-desc"),
                        ])
                        .map(|v| v.join(" "))
                        .into(),
                    ),
                ]),
                similar: html::item_processor(
                    "#owl-rel a",
                    html::ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map(|s| datalife::extract_id_from_url(URL, s))
                            .into(),
                        title: html::text_value(".rel-movie-title"),
                        secondary_title: html::default_value::<Option<String>>(),
                        image: html::self_hosted_image(URL, "img", "data-src"),
                    }
                    .into(),
                ),
                params: html::join_processors(vec![html::attr_value("src", ".player-box>iframe")]),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static HashMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<HashMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        let now = chrono::Utc::now();
        let year = now.year();

        HashMap::from([
            ("Новинки".into(), format!("{URL}/year/{year}/page/")),
            ("Фільми".into(), format!("{URL}/filmys/page/")),
            ("Серіали".into(), format!("{URL}/serialy/page/")),
            ("Аніме".into(), format!("{URL}/anime/page/")),
            ("Мультфільми".into(), format!("{URL}/cartoons/page/")),
            ("Мультсеріали".into(), format!("{URL}/multserialy/page/")),
        ])
    })
}
