use std::sync::OnceLock;

use anyhow::anyhow;
use scraper::Selector;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::{ContentSupplier, MangaPagesLoader},
    utils::{
        self, create_client,
        html::{self, DOMProcessor, ItrDOMProcessor},
    },
};

const URL: &str = "https://weebcentral.com";
const PAGE_SIZE: u16 = 32;

#[derive(Default)]
pub struct WeebCentralContentSupplier;

impl ContentSupplier for WeebCentralContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Manga]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["en".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let mut request_builder = utils::create_client()
            .get(format!("{URL}/search/data"))
            .query(&[
                ("text", query),
                ("sort", "Best Match"),
                ("order", "Descending"),
                ("official", "Any"),
                ("anime", "Any"),
                ("adult", "Any"),
                ("display_mode", "Full Display"),
            ]);

        if page > 1 {
            request_builder =
                request_builder.query(&[("offset", PAGE_SIZE * page), ("limit", PAGE_SIZE)]);
        }

        utils::scrap_fragment(request_builder, content_info_items_processor()).await
    }

    async fn load_channel(&self, _channel: &str, _page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{URL}/series/{id}");

        utils::scrap_page(
            utils::create_client().get(&url),
            content_details_processor(),
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: &str,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let maybe_actual_id = id.split_once("/").and_then(|(left, _)| left.into());
        let actual_id = match maybe_actual_id {
            Some(s) => s,
            None => return Err(anyhow!("incorrect id value")),
        };

        let url = format!("{URL}/series/{actual_id}/full-chapter-list");

        static PROCESSOR: OnceLock<html::FilterProcessor<ContentMediaItem>> = OnceLock::new();
        let processor = PROCESSOR.get_or_init(|| {
            html::ItemsProcessor {
                scope: None,
                item_processor: html::ContentMediaItemProcessor {
                    title: html::text_value("a > span:nth-child(2) > span:nth-child(1)"),
                    section: html::default_value(),
                    image: html::default_value(),
                    sources: html::attr_value_map("input", "value", |chapter_id| {
                        Some(vec![ContentMediaItemSource::Manga {
                            description: "Default".to_string(),
                            headers: None,
                            pages: None,
                            params: vec![chapter_id],
                        }])
                    }),
                    params: html::default_value(),
                }
                .boxed(),
            }
            .filter(|i| !i.title.is_empty())
        });

        utils::scrap_fragment(create_client().get(&url), processor).await
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

impl MangaPagesLoader for WeebCentralContentSupplier {
    async fn load_pages(&self, _id: &str, params: Vec<String>) -> anyhow::Result<Vec<String>> {
        if params.len() != 1 {
            return Err(anyhow!("expected singe param"));
        }

        let chapter_id = &params[0];

        let html = utils::create_client()
            .get(format!("{URL}/chapters/{chapter_id}/images"))
            .query(&[("is_prev", "False"), ("reading_style", "long_strip")])
            .send()
            .await?
            .text()
            .await?;

        static SELECTOR: OnceLock<Selector> = OnceLock::new();
        let selector = SELECTOR.get_or_init(|| Selector::parse("section > img").unwrap());

        let doc = scraper::Html::parse_fragment(&html);

        let pages: Vec<_> = doc
            .root_element()
            .select(selector)
            .filter_map(|el| el.attr("src"))
            .map(|s| s.to_string())
            .collect();

        Ok(pages)
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(extract_id)
            .unwrap_or_default()
            .boxed(),
        title: html::text_value("article > div:not([class]) > div.bottom-0 > div"),
        secondary_title: html::default_value(),
        image: html::attr_value("article > picture > img", "src"),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new("article > section > a", content_info_processor())
    })
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "main",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value("#top > section > section > h1"),
                original_title: html::default_value(),
                image: html::attr_value(
                    "#top > section > section:nth-child(1) > section > picture > img",
                    "src",
                ),
                description: html::text_value(
                    "#top > section > section:nth-child(2) > section:nth-child(3) ul li p",
                ),
                additional_info: html::items_processor(
                    "#top > section > section:nth-child(1) > section:nth-child(5) ul li",
                    html::TextValue::new()
                        .all_nodes()
                        .map(|s| utils::text::sanitize_text(&s))
                        .boxed(),
                )
                .filter(|s| !s.starts_with("RSS") && !s.starts_with("Track"))
                .boxed(),
                similar: html::items_processor(
                    "#top > section:nth-child(2) > div ul li",
                    html::ContentInfoProcessor {
                        id: html::attr_value_map("a", "href", extract_id),
                        title: html::text_value("a > div > div:nth-child(2) div"),
                        secondary_title: html::default_value(),
                        image: html::attr_value("a > div > div:nth-child(1) img", "src"),
                    }
                    .boxed(),
                ),
                params: html::default_value(),
            }
            .boxed(),
        )
    })
}

fn extract_id(text: String) -> String {
    static OFFSET: usize = URL.len() + 8usize;
    text[OFFSET..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_serach() {
        let res = WeebCentralContentSupplier.search("fairy", 1).await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn shoudl_get_content_details() {
        let res = WeebCentralContentSupplier
            .get_content_details("01J76XY7E5E1C5Y9J0M2FCVQ8H/Fairy-Tail", vec![])
            .await;

        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items() {
        let res = WeebCentralContentSupplier
            .load_media_items("01J76XY7E5E1C5Y9J0M2FCVQ8H/Fairy-Tail", vec![], vec![])
            .await;

        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_pages() {
        let res = WeebCentralContentSupplier
            .load_pages("", vec!["01J76XYY73BP96JM3WRJSTBVMX".to_string()])
            .await;

        println!("{res:#?}")
    }
}
