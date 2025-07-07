mod models;

use models::SearchResponse;
use serde_json::json;

use crate::{
    models::{ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType},
    utils,
};

use super::ContentSupplier;

const SITE_URL: &str = "https://anixl.to";
const GRAPHQL_URL: &str = "https://anixl.to/apo/";

#[derive(Default)]
struct AniXLContentSupplier;

impl ContentSupplier for AniXLContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec![]
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
            .get_search_anime
            .items
            .into_iter()
            .map(anime_node_to_content_info)
            .collect();

        Ok(results)
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        todo!()
    }

    async fn get_content_details(
        &self,
        id: &str,
        langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        todo!()
    }

    async fn load_media_items(
        &self,
        id: &str,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        todo!()
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        todo!()
    }
}

fn anime_node_to_content_info(item: models::AnimeNode) -> ContentInfo {
    let data = item.data;
    ContentInfo {
        id: data.ani_id,
        title: data.info_title,
        secondary_title: None,
        image: data.url_cover,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn search() {
        let res = AniXLContentSupplier.search("Naruto", 0).await.unwrap();
        println!("{res:#?}");
    }
}
