use anyhow::anyhow;
use indexmap::IndexMap;
use log::error;
use scraper::Selector;
use serde::Deserialize;

use crate::{
    extractors::megaplay,
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{
        self,
        html::{self, DOMProcessor, text_value, text_value_map},
    },
};

const URL: &str = "https://anikototv.to";

pub struct AnikotoContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
    selector_subs: scraper::Selector,
    selector_dubs: scraper::Selector,
    seasons_selector: scraper::Selector,
}

impl Default for AnikotoContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([("New Releases", format!("{URL}/new-release"))]),
            processor_content_info_items: html::ItemsProcessor::new(
                "#list-items > .item > .inner",
                html::ContentInfoProcessor {
                    id: html::attr_value_map(
                        ".poster > a",
                        "href",
                        AnikotoContentSupplier::extract_id_from_url,
                    ),
                    title: text_value(".info a.name"),
                    secondary_title: html::default_value(),
                    image: html::attr_value(".poster img", "src"),
                }
                .boxed(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "body",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value_map("#w-info h1.title", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    original_title: html::optional_attr_value("#w-info h1.title", "data-jp"),
                    image: html::self_hosted_image(URL, "#w-info .poster img", "src"),
                    description: html::text_value("#w-info .synopsis .shorting .content"),
                    additional_info: html::items_processor(
                        "#w-info .bmeta .meta > div",
                        html::TextValue::new()
                            .all_nodes()
                            .map(|s| utils::text::sanitize_text(&s))
                            .boxed(),
                    ),
                    similar: html::items_processor(
                        "aside.sidebar .body > .scaff > .item",
                        html::ContentInfoProcessor {
                            id: html::AttrValue::new("href")
                                .map_optional(AnikotoContentSupplier::extract_id_from_url)
                                .unwrap_or_default()
                                .boxed(),
                            title: text_value_map(".info .name", |s| {
                                utils::text::sanitize_text(&s)
                            }),
                            secondary_title: html::default_value(),
                            image: html::attr_value(".poster img", "src"),
                        }
                        .boxed(),
                    ),
                    params: html::attr_value_map("[data-id]", "data-id", |s| vec![s]),
                }
                .boxed(),
            ),
            selector_subs: Selector::parse(".type[data-type='sub'] ul li").unwrap(),
            selector_dubs: Selector::parse(".type[data-type='dub'] ul li").unwrap(),
            seasons_selector: Selector::parse(".ep-range li").unwrap(),
        }
    }
}

impl ContentSupplier for AnikotoContentSupplier {
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
                .get(format!("{URL}/filter"))
                .query(&[("keyword", query.to_string()), ("page", page.to_string())]),
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
        utils::scrap_page(
            utils::create_client().get(format!("{URL}/watch/{id}/ep-1")),
            &self.processor_content_details,
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if params.is_empty() {
            return Err(anyhow!("data id expected in params"));
        }

        let data_id = &params[0];

        let list_url = format!("{URL}/ajax/episode/list/{data_id}?vrf=");

        // println!("{list_url}");

        let list_response_str = utils::create_json_client()
            .get(list_url)
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", format!("{URL}/watch/{id}"))
            .send()
            .await?
            .text()
            .await?;

        // println!("{list_response_str}");

        let list_response: ListResponse = serde_json::from_str(&list_response_str)?;
        let document = scraper::Html::parse_fragment(&list_response.result);

        let params: Vec<_> = document
            .select(&self.seasons_selector)
            .filter_map(|el| {
                let inner_link = el.child_elements().next()?;
                let data_id = inner_link.attr("data-ids")?;

                let title = el
                    .attr("title")
                    .or_else(|| el.text().next())
                    .unwrap_or_default();

                Some(ContentMediaItem {
                    title: title.to_string(),
                    section: None,
                    image: None,
                    sources: None,
                    params: vec![data_id.to_string()],
                })
            })
            .collect();

        Ok(params)
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.is_empty() {
            return Err(anyhow!("episode id expected"));
        }

        let url = format!("{URL}/watch/{id}/ep-1");

        let episode_id = &params[0];
        let servers = self.extract_servers(id, episode_id).await?;

        // dbg!(&servers);

        let futures = servers
            .into_iter()
            .map(|server| self.load_server_sources(&url, server));

        let results = futures::future::join_all(futures).await;
        let sources: Vec<_> = results.into_iter().flatten().collect();

        Ok(sources)
    }
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    result: String,
}

#[derive(Debug)]
struct AnikotoServer {
    id: String,
    name: String,
    dub: bool,
}

impl AnikotoContentSupplier {
    async fn extract_servers(
        &self,
        id: &str,
        episode_id: &str,
    ) -> anyhow::Result<Vec<AnikotoServer>> {
        let servers_response: ListResponse = utils::create_client()
            .get(format!("{URL}/ajax/server/list"))
            .query(&[("servers", episode_id)])
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", format!("{URL}/watch/{id}"))
            .send()
            .await?
            .json()
            .await?;

        // dbg!(servers_response);

        let document = scraper::Html::parse_fragment(&servers_response.result);

        let servers: Vec<_> = document
            .select(&self.selector_dubs)
            .filter_map(|el| self.extract_server(&el, true))
            .chain(
                document
                    .select(&self.selector_subs)
                    .filter_map(|el| self.extract_server(&el, false)),
            )
            .collect();
        Ok(servers)
    }

    fn extract_server(&self, el: &scraper::ElementRef, dub: bool) -> Option<AnikotoServer> {
        let data_id = el.attr("data-link-id")?;
        let title = el
            .text()
            .map(utils::text::sanitize_text)
            .collect::<Vec<_>>()
            .join("");

        Some(AnikotoServer {
            id: data_id.to_owned(),
            name: utils::text::sanitize_text(&title).to_lowercase(),
            dub: dub,
        })
    }

    async fn load_server_sources(
        &self,
        referer: &str,
        server: AnikotoServer,
    ) -> Vec<ContentMediaItemSource> {
        let server_id = server.id.as_str();
        let server_name = server.name.as_str();

        let title = format!(
            "[{}] {}",
            if server.dub { "dub" } else { "sub" },
            server_name
        );

        let link = match self.load_server_source_link(referer, server_id).await {
            Ok(link) => link,
            Err(err) => {
                error!(
                    "[anikoto] fail to load source link (server_id: {server_id}, server_name: {server_name}): {err}"
                );
                return vec![];
            }
        };

        let res = match server.name.as_str() {
            "vidstream-1" | "vidstream-2" | "vidcloud-1" | "vidcloud-2" | "megaplay-1"
            | "megaplay-2" => megaplay::extract(&link, referer, title, false).await,
            _ => return vec![],
        };

        match res {
            Ok(sources) => sources,
            Err(err) => {
                error!(
                    "[anikoto] fail to load source link (link: {link}, server_name: {server_name}): {err}"
                );
                vec![]
            }
        }
    }

    async fn load_server_source_link(&self, url: &str, server_id: &str) -> anyhow::Result<String> {
        #[derive(Deserialize, Debug)]
        struct SourcesResponse {
            result: SourcesResponseResult,
        }

        #[derive(Deserialize, Debug)]
        struct SourcesResponseResult {
            url: String,
        }

        let sources_resposne: SourcesResponse = utils::create_client()
            .get(format!("{URL}/ajax/server"))
            .query(&[("get", server_id)])
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", url)
            .send()
            .await?
            .json()
            .await?;

        Ok(sources_resposne.result.url)
    }

    fn extract_id_from_url(id: String) -> String {
        id.split("/").nth(4).unwrap_or_default().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = AnikotoContentSupplier::default()
            .search("one piece", 0)
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_channel() {
        let res = AnikotoContentSupplier::default()
            .load_channel("New Releases", 1)
            .await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = AnikotoContentSupplier::default()
            .get_content_details("sakamoto-days-sfdxz")
            .await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items() {
        let res = AnikotoContentSupplier::default()
            .load_media_items("sakamoto-days-sfdxz", vec!["7498".into()])
            .await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_item_sources() {
        let res = AnikotoContentSupplier::default()
            .load_media_item_sources(
                "sakamoto-days-sfdxz",
                vec!["VHd5akNkRmpZSlR3ZmQ0UXNCVG41KzcxR3J0TmpraW9OWFQzUkNqelZJZVA0citBWU1jUTRlL3FQcU01RDVmNyt2b1RYRGJHMG9DMHYwQmk4ZWdNTEZXdWJRamJlYnVQcFd5Zm5uZlpnV053TUU5cWRYNytPRVRoVXkzMW0xTjQvYTJpMWJGTWxFY2gxTVh3L3ZGcHJnPT0".into()],
            )
            .await;
        println!("{res:#?}");
    }
}
