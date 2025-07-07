use std::sync::OnceLock;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        self, datalife,
        html::{self, DOMProcessor},
    },
};

use super::ContentSupplier;

use anyhow::{anyhow, Ok};
use indexmap::IndexMap;
use regex::Regex;

const URL: &str = "https://ufdub.com";

#[derive(Default)]
pub struct UFDubContentSupplier;

impl ContentSupplier for UFDubContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.into()).collect()
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

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        if page > 1 {
            return Ok(vec![]);
        }

        utils::scrap_page(
            datalife::search_request(URL, query),
            content_info_items_processor(),
        )
        .await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(get_channels_map(), channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
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

        let result: Vec<_> = re
            .captures_iter(&html)
            .filter_map(|c| {
                Some((
                    c.name("title")?.as_str().to_owned(),
                    c.name("url")?.as_str().to_owned(),
                ))
            })
            .filter(|(title, _)| title != "Трейлер")
            .map(|(title, url)| ContentMediaItem {
                title: title.to_owned(),
                section: None,
                image: None,
                sources: Some(vec![ContentMediaItemSource::Video {
                    link: url.to_owned(),
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
        _id: &str,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        todo!()
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(|s| datalife::extract_id_from_url(URL, s))
            .in_scope_flatten(".short-text > .short-t")
            .unwrap_or_default()
            .boxed(),
        title: html::text_value(".short-text > .short-t"),
        secondary_title: html::ItemsProcessor::new(
            ".short-text > .short-c > a",
            html::TextValue::new().boxed(),
        )
        .map(|v| Some(v.join(",")))
        .boxed(),
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
                    .boxed(),
                original_title: html::TextValue::new()
                    .map(|s| s.trim().to_owned())
                    .in_scope("article > .full-title > h1 > .short-t-or")
                    .boxed(),
                image: html::self_hosted_image(
                    URL,
                    "article > .full-desc > .full-text > .full-poster img",
                    "src",
                ),
                description: html::ItemsProcessor::new(
                    "article > .full-desc > .full-text p",
                    html::TextValue::new().boxed(),
                )
                .map(|v| html::sanitize_text(&v.join("")))
                .boxed(),
                additional_info: html::merge(vec![
                    html::items_processor(
                        "article > .full-desc > .full-info .fi-col-item",
                        html::TextValue::new().all_nodes().boxed(),
                    ),
                    html::items_processor(
                        "article > .full-desc > .full-text > .full-poster .voices",
                        html::TextValue::new().all_nodes().boxed(),
                    ),
                ]),
                similar: html::items_processor(
                    "article > .rels .rel",
                    html::ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map_optional(|s| datalife::extract_id_from_url(URL, s))
                            .unwrap_or_default()
                            .boxed(),
                        title: html::attr_value("img", "alt"),
                        secondary_title: html::default_value(),
                        image: html::self_hosted_image(URL, "img", "src"),
                    }
                    .boxed(),
                ),
                params: html::AttrValue::new("value")
                    .map_optional(|s| vec![s])
                    .in_scope_flatten("article input")
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
            ("Новинки", format!("{URL}/page/")),
            ("Фільми", format!("{URL}/film/page/")),
            ("Серіали", format!("{URL}/serial/page/")),
            ("Аніме", format!("{URL}/anime/page/")),
            ("Мультфільми", format!("{URL}/cartoons/page/")),
            ("Мультсеріали", format!("{URL}/cartoon-serial/page/")),
            ("Дорами", format!("{URL}/dorama/page/")),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = UFDubContentSupplier.load_channel("Аніме", 2).await.unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = UFDubContentSupplier
            .search("Засновник темного шляху", 0)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = UFDubContentSupplier
            .get_content_details("anime/302-the-oni-girl-moia-divchyna-oni", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_serial() {
        let res = UFDubContentSupplier
            .load_media_items(
                "anime/301-zasnovnyk-temnogo-shliakhu-mo-dao-zu-shi",
                vec![],
                vec![String::from("https://video.ufdub.com/AT/VP.php?ID=301")],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_movie() {
        let res = UFDubContentSupplier
            .load_media_items(
                "anime/302-the-oni-girl-moia-divchyna-oni",
                vec![],
                vec![String::from("https://video.ufdub.com/AT/VP.php?ID=302")],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
