mod models;

use std::sync::OnceLock;

use anyhow::anyhow;
use indexmap::IndexMap;
use log::error;
use models::{DetailsResponse, EpisodesResponse, SearchResponse};
use serde_json::json;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{self, lang},
};

use super::ContentSupplier;

const SITE_URL: &str = "https://anixl.to";
const GRAPHQL_URL: &str = "https://anixl.to/apo/";
const MAX_PAGE: usize = 20;

#[derive(Default)]
pub struct AniXLContentSupplier;

impl ContentSupplier for AniXLContentSupplier {
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
        vec!["en".to_string(), "ja".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let gql = include_str!("./queries/search.graphql");
        let variables = json!({"query": query, "page": page,});
        let body = json!({"query": gql, "variables": variables,});

        let response_str = utils::create_json_client()
            .post(GRAPHQL_URL)
            .header("Referer", SITE_URL)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;

        // println!("{result_str}");

        let response: SearchResponse = serde_json::from_str(&response_str)?;

        // println!("{response:#?}");

        let results: Vec<_> = response
            .data
            .result
            .items
            .into_iter()
            .map(search_anime_node_to_content_info)
            .collect();

        Ok(results)
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let gql = match get_channels_map().get(channel) {
            Some(&ch) => ch,
            None => {
                return Err(anyhow!("channel {channel} not found"));
            }
        };
        let variables = json!({"page": page,});
        let body = json!({"query": gql, "variables": variables,});

        let response_str = utils::create_json_client()
            .post(GRAPHQL_URL)
            .header("Referer", SITE_URL)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;

        // println!("{response_str}");

        let response: SearchResponse = serde_json::from_str(&response_str)?;

        // println!("{response:#?}");

        let results: Vec<_> = response
            .data
            .result
            .items
            .into_iter()
            .map(search_anime_node_to_content_info)
            .collect();

        Ok(results)
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let gql = include_str!("./queries/details.graphql");
        let variables = json!({"id": id,});
        let body = json!({"query": gql, "variables": variables,});

        let response_str = utils::create_json_client()
            .post(GRAPHQL_URL)
            .header("Referer", SITE_URL)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;

        // println!("{result_str}");

        let response: DetailsResponse = serde_json::from_str(&response_str)?;

        let maybe_details = response
            .data
            .map(|data| details_anime_node_to_content_details(data.get_animes_node));

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        id: &str,
        langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let gql = include_str!("./queries/episodes.graphql");
        let mut results: Vec<ContentMediaItem> = vec![];

        let mut has_more = true;
        let mut page: usize = 1;
        while has_more && page < MAX_PAGE {
            let variables = json!({"id": id, "page": page,});
            let body = json!({"query": gql, "variables": variables,});

            let maybe_response = utils::create_json_client()
                .post(GRAPHQL_URL)
                .header("Referer", SITE_URL)
                .json(&body)
                .send()
                .await?
                .text()
                .await;

            let response_str = match maybe_response {
                Ok(r) => r,
                Err(e) => {
                    error!("[anixl] fail to fetch anime {id} episodes list page {page}: {e}");
                    break;
                }
            };

            // println!("{response_str}");

            let response: EpisodesResponse = match serde_json::from_str(&response_str) {
                Ok(s) => s,
                Err(e) => {
                    error!("[anixl] fail to parse anime {id} episodes list page {page}: {e} {response_str}");
                    break;
                }
            };

            for ep in response.data.result.items {
                results.push(episode_to_media_item(ep, &langs));
            }

            page += 1;
            has_more = page < response.data.result.paging.pages + 1;
        }

        Ok(results)
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

fn search_anime_node_to_content_info(node: models::SearchAnimeNode) -> ContentInfo {
    let data = node.data;
    let cover = data.url_cover;

    ContentInfo {
        id: data.ani_id,
        title: data.info_title,
        secondary_title: None,
        image: format!("{SITE_URL}{cover}"),
    }
}

fn details_anime_node_to_content_details(node: models::DetailsAnimeNode) -> ContentDetails {
    let data = node.data;

    ContentDetails {
        title: data.info_title,
        original_title: None,
        image: format!("{SITE_URL}{}", data.url_cover),
        description: utils::text::sanitize_text(&data.info_filmdesc),
        media_type: MediaType::Video,
        additional_info: vec![
            data.score_avg.map(|s| s.to_string()),
            data.info_meta_year,
            data.info_meta_status,
            data.info_meta_date_aired_begin
                .map(|s| format!("Date aired begin: {s}")),
            data.info_meta_date_aired_end
                .map(|s| format!("Date aired end: {s}")),
            data.info_meta_genre.map(|g| g.join(",")),
        ]
        .into_iter()
        .flatten()
        .collect(),
        similar: vec![],
        media_items: None,
        params: vec![],
    }
}

fn episode_to_media_item(ep: models::AnimeEpisodeNode, langs: &[String]) -> ContentMediaItem {
    let episode_data = ep.data;

    let mut sources: Vec<ContentMediaItemSource> = vec![];

    for source in episode_data.sources {
        let source_data = source.data;

        sources.push(ContentMediaItemSource::Video {
            link: source_data.path,
            description: format!(
                "[{}] {}-{}",
                source_data.src_type, source_data.src_server, source_data.src_name
            ),
            headers: None,
        });

        for track in source_data.track {
            if lang::is_allowed(langs, &track.label) {
                sources.push(ContentMediaItemSource::Subtitle {
                    link: track.path,
                    description: track.label,
                    headers: None,
                });
            }
        }
    }

    ContentMediaItem {
        title: episode_data.ep_title,
        section: None,
        image: None,
        sources: Some(sources),
        params: vec![],
    }
}

fn get_channels_map() -> &'static IndexMap<&'static str, &'static str> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, &'static str>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("Latest", include_str!("./queries/latest.graphql")),
            (
                "Most Popular",
                include_str!("./queries/most_popular.graphql"),
            ),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = AniXLContentSupplier.search("Naruto", 0).await.unwrap();
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_most_popular_channel() {
        let res = AniXLContentSupplier
            .load_channel("Most Popular", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_latest_channel() {
        let res = AniXLContentSupplier
            .load_channel("Latest", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = AniXLContentSupplier
            .get_content_details("15956", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items() {
        let res = AniXLContentSupplier
            .load_media_items("15956", vec![], vec![])
            .await
            .unwrap();
        println!("{}", res.len());
        println!("{:#?}", res[0])
    }
}
