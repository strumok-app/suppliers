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

use anyhow::{Ok, anyhow};
use indexmap::IndexMap;
use regex::Regex;

const URL: &str = "https://ufdub.com";

pub struct UFDubContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
    re_video_links: Regex,
}

impl Default for UFDubContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("Новинки", format!("{URL}/page/")),
                ("Фільми", format!("{URL}/film/page/")),
                ("Серіали", format!("{URL}/serial/page/")),
                ("Аніме", format!("{URL}/anime/page/")),
                ("Мультфільми", format!("{URL}/cartoons/page/")),
                ("Мультсеріали", format!("{URL}/cartoon-serial/page/")),
                ("Дорами", format!("{URL}/dorama/page/")),
            ]),
            processor_content_info_items: html::ItemsProcessor::new(
                ".cont .short",
                html::ContentInfoProcessor {
                    id: html::attr_value_map(".short-text > .short-t", "href", |s| {
                        datalife::extract_id_from_url(URL, s)
                    }),
                    title: html::text_value(".short-text > .short-t"),
                    secondary_title: html::ItemsProcessor::new(
                        ".short-text > .short-c > a",
                        html::TextValue::new().boxed(),
                    )
                    .map(|v| Some(v.join(",")))
                    .boxed(),
                    image: html::self_hosted_image(URL, ".short-i img", "src"),
                }
                .boxed(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "div.cols",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value_map("article .full-title > h1", |s| {
                        s.trim().to_owned()
                    }),
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
                    .map(|v| utils::text::sanitize_text(&v.join("")))
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
                    params: html::attr_value_map("article input", "value", |s| vec![s]),
                }
                .boxed(),
            ),
            re_video_links: Regex::new(r#"\['(?<title>[^']*)','mp4','(?<url>https://ufdub\.com/video/VIDEOS\.php\?[^']*?)'\]"#).unwrap()
        }
    }
}

impl ContentSupplier for UFDubContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        self.channels_map.keys().map(|&s| s.into()).collect()
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
            &self.processor_content_info_items,
        )
        .await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(&self.channels_map, channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_info_items,
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
            &self.processor_content_details,
        )
        .await
    }

    async fn load_media_items(
        &self,
        _id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if params.is_empty() {
            return Err(anyhow!("iframe url expected"));
        }

        let html = utils::create_client()
            .get(&params[0])
            .send()
            .await?
            .text()
            .await?;

        let result: Vec<_> = self
            .re_video_links
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = UFDubContentSupplier::default()
            .load_channel("Аніме", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = UFDubContentSupplier::default()
            .search("Засновник темного шляху", 0)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = UFDubContentSupplier::default()
            .get_content_details("anime/302-the-oni-girl-moia-divchyna-oni", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_serial() {
        let res = UFDubContentSupplier::default()
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
        let res = UFDubContentSupplier::default()
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
