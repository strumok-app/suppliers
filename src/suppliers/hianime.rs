use anyhow::anyhow;
use std::sync::OnceLock;

use indexmap::IndexMap;

use crate::{
    models::{ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType},
    suppliers::utils::html,
};

use super::{utils, ContentSupplier};

const URL: &str = "https://hianime.to";

#[derive(Default)]
pub struct HianimeContentSupplier {}

impl ContentSupplier for HianimeContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
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

    async fn search(
        &self,
        query: String,
        types: Vec<String>,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        todo!()
    }

    async fn load_channel(
        &self,
        channel: String,
        page: u16,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        let url = match get_channels_map().get(&channel) {
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
    ) -> Result<Option<ContentDetails>, anyhow::Error> {
        todo!()
    }

    async fn load_media_items(
        &self,
        id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
        todo!()
    }

    async fn load_media_item_sources(
        &self,
        id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        todo!()
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href").into(),
        title: html::text_value(".film-detail .film-name"),
        secondary_title: html::ItemsProcessor::new(
            ".film-detail .fd-infor > *",
            html::TextValue::new().into(),
        )
        .map(|v| Some(v.join(" ")))
        .into(),
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

fn get_channels_map() -> &'static IndexMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<IndexMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        IndexMap::from([
            ("New".into(), format!("{URL}/recently-added?page")),
            ("Most Popular".into(), format!("{URL}/most-popular?page")),
            (
                "Recently Updated".into(),
                format!("{URL}/recently-updated?page"),
            ),
            ("Top Airing".into(), format!("{URL}/top-airing?page")),
            ("Movies".into(), format!("{URL}/movie?page")),
            ("TV Series".into(), format!("{URL}/tv?page")),
        ])
    })
}
