use std::{collections::HashMap, sync::OnceLock};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::utils::html::ContentInfoProcessor,
};

use super::{
    utils::{self, datalife, html},
    ContentSupplier,
};

use anyhow::anyhow;
use regex::Regex;

const URL: &str = "https://ufdub.com";

#[derive(Default)]
pub struct UFDubContentSupplier;

impl ContentSupplier for UFDubContentSupplier {
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
        static VIDEO_LIKNS_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
        let re =
        VIDEO_LIKNS_REGEXP.get_or_init(|| Regex::new(r#"\['(?<title>[^']*)','mp4','(?<url>https://ufdub\.com/video/VIDEOS\.php\?[^']*?)'\]"#).unwrap());

        if params.is_empty() {
            return Err(anyhow!("iframe url expected"));
        }

        let html = utils::create_client()
            .get(&params[0])
            .send()
            .await?
            .text()
            .await?;

        let result: Vec<_> = re.captures_iter(&html)
            .map(|c| Some((c.name("title")?, c.name("url")?)))
            .flatten()
            .enumerate()
            .map(|(number, (title, url))| ContentMediaItem {
                number: number as u32,
                title: title.as_str().to_owned(),
                section: None,
                image: None,
                sources: Some(vec![ContentMediaItemSource::Video {
                    link: url.as_str().to_owned(),
                    description: "Default".into(),
                    headers: None,
                }]),
                params: vec![],
            })
            .collect();

        Ok(result)
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        todo!()
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map(|s| datalife::extract_id_from_url(URL, s))
            .in_scope(".short-text > .short-t")
            .unwrap_or_default()
            .into(),
        title: html::text_value(".short-text > .short-t"),
        secondary_title: html::ItemsProcessor::new(
            ".short-text > .short-c > a",
            html::TextValue::new().into(),
        )
        .map(|v| Some(v.join(",")))
        .into(),
        image: html::self_hosted_image(URL, ".short-i img", "src"),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::ItemsProcessor::new(".cont .short", content_info_processor()))
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "div.cols",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::TextValue::new()
                    .map(|s| s.trim().to_owned())
                    .in_scope("article .full-title > h1")
                    .unwrap_or_default()
                    .into(),
                original_title: html::TextValue::new()
                    .map(|s| s.trim().to_owned())
                    .in_scope("article > .full-title > h1 > .short-t-or")
                    .into(),
                image: html::self_hosted_image(
                    URL,
                    "article > .full-desc > .full-text > .full-poster img",
                    "src",
                ),
                description: html::ItemsProcessor::new(
                    "article > .full-desc > .full-text p",
                    html::TextValue::new().into(),
                )
                .map(|v| html::sanitize_text(v.join("")))
                .into(),
                additional_info: html::flatten(vec![
                    html::items_processor(
                        "article > .full-desc > .full-info .fi-col-item",
                        html::TextValue::new().all_nodes().into(),
                    ),
                    html::items_processor(
                        "article > .full-desc > .full-text > .full-poster .voices",
                        html::TextValue::new().all_nodes().into(),
                    ),
                ]),
                similar: html::items_processor(
                    "article > .rels .rel",
                    ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map(|s| datalife::extract_id_from_url(URL, s))
                            .into(),
                        title: html::attr_value("img", "alt"),
                        secondary_title: html::default_value::<Option<String>>(),
                        image: html::self_hosted_image(URL, "img", "src"),
                    }
                    .into(),
                ),
                params: html::AttrValue::new("value")
                    .map(|s| vec![s])
                    .in_scope("article input")
                    .unwrap_or_default()
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
            ("Новинки".into(), format!("{URL}/page/")),
            ("Фільми".into(), format!("{URL}/film/page/")),
            ("Серіали".into(), format!("{URL}/serial/page/")),
            ("Аніме".into(), format!("{URL}/anime/page/")),
            ("Мультфільми".into(), format!("{URL}/cartoons/page/")),
            ("Мультсеріали".into(), format!("{URL}/cartoon-serial/page/")),
            ("Дорами".into(), format!("{URL}/dorama/page/")),
        ])
    })
}
