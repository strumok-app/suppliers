use anyhow::anyhow;
use indexmap::IndexMap;
use std::sync::OnceLock;

use super::ContentSupplier;
use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType, MediaType,
};
use crate::utils::html::DOMProcessor;
use crate::utils::{self, datalife, html, playerjs};

const URL: &str = "https://uaserials.film";

#[derive(Default)]
pub struct UASerialsProContentSupplier;

impl ContentSupplier for UASerialsProContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.into()).collect()
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

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(get_channels_map(), channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
        )
        .await
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        utils::scrap_page(
            datalife::search_request(URL, query).query(&[("search_start", page.to_string())]),
            content_info_items_processor(),
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = datalife::format_id_from_url(URL, id);

        utils::scrap_page(
            utils::create_client().get(&url),
            content_details_processor(),
        )
        .await
    }

    async fn load_media_items(
        &self,
        _id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if !params.is_empty() {
            playerjs::load_and_parse_playerjs(&params[0], playerjs::convert_strategy_season_dub_ep)
                .await
        } else {
            Err(anyhow!("iframe url expected"))
        }
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Err(anyhow!("unimplemented"))
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(|id| datalife::extract_id_from_url(URL, id))
            .in_scope_flatten("a.short-img")
            .unwrap_or_default()
            .boxed(),
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
                    html::TextValue::new().all_nodes().boxed(),
                ),
                similar: html::default_value(),
                params: html::AttrValue::new("data-src")
                    .map_optional(|s| vec![s])
                    .in_scope_flatten("#content > .video_box > iframe")
                    .unwrap_or_default()
                    .boxed(),
            }
            .boxed(),
        )
    })
}

fn get_channels_map() -> &'static IndexMap<&'static str, String> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, String>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("Фільми", format!("{URL}/films/page/")),
            ("Серіали", format!("{URL}/series/page/")),
            ("Мультфільми", format!("{URL}/fcartoon/page/")),
            ("Мультсеріали", format!("{URL}/cartoon/page/")),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = UASerialsProContentSupplier
            .load_channel("Серіали", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = UASerialsProContentSupplier
            .search("Зоряний шлях", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = UASerialsProContentSupplier
            .get_content_details("8831-gotel-kokayin", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = UASerialsProContentSupplier
            .load_media_items(
                "8831-gotel-kokayin",
                vec![],
                vec!["https://hdvbua.pro/embed/9123".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
