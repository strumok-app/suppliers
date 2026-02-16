use std::sync::OnceLock;

use anyhow::Ok;
use scraper::{ElementRef, Selector, selector};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{
        self,
        html::{self, DOMProcessor},
    },
};

const URL: &str = "https://uafix.net";
const SEARCH_URL: &str = "https://uafix.net/search.html";

#[derive(Default)]
pub struct UAFlixSupplier;

impl ContentSupplier for UAFlixSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Movie, ContentType::Series]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let client = utils::create_client();

        let request = client
            .get(SEARCH_URL)
            .query(&[("do", "search"), ("subaction", "search"), ("story", query)])
            .query(&[("search_start", (page + 1))]);

        let results = utils::scrap_page(request, content_info_items_processor()).await?;

        Ok(results)
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        Ok(vec![])
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{URL}/{id}/");

        let html = utils::create_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let document = scraper::Html::parse_document(&html);
        let root = document.root_element();

        let mut maybe_details = content_details_processor().process(&root);

        if let Some(&mut ref mut details) = maybe_details.as_mut() {
            details.media_items = try_extract_media_items(root);
        }

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        Ok(vec![])
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Ok(vec![])
    }
}

fn try_extract_media_items(root: ElementRef<'_>) -> Option<Vec<ContentMediaItem>> {
    static SIRIES_SELECTOR: OnceLock<Selector> = OnceLock::new();
    let selector =
        SIRIES_SELECTOR.get_or_init(|| Selector::parse("#sers-wr .video-item a").unwrap());

    root.select(selector).filter_map(|el| {
        let href = el.attr("href")?;
        let ep_id = utils::datalife::format_id_from_url(URL, href);

        Some(ContentMediaItem {
            title: "".to_string(),
            image: None,
            section: None,
            sources: None,
            params: vec![],
        })
    });

    None
}

// default pattern
fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::ItemsProcessor::new(".sres-wrap", content_info_processor()))
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(|link| utils::datalife::extract_id_from_url(URL, link))
            .unwrap_or_default()
            .boxed(),
        title: html::text_value("h2"),
        secondary_title: html::default_value(),
        image: html::self_hosted_image(URL, ".sres-img img", "src"),
    }
    .into()
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#dle-content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value("#ftitle > span"),
                original_title: html::default_value(),
                image: html::self_hosted_image(URL, ".fposter2 img", "src"),
                description: html::text_value_map("#serial-kratko", |text| {
                    utils::text::sanitize_text(&text)
                }),
                additional_info: html::items_processor(
                    "#finfo li",
                    html::TextValue::new().all_nodes().boxed(),
                ),
                similar: html::default_value(),
                params: html::default_value(),
            }
            .boxed(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_search() {
        let res = UAFlixSupplier.search("Наруто", 1).await;

        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_get_content_details() {
        let res = UAFlixSupplier
            .get_content_details("serials/schodennik-z-chuzhozemja", vec![])
            .await;

        println!("{res:#?}")
    }
}
