mod models;

use anyhow::{Ok, anyhow};
use indexmap::IndexMap;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{self, playerjs::PlayerJSFile},
};

const API_URL: &str = "https://animeon.club/api/anime";
const API_IMAGE_URL: &str = "https://animeon.club/api/uploads/images";
const API_PLAYER_URL: &str = "https://animeon.club/api/player";

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

        // println!("response_str: {response_str}");
        if let Some(response_str) = maybe_response_str {
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
        let url = format!("{API_PLAYER_URL}/{id}");
        let response_str = utils::create_json_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let response: Vec<models::PlayerReponse> = serde_json::from_str(&response_str)?;

        let maybe_ashdi_player = response.into_iter().filter(|p| p.name == "Ashdi").next();
        if let Some(ashdi_player) = maybe_ashdi_player {
            let playerjs: Vec<PlayerJSFile> = serde_json::from_str(&ashdi_player.json)?;

            let media_items = utils::playerjs::convert_strategy_dub_season_ep(&playerjs);

            return Ok(media_items);
        }

        Ok(vec![])
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Err(anyhow!("not implemented"))
    }
}

impl AnimeONContentSupplier {
    fn parse_serach_response(response: models::SearchResponse) -> Vec<ContentInfo> {
        response
            .results
            .into_iter()
            .map(Self::parse_search_result_item)
            .collect()
    }

    fn parse_search_result_item(item: models::SearchResultItem) -> ContentInfo {
        ContentInfo {
            id: item.id.to_string(),
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
    async fn should_search() {
        let res = AnimeONContentSupplier::default()
            .search("one piece", 1)
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_channel() {
        let res = AnimeONContentSupplier::default()
            .load_channel("Популярні", 1)
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = AnimeONContentSupplier::default()
            .get_content_details("175")
            .await;

        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items() {
        let res = AnimeONContentSupplier::default()
            .load_media_items("175", vec![])
            .await;

        println!("{res:#?}");
    }
}
