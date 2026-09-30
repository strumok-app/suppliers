mod models;

use std::collections::{BTreeMap, HashSet};

use anyhow::{Ok, anyhow};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;

use crate::{
    extractors::moonanime,
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{self},
};

const SITE_URL: &str = "https://animeon.club/";
const API_URL: &str = "https://animeon.club/api/anime";
const API_IMAGE_URL: &str = "https://animeon.club/api/uploads/images";
const API_PLAYER_URL: &str = "https://animeon.club/api/player";
const EPISODES_PAGE_SIZE: usize = 1000;

pub struct AnimeONContentSupplier {
    channels_map: IndexMap<&'static str, &'static str>,
}

impl Default for AnimeONContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("Останій реліз", "sort=desc&sortType=created&search="),
                ("Нові", "sort=desc&sortType=date-out&search="),
                ("Популярні", "sort=desc&sortType=rating&search="),
            ]),
        }
    }
}

impl ContentSupplier for AnimeONContentSupplier {
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
        vec!["uk".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let response_str = utils::create_json_client()
            .get(API_URL)
            .query(&[("search", query), ("pageIndex", &page.to_string())])
            .send()
            .await?
            .text()
            .await?;

        let response: models::SearchResponse = serde_json::from_str(&response_str)?;
        Ok(Self::parse_serach_response(response))
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = match self.channels_map.get(channel) {
            Some(&params) => format!("{API_URL}?{params}&page={page}"),
            None => return Err(anyhow!("unknown channel")),
        };

        // println!("url: {url}");

        let response_str = utils::create_json_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        // println!("response_str: {response_str}");

        let response: models::SearchResponse = serde_json::from_str(&response_str)?;
        Ok(Self::parse_serach_response(response))
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{API_URL}/{id}");
        let maybe_response_str = utils::create_json_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await
            .ok();

        if let Some(response_str) = maybe_response_str {
            // println!("response_str: {response_str}");
            let response: models::DetailsResponse = serde_json::from_str(&response_str)?;
            return Ok(Some(Self::parse_details_response(response)));
        }

        Ok(None)
    }

    async fn load_media_items(
        &self,
        id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        // id is a slug like "175-povsyakdennoshchi", player api expects numeric anime id
        let anime_id = id.split('-').next().unwrap_or(id);

        let response: models::TranslationsResponse =
            Self::get_json(&format!("{API_PLAYER_URL}/{anime_id}/translations")).await?;

        let mut media_items: BTreeMap<i32, ContentMediaItem> = BTreeMap::new();

        for item in response.translations {
            for player in item.player {
                let description = format!("{} ({})", item.translation.name, player.name);
                let episodes =
                    Self::load_episodes(anime_id, player.id, item.translation.id).await?;

                if episodes.is_empty() {
                    // no episodes list (e.g. movie) - use direct player endpoint
                    let path = format!("{}/{}", player.id, item.translation.id);
                    Self::add_source_params(&mut media_items, 1, None, &description, path);
                    continue;
                }

                for ep in episodes {
                    let path = format!("{}/episode", ep.id);
                    Self::add_source_params(
                        &mut media_items,
                        ep.episode,
                        ep.poster,
                        &description,
                        path,
                    );
                }
            }
        }

        Ok(media_items.into_values().collect())
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if !params.len().is_multiple_of(2) {
            return Err(anyhow!("Wrong params size"));
        }

        let mut results = vec![];
        for chunk in params.chunks(2) {
            let description = &chunk[0];
            let path = &chunk[1];

            let Some(link) = Self::load_video_url(&format!("{API_PLAYER_URL}/{path}")).await else {
                continue;
            };

            let mut sources = if link.contains("ashdi.vip") {
                utils::playerjs::load_and_parse_playerjs_sources(
                    utils::create_client()
                        .get(&link)
                        .header("Referer", SITE_URL),
                    description,
                )
                .await
                .unwrap_or_default()
            } else if link.contains("moonanime.art") {
                moonanime::extract(&link, description)
                    .await
                    .unwrap_or_default()
            } else {
                continue;
            };

            results.append(&mut sources);
        }

        Ok(results)
    }
}

impl AnimeONContentSupplier {
    async fn get_json<T: DeserializeOwned>(url: &str) -> anyhow::Result<T> {
        Ok(utils::create_json_client()
            .get(url)
            .send()
            .await?
            .json()
            .await?)
    }

    async fn load_episodes(
        anime_id: &str,
        player_id: u32,
        translation_id: u32,
    ) -> anyhow::Result<Vec<models::Episode>> {
        let base_url = format!(
            "{API_PLAYER_URL}/{anime_id}/episodes?take={EPISODES_PAGE_SIZE}&playerId={player_id}&translationId={translation_id}"
        );

        let mut seen_ids = HashSet::new();
        let mut result = vec![];

        for include_alternative in [true, false] {
            // skip=-1 additionally returns episodes numbered <= 0 (specials)
            let mut skip: i64 = -1;
            loop {
                let url =
                    format!("{base_url}&skip={skip}&includeAlternative={include_alternative}");
                let episodes = Self::get_json::<models::EpisodesResponse>(&url)
                    .await?
                    .episodes;
                let count = episodes.len();

                result.extend(episodes.into_iter().filter(|ep| seen_ids.insert(ep.id)));

                if skip >= 0 && count < EPISODES_PAGE_SIZE {
                    break;
                }
                skip = if skip < 0 {
                    0
                } else {
                    skip + EPISODES_PAGE_SIZE as i64
                };
            }
        }

        Ok(result)
    }

    async fn load_video_url(url: &str) -> Option<String> {
        let response: models::VideoResponse = Self::get_json(url).await.ok()?;
        response
            .video_url
            .or(response.file_url)
            .filter(|link| !link.is_empty())
    }

    /// Appends `[description, player api path]` pair to episode params,
    /// resolved later in `load_media_item_sources`
    fn add_source_params(
        media_items: &mut BTreeMap<i32, ContentMediaItem>,
        episode: i32,
        poster: Option<String>,
        description: &str,
        path: String,
    ) {
        let item = media_items
            .entry(episode)
            .or_insert_with(|| ContentMediaItem {
                title: format!("Серія {episode}"),
                section: None,
                image: poster.filter(|p| !p.is_empty()),
                sources: None,
                params: vec![],
            });

        item.params.push(description.to_string());
        item.params.push(path);
    }

    fn parse_serach_response(response: models::SearchResponse) -> Vec<ContentInfo> {
        response
            .results
            .into_iter()
            .map(Self::parse_search_result_item)
            .collect()
    }

    fn parse_search_result_item(item: models::SearchResultItem) -> ContentInfo {
        ContentInfo {
            id: item.slug,
            title: item.title_ua,
            secondary_title: None,
            image: format!("{}/{}", API_IMAGE_URL, item.image.preview),
        }
    }

    fn parse_details_response(response: models::DetailsResponse) -> ContentDetails {
        let genres = response
            .genres
            .into_iter()
            .map(|g| g.name_ua)
            .collect::<Vec<_>>()
            .join(", ");

        let additional_info = vec![
            Some(format!("Жанри: {genres}")),
            response
                .studio
                .as_ref()
                .map(|s| format!("Студія: {}", s.name)),
            response
                .release_date
                .as_ref()
                .map(|d| format!("Дата релізу: {d}")),
            response.raiting.as_ref().map(|r| format!("Рейтинг: {r}")),
            response.status.as_ref().map(|s| format!("Статус: {s}")),
            response
                .mal_scored
                .as_ref()
                .map(|s| format!("Оцінка MAL: {s}")),
        ]
        .into_iter()
        .flatten()
        .collect();

        ContentDetails {
            // id: response.id.to_string(),
            title: response.title_ua,
            media_type: MediaType::Video,
            description: response
                .description
                .map(|s| utils::text::sanitize_text(&s))
                .unwrap_or_default(),
            original_title: response.title_original,
            additional_info,
            image: format!("{}/{}", API_IMAGE_URL, response.image.preview),
            params: vec![],
            media_items: None,
            similar: vec![],
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test_log::test(tokio::test)]
    async fn animeon_should_search() {
        let res = AnimeONContentSupplier::default()
            .search("one piece", 1)
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn animeon_should_load_channel() {
        let res = AnimeONContentSupplier::default()
            .load_channel("Популярні", 1)
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn animeon_should_get_content_details() {
        let res = AnimeONContentSupplier::default()
            .get_content_details("175-povsyakdennoshchi")
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn animeon_should_load_media_items() {
        let res = AnimeONContentSupplier::default()
            .load_media_items("7930-kagurabachi", vec![])
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn animeon_should_load_media_item_sources() {
        let res = AnimeONContentSupplier::default()
            .load_media_item_sources(
                "175-povsyakdennoshchi",
                vec![
                    "MelodicVoiceStudio (Ashdi)".into(),
                    "81218/episode".into(),
                    "MelodicVoiceStudio (Moon)".into(),
                    "113885/episode".into(),
                ],
            )
            .await;

        println!("{res:#?}");
    }
}
