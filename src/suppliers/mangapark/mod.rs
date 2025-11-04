mod models;

use std::sync::OnceLock;

use anyhow::anyhow;
use indexmap::IndexMap;
use serde_json::json;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::{
        ContentSupplier, MangaPagesLoader,
        mangapark::models::{
            ChapterNode, ComicNodeData, DetailsResponse, LatestItem, LatestResponse, PagesResponse,
            SearchComicNode, SearchResponse,
        },
    },
    utils,
};

const SITE_URL: &str = "https://mangapark.org";
const GRAPHQL_URL: &str = "https://mangapark.org/apo/";

#[derive(Default)]
pub struct MangaParkContentSupplier;

impl ContentSupplier for MangaParkContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.into()).collect()
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

        let response: SearchResponse = serde_json::from_str(&response_str)?;

        let results: Vec<_> = response
            .data
            .result
            .items
            .into_iter()
            .map(search_comic_node_to_content_info)
            .collect();

        Ok(results)
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let gql = include_str!("./queries/latest.graphql");
        let maybe_channel = get_channels_map().get(channel);
        let mut variables = match maybe_channel.as_ref() {
            Some(&ch) => ch.as_object().unwrap().clone(),
            None => {
                return Err(anyhow!("channel {channel} not found"));
            }
        };

        variables.insert("page".to_string(), page.into());

        let body = json!({"query": gql, "variables": variables,});

        let response_str = utils::create_json_client()
            .post(GRAPHQL_URL)
            .header("Referer", SITE_URL)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;

        let response: LatestResponse = serde_json::from_str(&response_str)?;

        let results: Vec<_> = response
            .data
            .result
            .items
            .into_iter()
            .map(latest_response_to_content_info)
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

        let response: DetailsResponse = serde_json::from_str(&response_str)?;

        let maybe_details = response
            .data
            .comic_node
            .data
            .map(|data| comic_node_to_content_details(data, response.data.chapter_list));

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        _id: &str,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        Err(anyhow!("unimplemented"))
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

impl MangaPagesLoader for MangaParkContentSupplier {
    async fn load_pages(&self, _id: &str, params: Vec<String>) -> anyhow::Result<Vec<String>> {
        if params.len() != 1 {
            return Err(anyhow!("expected singe param"));
        }

        let chapter_id = &params[0];

        let gql = include_str!("./queries/pages.graphql");
        let variables = json!({"id": chapter_id,});
        let body = json!({"query": gql, "variables": variables,});

        let response_str = utils::create_json_client()
            .post(GRAPHQL_URL)
            .header("Referer", SITE_URL)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;

        let pages_res: PagesResponse = serde_json::from_str(&response_str)?;
        let pages = pages_res.data.chapter_node.data.image_file.url_list;

        Ok(pages)
    }
}

fn comic_node_to_content_details(
    node: ComicNodeData,
    chapters: Vec<ChapterNode>,
) -> ContentDetails {
    let mut additional_info: Vec<String> = vec![];

    if let Some(score) = node.score_val
        && score != 0.0
    {
        additional_info.push(format!("Score: {score:.2}"));
    }

    if !node.alt_names.is_empty() {
        additional_info.push(format!("Alt Names: {}", node.alt_names.join(", ")));
    }

    if !node.artists.is_empty() {
        additional_info.push(format!("Artists: {}", node.artists.join(", ")));
    }

    if !node.authors.is_empty() {
        additional_info.push(format!("Authors: {}", node.authors.join(", ")));
    }

    if !node.genres.is_empty() {
        additional_info.push(format!("Genres: {}", node.genres.join(", ")));
    }

    additional_info.push(format!(
        "Status: {}",
        node.upload_status.unwrap_or(node.original_status)
    ));

    let media_items: Vec<_> = chapters
        .into_iter()
        .map(|ch| {
            let data = ch.data;
            let dname = data.dname;
            let dname_ref = &dname;

            let sources: Vec<_> = data
                .dup_chapters
                .into_iter()
                .map(|dub| ContentMediaItemSource::Manga {
                    description: dub.data.src_title.unwrap_or_else(|| dname_ref.clone()),
                    headers: None,
                    pages: None,
                    params: vec![dub.data.id],
                })
                .collect();

            let name = if let Some(title) = data.title {
                format!("{}. {}", dname, title)
            } else {
                dname
            };

            ContentMediaItem {
                title: name,
                section: None,
                sources: Some(sources),
                image: None,
                params: vec![],
            }
        })
        .collect();

    ContentDetails {
        title: node.name,
        original_title: node.alt_names.into_iter().next(),
        image: node
            .cover_url
            .map(|s| format!("{}{}", SITE_URL, s))
            .unwrap_or_default(),
        description: node
            .summary
            .map(|s| utils::text::sanitize_text(&s))
            .unwrap_or_default(),
        media_type: MediaType::Manga,
        additional_info,
        similar: vec![],
        media_items: Some(media_items),
        params: vec![],
    }
}

fn search_comic_node_to_content_info(node: SearchComicNode) -> ContentInfo {
    let data = node.data;

    ContentInfo {
        id: data.id,
        title: data.name,
        secondary_title: data.alt_names.into_iter().next(),
        image: data
            .cover_url
            .map(|s| format!("{SITE_URL}{s}"))
            .unwrap_or_default(),
    }
}

fn latest_response_to_content_info(item: LatestItem) -> ContentInfo {
    let data = item.data;
    let cover = data.cover_url.unwrap_or_default();

    let mut secondary_title_parts = Vec::new();
    if let Some(ch) = data.last_chapters.iter().next() {
        secondary_title_parts.push(ch.data.dname.clone());
    }
    secondary_title_parts.push(data.tran_lang);
    if data.score_val != 0.0 {
        secondary_title_parts.push(format!("{:.2}", data.score_val));
    }
    let secondary_title = secondary_title_parts.join(", ");

    ContentInfo {
        id: data.id,
        title: data.name,
        secondary_title: Some(secondary_title),
        image: format!("{SITE_URL}{cover}"),
    }
}

fn get_channels_map() -> &'static IndexMap<&'static str, serde_json::Value> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, serde_json::Value>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("Popular Updates", json!({"select": {"where": "popular"}})),
            ("Member Uploads", json!({"select": {"where": "uploads"}})),
            ("Latest Releases", json!({"select": {"where": "release"}})),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_load_popular_channel() {
        let res = MangaParkContentSupplier
            .load_channel("Popular Updates", 1)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_members_upload_channel() {
        let res = MangaParkContentSupplier
            .load_channel("Member Uploads", 1)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = MangaParkContentSupplier.search("chainsaw man", 0).await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = MangaParkContentSupplier
            .get_content_details("428464", vec![])
            // .get_content_details("74763", vec![])
            .await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_pages() {
        let res = MangaParkContentSupplier
            .load_pages("", vec!["9906653".to_string()])
            .await;

        println!("{res:#?}");
    }
}
