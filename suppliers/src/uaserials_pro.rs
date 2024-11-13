use std::{error::Error, sync::OnceLock};

use scraper::Html;

use crate::{
    html::{self, DOMProcessor},
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentSupplier,
        ContentType, MediaType,
    },
    utils::{self, datalife},
};

pub const NAME: &str = "UaserialsPro";
const URL: &str = "https://uaserials.pro";

pub struct UaserialsProContentSupplier;

impl Default for UaserialsProContentSupplier {
    fn default() -> Self {
        Self {}
    }
}

impl UaserialsProContentSupplier {
    fn extract_id_from_url(mut id: String) -> String {
        // remove site name
        id.replace_range(0..(URL.len() + 1), "");
        // remove .html
        id.replace_range((id.len() - 5)..id.len(), "");
        id
    }

    fn content_info_processor() -> Box<html::ContentInfoProcessor> {
        Box::new(html::ContentInfoProcessor {
            id: html::map_value(
                UaserialsProContentSupplier::extract_id_from_url,
                html::attr_value("href", "a.short-img"),
            ),
            title: html::text_value("div.th-title"),
            secondary_title: html::optional_text_value("div.th-title-oname"),
            image: html::map_value(
                |a| format!("{URL}{a}"),
                html::attr_value("data-src", "a.short-img img"),
            ),
        })
    }

    fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
        static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
            OnceLock::new();
        CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
            html::items_processor_raw(
                "div.short-item",
                UaserialsProContentSupplier::content_info_processor(),
            )
        })
    }

    fn content_details_processor() -> &'static html::ScopedProcessor<ContentDetails> {
        static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopedProcessor<ContentDetails>> =
            OnceLock::new();
        CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
            html::scoped_processor(
                "#dle-content",
                Box::new(html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value("h1.short-title .oname_ua"),
                    original_title: html::optional_text_value(
                        ".oname",
                    ),
                    image: html::optional_map_value(
                        |a| format!("{URL}{a}"),
                        html::optional_attr_value("src", ".fimg > img"),
                    ),
                    description: html::text_value(".ftext.full-text"),
                    additional_info: html::iter_text_values("ul.short-list > li:not(.mylists-mobile)"),
                    similar: html::default_value::<Vec<ContentInfo>>(),
                    params: html::join_processors(
                        vec![html::attr_value("data-src", "#content > .video_box > iframe")]
                    )
                }),
            )
        })
    }

    fn get_channel_url(channel: &str, page: u32) -> Result<String, Box<dyn Error>> {
        match channel {
            "Фільми" => Ok(format!("{URL}/films/page/{page}")),
            "Серіали" => Ok(format!("{URL}/series/page/{page}")),
            "Мультфільми" => Ok(format!("{URL}/fcartoon/page/{page}")),
            "Мультсеріали" => Ok(format!("{URL}/cartoon/page/{page}")),
            _ => Err("unkown channel".into()),
        }
    }
}

impl ContentSupplier for UaserialsProContentSupplier {
    fn get_channels(&self) -> Vec<&str> {
        vec!["Фільми", "Серіали", "Мультфільми", "Мультсеріали"]
    }

    fn get_default_channels(&self) -> Vec<&str> {
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

    fn get_supported_languages(&self) -> Vec<&str> {
        vec!["uk"]
    }

    async fn load_channel(
        &self,
        channel: &str,
        page: u32,
    ) -> Result<Vec<ContentInfo>, Box<dyn Error>> {
        let url = UaserialsProContentSupplier::get_channel_url(channel, page)?;

        let html = utils::create_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let document = Html::parse_document(&html);
        let root = document.root_element();
        let processor = UaserialsProContentSupplier::content_info_items_processor();

        Ok(processor.process(&root))
    }

    async fn search(
        &self,
        query: &str,
        _types: Vec<ContentType>,
    ) -> Result<Vec<ContentInfo>, Box<dyn Error>> {
        let html = datalife::search_request(URL, query)
            .send()
            .await?
            .text()
            .await?;

        let document = Html::parse_document(&html);
        let root = document.root_element();
        let processor = UaserialsProContentSupplier::content_info_items_processor();

        Ok(processor.process(&root))
    }

    async fn get_content_details(
        &self,
        id: &str,
    ) -> Result<Option<ContentDetails>, Box<dyn Error>> {
        let url = format!("{URL}/{id}.html");

        let html = utils::create_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let document = Html::parse_document(&html);
        let root = document.root_element();
        let processor = UaserialsProContentSupplier::content_details_processor();

        Ok(processor.process(&root))
    }

    async fn load_media_items(
        &self,
        _id: &str,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItem>, Box<dyn Error>> {
        utils::playerjs::load_and_parse_playerjs(&params[0]).await
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, Box<dyn Error>> {
        unimplemented!()
    }
}
