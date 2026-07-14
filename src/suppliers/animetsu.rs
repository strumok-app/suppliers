use std::collections::HashMap;

use serde::Deserialize;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils,
};
use anyhow::anyhow;

const URL: &str = "https://animetsu.live";
const API_URL: &str = "https://animetsu.live/v2/api";
const PROXY_URL: &str = "https://swiftstream.top/proxy";
const PAGE_SIZE: u16 = 20;

#[derive(Default)]
pub struct AnimetsuContentSupplier;

impl ContentSupplier for AnimetsuContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec!["Recent".to_string()]
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
        if page > 1 {
            return Ok(vec![]);
        }

        self.fetch_anim_list(
            format!("{API_URL}/anime/search/"),
            &[("query", query.to_string())],
        )
        .await
    }

    async fn load_channel(&self, _: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        self.fetch_anim_list(
            format!("{API_URL}/anime/search/"),
            &[
                ("sort", "tranding".to_string()),
                ("page", page.to_string()),
                ("per_page", PAGE_SIZE.to_string()),
            ],
        )
        .await
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        let response_str = utils::create_json_client()
            .get(format!("{API_URL}/anime/info/{id}"))
            .header("Referer", URL)
            .send()
            .await?
            .text()
            .await?;

        let item: AnimeDetailsResponse = serde_json::from_str(&response_str)?;

        let title = item.title.english.unwrap_or(item.title.romaji);
        let original_title = item.title.native;
        let image = item.cover_image.large.unwrap_or_default();
        let description = item.description.unwrap_or_default();

        let mut additional_info = vec![];
        if let Some(status) = item.status {
            additional_info.push(format!("Status: {status}"));
        }
        if let Some(format) = item.format {
            additional_info.push(format!("Format: {format}"));
        }
        if let Some(year) = item.year {
            additional_info.push(format!("Year: {year}"));
        }
        if let Some(season) = item.season {
            additional_info.push(format!("Season: {season}"));
        }
        if let Some(total_eps) = item.total_eps {
            additional_info.push(format!("Episodes: {total_eps}"));
        }
        if let Some(duration) = item.duration {
            additional_info.push(format!("Duration: {duration} min"));
        }
        if let Some(score) = item.average_score {
            additional_info.push(format!("Score: {score}"));
        }
        if let Some(genres) = item.genres {
            if !genres.is_empty() {
                additional_info.push(format!("Genres: {}", genres.join(", ")));
            }
        }

        let similar = item
            .recommendations
            .unwrap_or_default()
            .into_iter()
            .map(|rec| ContentInfo {
                id: rec.id,
                title: rec.title.english.unwrap_or(rec.title.romaji),
                secondary_title: rec.title.native,
                image: rec.cover_image.large.unwrap_or_default(),
            })
            .collect();

        Ok(Some(ContentDetails {
            title,
            original_title,
            image,
            description,
            media_type: MediaType::Video,
            additional_info,
            similar,
            media_items: None,
            params: vec![],
        }))
    }

    async fn load_media_items(
        &self,
        id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let response_str = utils::create_json_client()
            .get(format!("{API_URL}/anime/eps/{id}"))
            .header("Referer", URL)
            .send()
            .await?
            .text()
            .await?;

        let mut episodes: Vec<AnimeEpisode> = serde_json::from_str(&response_str)?;

        episodes.sort_by(|a, b| {
            a.ep_num
                .partial_cmp(&b.ep_num)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let items = episodes
            .into_iter()
            .map(|ep| {
                let title = match ep.name.filter(|n| !n.is_empty()) {
                    Some(name) => format!("Episode {} - {}", ep.ep_num, name),
                    None => format!("Episode {}", ep.ep_num),
                };

                ContentMediaItem {
                    title,
                    section: None,
                    image: ep.img.map(|img| format!("{URL}{img}")),
                    sources: None,
                    params: vec![ep.ep_num.to_string()],
                }
            })
            .collect();

        Ok(items)
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        let ep_num = params
            .first()
            .ok_or_else(|| anyhow!("episode number expected in params"))?;

        let servers = ["pahe", "kite", "dio", "meg", "kiss"];
        let source_types = ["sub", "dub"];

        let futures = servers.iter().flat_map(|server| {
            source_types.iter().map(move |source_type| async move {
                let response = self
                    .fetch_episode_sources(id, ep_num, server, source_type)
                    .await;
                response.map(|r| (*source_type, r))
            })
        });

        let results = futures::future::join_all(futures).await;

        let sources = results
            .into_iter()
            .filter_map(|r| r.ok())
            .flat_map(|(source_type, response)| Self::map_sources_response(response, source_type))
            .collect();

        Ok(sources)
    }
}

impl AnimetsuContentSupplier {
    async fn fetch_anim_list(
        &self,
        url: String,
        query: &[(&str, String)],
    ) -> anyhow::Result<Vec<ContentInfo>> {
        let response_str = utils::create_json_client()
            .get(url)
            .query(query)
            .header("Referer", URL)
            .send()
            .await?
            .text()
            .await?;

        let response: AnimeListResponse = serde_json::from_str(&response_str)?;

        let results = response
            .results
            .into_iter()
            .map(|item| {
                let title = item.title.english.unwrap_or(item.title.romaji);
                let image = item.cover_image.large.unwrap_or_default();

                ContentInfo {
                    id: item.id,
                    title,
                    secondary_title: item.title.native,
                    image,
                }
            })
            .collect();

        Ok(results)
    }

    async fn fetch_episode_sources(
        &self,
        id: &str,
        ep_num: &str,
        server: &str,
        source_type: &str,
    ) -> anyhow::Result<EpisodeSourcesResponse> {
        let url = format!(
            "{API_URL}/anime/oppai/{id}/{ep_num}?server={server}&source_type={source_type}"
        );

        let response_str = utils::create_json_client()
            .get(&url)
            .header("Referer", URL)
            .send()
            .await?
            .text()
            .await?;

        Ok(serde_json::from_str(&response_str)?)
    }

    fn map_sources_response(
        response: EpisodeSourcesResponse,
        source_type: &str,
    ) -> Vec<ContentMediaItemSource> {
        let server = response.server.unwrap_or_default();
        let mut sources = vec![];
        let mut index = 1;

        for source in response.sources.unwrap_or_default() {
            let link = if source.need_proxy {
                format!("{PROXY_URL}{}", source.url)
            } else {
                source.url
            };

            sources.push(ContentMediaItemSource::Video {
                link,
                description: format!(
                    "[{server}] {index}. [{source_type}] {}",
                    source.quality.unwrap_or_default()
                ),
                headers: Some(HashMap::from([("Referer".to_string(), URL.to_string())])),
                hls_proxy: false,
            });
            index += 1;
        }

        for sub in response.subs.unwrap_or_default() {
            let link = if sub.need_proxy.unwrap_or(false) {
                format!("{PROXY_URL}{}", sub.url)
            } else {
                sub.url
            };

            sources.push(ContentMediaItemSource::Subtitle {
                link,
                description: format!(
                    "[{server}] {index}. [{source_type}] {}",
                    sub.lang.unwrap_or_default()
                ),
                headers: None,
            });
            index += 1;
        }

        sources
    }
}

#[derive(Debug, Deserialize)]
struct AnimeListResponse {
    results: Vec<AnimeListResultItem>,
}

#[derive(Debug, Deserialize)]
struct AnimeListResultItem {
    id: String,
    title: AnimeTitle,
    cover_image: AnimeCoverImage,
}

#[derive(Debug, Deserialize)]
struct AnimeDetailsResponse {
    title: AnimeTitle,
    cover_image: AnimeCoverImage,
    description: Option<String>,
    status: Option<String>,
    format: Option<String>,
    year: Option<u32>,
    season: Option<String>,
    total_eps: Option<u32>,
    duration: Option<u32>,
    average_score: Option<u32>,
    genres: Option<Vec<String>>,
    recommendations: Option<Vec<AnimeRecommendation>>,
}

#[derive(Debug, Deserialize)]
struct AnimeRecommendation {
    id: String,
    title: AnimeTitle,
    cover_image: AnimeCoverImage,
}

#[derive(Debug, Deserialize)]
struct AnimeEpisode {
    ep_num: f32,
    name: Option<String>,
    img: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EpisodeSourcesResponse {
    sources: Option<Vec<EpisodeSource>>,
    subs: Option<Vec<EpisodeSubtitle>>,
    server: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EpisodeSource {
    url: String,
    quality: Option<String>,
    need_proxy: bool,
}

#[derive(Debug, Deserialize)]
struct EpisodeSubtitle {
    url: String,
    lang: Option<String>,
    need_proxy: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct AnimeTitle {
    romaji: String,
    english: Option<String>,
    native: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnimeCoverImage {
    large: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn animetsu_should_search() {
        let res = AnimetsuContentSupplier.search("dr. stone", 1).await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn animetsu_should_load_channel() {
        let res = AnimetsuContentSupplier.load_channel("", 1).await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn animetsu_should_get_content_details() {
        let res = AnimetsuContentSupplier
            .get_content_details("6989be1a29cf95f4eb03f95d")
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn animetsu_should_load_media_items() {
        let res = AnimetsuContentSupplier
            .load_media_items("6989be1a29cf95f4eb03f95d", vec![])
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn animetsu_should_load_media_item_sources() {
        let res = AnimetsuContentSupplier
            .load_media_item_sources("6989be1a29cf95f4eb03f95d", vec!["1".to_string()])
            .await;
        println!("{res:#?}")
    }
}
