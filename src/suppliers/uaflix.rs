use std::sync::OnceLock;

use anyhow::Ok;

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
        langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        Ok(None)
    }

    async fn load_media_items(
        &self,
        id: &str,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        Ok(vec![])
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Ok(vec![])
    }
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
                title: html::default_value(),
                original_title: html::default_value(),
                image: html::default_value(),
                description: html::default_value(),
                additional_info: html::default_value(),
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
        let res = UAFlixSupplier.search("Наруто", 0).await;

        println!("{res:#?}");
    }
}
