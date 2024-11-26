use anyhow::anyhow;
use std::{collections::HashMap, sync::OnceLock};

use super::utils::{self, html, playerjs};
use super::ContentSupplier;
use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType, MediaType,
};
use crate::suppliers::utils::datalife;

const URL: &str = "https://uaserials.pro";

#[derive(Default)]
pub struct UASerialsProContentSupplier;

impl ContentSupplier for UASerialsProContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map()
            .keys()
            .map(|s| s.clone())
            .collect::<Vec<_>>()
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
            playerjs::load_and_parse_playerjs(
                &params[0],
                playerjs::convert_strategy_season_ep_dub,
            )
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
            .in_scope("a.short-img")
            .map_optional(|id| datalife::extract_id_from_url(URL, id))
            .unwrap()
            .into(),
        title: html::text_value("div.th-title"),
        secondary_title: html::optional_text_value("div.th-title-oname"),
        image: html::self_hosted_image(URL, "a.short-img img", "data-src"),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::ItemsProcessor::new("div.short-item", content_info_processor()))
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#dle-content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value("h1.short-title .oname_ua"),
                original_title: html::optional_text_value(".oname"),
                image: html::self_hosted_image(URL, ".fimg > img", "src"),
                description: html::text_value(".ftext.full-text"),
                additional_info: html::items_processor(
                    "ul.short-list > li:not(.mylists-mobile)",
                    html::TextValue::new().all_nodes().into(),
                ),
                similar: html::default_value::<Vec<ContentInfo>>(),
                params: html::AttrValue::new("data-src")
                    .map(|s| vec![s])
                    .in_scope("#content > .video_box > iframe")
                    .unwrap()
                    .into(),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static HashMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<HashMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        HashMap::from([
            ("Фільми".into(), format!("{URL}/films/page/")),
            ("Серіали".into(), format!("{URL}/series/page/")),
            ("Мультфільми".into(), format!("{URL}/fcartoon/page/")),
            ("Мультсеріали".into(), format!("{URL}/cartoon/page/")),
        ])
    })
}
