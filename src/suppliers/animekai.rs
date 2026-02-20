use anyhow::anyhow;
use indexmap::IndexMap;

use crate::{
    extractors::megaup,
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{
        self, GenericResponse, create_json_client, enc_dec_app,
        html::{self, DOMProcessor},
    },
};

const URL: &str = "https://anikai.to";
const ENTITY_SECTION: &str = ".watch-section  .container  .entity-section";

pub struct AnimeKaiContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
}

impl Default for AnimeKaiContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("New Releases", format!("{URL}/new-releases")),
                ("Updates", format!("{URL}/updates")),
                ("Ongoing", format!("{URL}/ongoing")),
                ("Recent", format!("{URL}/recent")),
            ]),
            processor_content_details: html::ScopeProcessor::new(
                "main",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value(&format!("{ENTITY_SECTION} h1")),
                    original_title: html::optional_text_value(&format!(
                        "{ENTITY_SECTION} .al-title"
                    )),
                    image: html::attr_value(&format!("{ENTITY_SECTION} .poster img"), "src"),
                    description: html::text_value("#main-entity .desc"),
                    additional_info: html::items_processor(
                        "#main-entity .detail div div",
                        html::TextValue::new()
                            .all_nodes()
                            .map(|s| s.trim_end().to_owned())
                            .boxed(),
                    ),
                    similar: html::default_value(),
                    params: html::join_processors(vec![html::attr_value(
                        &format!("{ENTITY_SECTION} .rate-box"),
                        "data-id",
                    )]),
                }
                .boxed(),
            ),
            processor_content_info_items: html::ItemsProcessor::new(
                ".aitem-wrapper.regular .aitem .inner",
                html::ContentInfoProcessor {
                    id: html::attr_value_map("a.poster", "href", |s| {
                        s.rsplit_once("/")
                            .map(|(_, r)| r.to_string())
                            .unwrap_or_default()
                    }),
                    title: html::text_value("a.title"),
                    secondary_title: html::default_value(),
                    image: html::attr_value("a.poster img", "data-src"),
                }
                .boxed(),
            ),
        }
    }
}

impl ContentSupplier for AnimeKaiContentSupplier {
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
        vec!["en".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let mut request_builder = utils::create_client()
            .get(format!("{URL}/browser"))
            .query(&[("keyword", query)]);

        if page > 1 {
            request_builder = request_builder.query(&[("page", page)]);
        }

        utils::scrap_page(request_builder, &self.processor_content_info_items).await
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

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{URL}/watch/{id}");

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
            return Err(anyhow!("kai_id expected in params"));
        }

        let kai_id = &params[0];

        let media_items = enc_dec_app::kai_db_find(enc_dec_app::KaiBDId::KaiId, kai_id).await?;

        Ok(media_items)
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.is_empty() {
            return Err(anyhow!("token expected in params"));
        }

        let token = &params[0];

        let server_link_infos = load_servers_links_by_token(token).await?;

        let sources_futures = server_link_infos.iter().map(extract_server_source);

        let results: Vec<ContentMediaItemSource> = futures::future::try_join_all(sources_futures)
            .await?
            .into_iter()
            .flatten()
            .collect();

        Ok(results)
    }
}

#[derive(Debug)]
struct ServerLinkInfo {
    id: String,
    name: String,
    lid: String,
}

async fn load_servers_links_by_token(token: &str) -> Result<Vec<ServerLinkInfo>, anyhow::Error> {
    let enc_token = enc_dec_app::kai_enc(token).await?;
    let client = create_json_client();

    let links_list_res_str = client
        .get(format!("{URL}/ajax/links/list?token={token}&_={enc_token}"))
        .send()
        .await?
        .text()
        .await?;

    let links_list_res: GenericResponse = serde_json::from_str(&links_list_res_str)?;
    let links_html = scraper::Html::parse_fragment(&links_list_res.result);
    let server_items_selector = scraper::Selector::parse(".server-items").unwrap();

    let mut result: Vec<ServerLinkInfo> = vec![];
    let elements: Vec<_> = links_html
        .root_element()
        .select(&server_items_selector)
        .collect();

    for el in elements.iter().rev() {
        let maybe_id = el.attr("data-id");

        if let Some(id) = maybe_id {
            for s_el in el.child_elements() {
                let name: String = s_el.text().collect();
                let maybe_lid = s_el.attr("data-lid");

                if let Some(lid) = maybe_lid {
                    result.push(ServerLinkInfo {
                        id: id.to_string(),
                        name,
                        lid: lid.to_string(),
                    });
                }
            }
        }
    }

    Ok(result)
}

async fn extract_server_source(
    info: &ServerLinkInfo,
) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
    let lid = &info.lid;
    let server_token = enc_dec_app::kai_enc(lid).await?;

    let client = create_json_client();
    let server_link_res_str = client
        .get(format!("{URL}/ajax/links/view?id={lid}&_={server_token}"))
        .send()
        .await?
        .text()
        .await?;

    // println!("{server_link_res_str}");

    let server_link_res: GenericResponse = serde_json::from_str(&server_link_res_str)?;

    let link = enc_dec_app::kai_dec(&server_link_res.result).await?;

    // println!("{link}");

    let prefix = format!("[{}] {} -", info.id, info.name);
    let sources = megaup::extract(&link, &prefix).await?;

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_load_channel() {
        let res = AnimeKaiContentSupplier::default()
            .load_channel("New Releases", 1)
            .await;

        println!("{res:?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = AnimeKaiContentSupplier::default().search("fairy", 2).await;

        println!("{res:?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = AnimeKaiContentSupplier::default()
            .get_content_details(
                "konosuba-gods-blessing-on-this-wonderful-world-0kp7",
                vec![],
            )
            .await;

        println!("{res:?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items() {
        let res = AnimeKaiContentSupplier::default()
            .load_media_items(
                "konosuba-gods-blessing-on-this-wonderful-world-0kp7",
                vec![],
                vec!["d4W59g".to_string()],
            )
            .await;

        println!("{res:?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_item_sources() {
        let res = AnimeKaiContentSupplier::default()
            .load_media_item_sources(
                "konosuba-gods-blessing-on-this-wonderful-world-0kp7",
                vec![],
                vec!["Jte4p_jlugjhm3QQ0MuI".to_string()],
            )
            .await;

        println!("{res:?}");
    }
}
