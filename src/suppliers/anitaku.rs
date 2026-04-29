use std::{collections::HashSet, vec};

use anyhow::anyhow;
use indexmap::IndexMap;
use log::error;
use scraper::Selector;

use crate::{
    extractors::{m3u8_link, packer_hls},
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{
        self,
        html::{self, DOMProcessor, ItrDOMProcessor},
    },
};

const URL: &str = "https://anitaku.to";
const SEARCH_URL: &str = "https://anitaku.to/search.html";

pub struct AnitakuContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
    selector_episodes: scraper::Selector,
    selector_dub: scraper::Selector,
    selector_sub: scraper::Selector,
    selector_hdub: scraper::Selector,
    subs_regex: regex::Regex,
}

impl Default for AnitakuContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("Recent Release", URL.to_string()),
                ("New Season", format!("{URL}/season.html")),
                ("Popular", format!("{URL}/popular.html")),
                ("Movies", format!("{URL}/anime-movies.html")),
            ]),
            processor_content_info_items: html::ItemsProcessor::new(
                ".last_episodes .items li",
                AnitakuContentSupplier::content_info_processor(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "#wrapper",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value_map(".anime_info_body h1", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    original_title: html::optional_text_value(".anime_info_body p.other-name a"),
                    image: html::attr_value(".anime_info_body .anime_info_body_bg img", "src"),
                    description: html::text_value_map(".anime_info_body .description", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    additional_info: html::ItemsProcessor::new(
                        ".anime_info_body p.type",
                        html::TextValue::new()
                            .all_nodes()
                            .map(|s| utils::text::sanitize_text(&s))
                            .boxed(),
                    )
                    .filter(|s| !s.starts_with("Plot Summary") && !s.starts_with("Other name"))
                    .boxed(),
                    similar: html::items_processor(
                        ".recent ul li",
                        html::ContentInfoProcessor {
                            id: html::attr_value_map(
                                "a",
                                "href",
                                AnitakuContentSupplier::extract_id_from_url,
                            ),
                            title: html::attr_value("a", "title"),
                            secondary_title: html::text_value_map(".time_2", |s| {
                                Some(utils::text::sanitize_text(&s))
                            }),
                            image: html::attr_value_map("a div", "style", |s| {
                                utils::text::extract_css_background_url(&s).unwrap_or_default()
                            }),
                        }
                        .boxed(),
                    ),
                    params: html::default_value(),
                }
                .boxed(),
            ),
            selector_episodes: Selector::parse("#load_ep li a").unwrap(),
            selector_dub: Selector::parse(".servers .type_DUB li a").unwrap(),
            selector_sub: Selector::parse(".servers .type_SUB li a").unwrap(),
            selector_hdub: Selector::parse(".servers .type_HSUB li a").unwrap(),
            subs_regex: regex::Regex::new(r"(sub|caption_1)=(?<sub>[a-z0-9_://\.]+)").unwrap(),
        }
    }
}

impl ContentSupplier for AnitakuContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        self.channels_map.keys().map(|&s| s.to_string()).collect()
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

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        utils::scrap_page(
            utils::create_client()
                .get(SEARCH_URL)
                .query(&[("keyword", query), ("page", &page.to_string())]),
            &self.processor_content_info_items,
        )
        .await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = match self.channels_map.get(channel) {
            Some(url) => format!("{url}?page={page}"),
            None => return Err(anyhow!("unknown channel")),
        };

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_info_items,
        )
        .await
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{URL}/category/{id}");

        let html = utils::create_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let document = scraper::Html::parse_document(&html);
        let root = document.root_element();

        let mut maybe_details = self.processor_content_details.process(&root);

        if let Some(&mut ref mut details) = maybe_details.as_mut() {
            let media_items = root
                .select(&self.selector_episodes)
                .filter_map(|el| {
                    let mut href = el.attr("href")?.to_string();
                    let num = el.attr("data-num")?;

                    href.remove(0); // remove leading slash

                    Some(ContentMediaItem {
                        title: format!("Episode {num}"),
                        section: None,
                        sources: None,
                        image: None,
                        params: vec![href],
                    })
                })
                .collect();

            details.media_items = Some(media_items);
            // details.params = self.extract_params(&html).unwrap_or_default()
        }

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        _id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        Err(anyhow!("unimplemented"))
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() != 1 {
            return Err(anyhow!("expected id in params"));
        }

        let url = format!("{}/{}", URL, params[0]);

        let servers = self.extract_servers(&url).await?;

        let subs = self.extract_subs(&servers);

        let futures = servers
            .into_iter()
            .map(|server| self.load_server_sources(&url, server));

        let results = futures::future::join_all(futures).await;
        let mut sources: Vec<_> = results.into_iter().flatten().collect();

        sources.extend(subs);

        Ok(sources)
    }
}

#[derive(Debug)]
struct AnitakuServer {
    group: u8,
    name: String,
    url: String,
}

impl AnitakuContentSupplier {
    fn content_info_processor() -> Box<html::ContentInfoProcessor> {
        html::ContentInfoProcessor {
            id: html::attr_value_map(
                ".name a",
                "href",
                AnitakuContentSupplier::extract_id_from_url,
            ),
            title: html::text_value(".name a"),
            secondary_title: html::text_value_map(".released, .episode", |s| {
                Some(utils::text::sanitize_text(&s))
            }),
            image: html::attr_value(".img img", "src"),
        }
        .into()
    }

    async fn extract_servers(&self, url: &str) -> anyhow::Result<Vec<AnitakuServer>> {
        let html = utils::create_client().get(url).send().await?.text().await?;

        let document = scraper::Html::parse_document(&html);

        let servers: Vec<_> = document
            .select(&self.selector_dub)
            .filter_map(|el| self.extract_server(0, el))
            .chain(
                document
                    .select(&self.selector_sub)
                    .filter_map(|el| self.extract_server(1, el)),
            )
            .chain(
                document
                    .select(&self.selector_hdub)
                    .filter_map(|el| self.extract_server(2, el)),
            )
            .collect();

        Ok(servers)
    }

    fn extract_server(&self, group: u8, el: scraper::ElementRef) -> Option<AnitakuServer> {
        let name = el.text().nth(1)?.trim().to_lowercase();
        let url = el.attr("data-video")?.to_string();

        Some(AnitakuServer { group, name, url })
    }

    async fn load_server_sources(
        &self,
        referer: &str,
        server: AnitakuServer,
    ) -> Vec<ContentMediaItemSource> {
        let group_name = match server.group {
            0 => "[DUB]",
            1 => "[SUB]",
            2 => "[HDUB]",
            _ => "unknown",
        };

        let res = match server.name.as_str() {
            "hd-1" | "hd-2" => {
                m3u8_link::extract(
                    &server.url,
                    referer,
                    format!("{} {}", group_name, server.name),
                    true,
                )
                .await
            }
            "streamhg" | "earnvids" => {
                packer_hls::extract(
                    &server.url,
                    referer,
                    format!("{} {}", group_name, server.name),
                    true,
                )
                .await
            }
            _ => return vec![],
        };

        match res {
            Ok(sources) => sources,
            Err(err) => {
                error!(
                    "[anitaku] fail to load source (server: {}: {})",
                    server.url, err
                );
                vec![]
            }
        }
    }

    fn extract_subs(&self, servers: &[AnitakuServer]) -> Vec<ContentMediaItemSource> {
        let unique_subs: HashSet<_> = servers
            .iter()
            .filter(|s| s.group == 1) // only look in SUB group
            .filter_map(|s| {
                self.subs_regex
                    .captures(&s.url)
                    .and_then(|caps| caps.name("sub").map(|m| m.as_str().to_string()))
            })
            .collect();

        unique_subs
            .into_iter()
            .map(|url| ContentMediaItemSource::Subtitle {
                link: url,
                description: "English".to_string(),
                headers: None,
            })
            .collect()
    }

    fn extract_id_from_url(mut id: String) -> String {
        if id.is_empty() {
            return id;
        }

        id.remove(0); // remove leading slash

        let cutoff = ["-season-", "-episode-"]
            .iter()
            .filter_map(|pat| id.find(pat))
            .min()
            .unwrap_or(id.len());

        id.truncate(cutoff);
        id
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = AnitakuContentSupplier::default().search("naruto", 1).await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_channel() {
        let res = AnitakuContentSupplier::default()
            .load_channel("Recent Release", 2)
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = AnitakuContentSupplier::default()
            .get_content_details("classroom-of-the-elite-iv")
            .await;

        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_item_sources() {
        let res = AnitakuContentSupplier::default()
            .load_media_item_sources("", vec!["classroom-of-the-elite-iv-episode-2".to_string()])
            .await;

        println!("{res:#?}")
    }
}
