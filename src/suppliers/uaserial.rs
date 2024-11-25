use anyhow::anyhow;
use std::{collections::HashMap, sync::OnceLock};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::utils::html,
};

use super::{utils, ContentSupplier};

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

        utils::scrap_page(
            utils::create_client().get(&url),
            content_details_processor(),
        )
        .await
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
        _params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        Err(anyhow!("unimplemented"))
    }
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
                title: html::text_value(".block__breadcrumbs .current"),
                original_title: html::optional_text_value(".header--title .original"),
                image: html::self_hosted_image(URL, ".poster img", "src"),
                description: html::text_value(".player__info .player__description .text"),
                additional_info: html::default_value::<Vec<String>>(),
                similar: html::default_value::<Vec<ContentInfo>>(),
                params: html::join_processors(vec![html::attr_value("#embed", "src")]),
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
