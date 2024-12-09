use anyhow::anyhow;
use indexmap::IndexMap;
use std::sync::OnceLock;

use chrono::Datelike;

use super::utils::{self, html, playerjs};
use super::ContentSupplier;
use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType, MediaType,
};
use crate::suppliers::utils::datalife;

const URL: &str = "https://uafilm.pro";

#[derive(Default)]
pub struct UAFilmsContentSupplier;

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
        if !params.is_empty() {
            playerjs::load_and_parse_playerjs(&params[0], playerjs::convert_strategy_dub_season_ep)
                .await
        } else {
            Err(anyhow!("iframe url expected"))
        }
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        Err(anyhow!("unimplemented"))
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .in_scope("a.movie-title")
            .map_optional(|id| datalife::extract_id_from_url(URL, id))
            .flatten()
            .into(),
        title: html::text_value("a.movie-title"),
        secondary_title: html::JoinProcessors::default()
            .add_processor(html::text_value(".movie-img>span"))
            .add_processor(html::text_value(".movie-img>.movie-series"))
            .filter(|s| !s.is_empty())
            .map(|v| Some(v.join(",")))
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

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#dle-content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value("h1[itemprop='name']"),
                original_title: html::optional_text_value("span[itemprop='alternativeHeadline']"),
                image: html::self_hosted_image(URL, ".m-img>img", "src"),
                description: html::TextValue::new()
                    .in_scope(".m-desc")
                    .map_optional(html::sanitize_text)
                    .flatten()
                    .into(),
                additional_info: html::flatten(vec![
                    html::join_processors(vec![html::text_value(".m-ratings > .mr-item-rate > b")]),
                    html::items_processor(
                        ".m-desc>.m-info>.m-info>.mi-item",
                        html::JoinProcessors::new(vec![
                            html::text_value(".mi-label-desc"),
                            html::text_value(".mi-desc"),
                        ])
                        .map(|v| v.join(" "))
                        .into(),
                    ),
                ]),
                similar: html::items_processor(
                    "#owl-rel a",
                    html::ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map(|s| datalife::extract_id_from_url(URL, s))
                            .into(),
                        title: html::text_value(".rel-movie-title"),
                        secondary_title: html::default_value(),
                        image: html::self_hosted_image(URL, "img", "data-src"),
                    }
                    .into(),
                ),
                params: html::join_processors(vec![html::attr_value(".player-box>iframe", "src")]),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static IndexMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<IndexMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        let now = chrono::Utc::now();
        let year = now.year();

        IndexMap::from([
            ("Новинки".into(), format!("{URL}/year/{year}/page/")),
            ("Фільми".into(), format!("{URL}/filmys/page/")),
            ("Серіали".into(), format!("{URL}/serialy/page/")),
            ("Аніме".into(), format!("{URL}/anime/page/")),
            ("Мультфільми".into(), format!("{URL}/cartoons/page/")),
            ("Мультсеріали".into(), format!("{URL}/multserialy/page/")),
        ])
    })
}
