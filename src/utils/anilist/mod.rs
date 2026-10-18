pub mod filter;
mod models;

pub use filter::BrowseFilter;
pub use models::{AnimeEpisodes, StreamingEpisode};

use anyhow::Ok;
use models::{Date, GetAnimeEpisodesResponse, GetAnimeResponse, SearchMedia, SearchResponse};
use serde_json::json;

use crate::{
    models::{ContentDetails, ContentInfo, MediaType},
    utils,
};

const URL: &str = "https://graphql.anilist.co";

pub async fn search_anime(query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
    let gql = include_str!("./queries/search_anime.graphql");
    let variables = json!({"search": query, "page": page, "per_page": 20,});

    let body = json!({"query": gql, "variables": variables,});

    let result: SearchResponse = utils::create_json_client()
        .post(URL)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    let content_info: Vec<_> = result
        .data
        .page
        .media
        .into_iter()
        .map(|media| media.into())
        .collect();

    Ok(content_info)
}

pub async fn browse_anime(filter: &BrowseFilter, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
    let gql = include_str!("./queries/browse_anime.graphql");
    let mut variables = serde_json::to_value(filter)?;
    variables["page"] = json!(page);
    variables["per_page"] = json!(20);

    let body = json!({"query": gql, "variables": variables,});

    let result: SearchResponse = utils::create_json_client()
        .post(URL)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    let content_info: Vec<_> = result
        .data
        .page
        .media
        .into_iter()
        .map(|media| media.into())
        .collect();

    Ok(content_info)
}

pub async fn get_anime(id: &str) -> anyhow::Result<Option<ContentDetails>> {
    let gql = include_str!("./queries/get_anime.graphql");
    let variables = json!({"id": id,});

    let body = json!({"query": gql, "variables": variables,});

    let result: GetAnimeResponse = utils::create_json_client()
        .post(URL)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    let details = result.data.and_then(|data| data.media).map(|media| {
        let title = media.title;

        let mut additional_info = vec![];

        if let Some(score) = media.average_score {
            additional_info.push(format!("Score: {score}"));
        }

        if let Some(status) = media.status {
            additional_info.push(format!("Status: {status}"));
        }

        if let Some(start_date) = media.start_date.as_ref().and_then(format_date) {
            additional_info.push(format!("Start date: {start_date}"));
        }

        if let Some(genres) = media.genres.filter(|genres| !genres.is_empty()) {
            additional_info.push(format!("Genres: {}", genres.join(", ")));
        }

        if let Some(country) = media.country_of_origin {
            additional_info.push(format!("Country: {country}"));
        }

        ContentDetails {
            title: title.english.or(title.romaji).unwrap_or_default(),
            original_title: title.native,
            image: media
                .cover_image
                .and_then(|cover| cover.extra_large.or(cover.large))
                .unwrap_or_default(),
            description: utils::text::strip_html(&media.description.unwrap_or_default()),
            additional_info,
            similar: vec![],
            media_items: None,
            media_type: MediaType::Video,
            params: vec![],
        }
    });

    Ok(details)
}

pub async fn get_anime_episodes(id: &str) -> anyhow::Result<Option<AnimeEpisodes>> {
    let id: u32 = id.parse()?;
    let gql = include_str!("./queries/get_anime_episodes.graphql");
    let variables = json!({"id": id,});

    let body = json!({"query": gql, "variables": variables,});

    let result: GetAnimeEpisodesResponse = utils::create_json_client()
        .post(URL)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    Ok(result.data.and_then(|data| data.media))
}

/// Formats as much of the date as is known: `2027`, `2027-04` or `2027-04-10`.
/// A month/day without a year is meaningless, so it yields `None`.
fn format_date(date: &Date) -> Option<String> {
    let year = date.year?;

    let formatted = match (date.month, date.day) {
        (Some(month), Some(day)) => format!("{year}-{month:02}-{day:02}"),
        (Some(month), None) => format!("{year}-{month:02}"),
        _ => year.to_string(),
    };

    Some(formatted)
}

impl From<SearchMedia> for ContentInfo {
    fn from(media: SearchMedia) -> Self {
        Self {
            id: media.id.to_string(),
            title: media
                .title
                .english
                .or(media.title.romaji)
                .unwrap_or_default(),
            secondary_title: media.title.native,
            image: media
                .cover_image
                .and_then(|cover| cover.large.or(cover.extra_large))
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn should_search() {
        let res = search_anime("frieren", 1).await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_browse() {
        let res = browse_anime(&BrowseFilter::popular_this_season(), 1).await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_get_episodes_by_id() {
        let res = get_anime_episodes("21").await;
        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_get_by_id() {
        let res = get_anime("21").await;
        // let res = get_anime("154587").await;
        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_get_upcoming_by_id() {
        // Not yet released: no score and no start date at the time of writing.
        let res = get_anime("186712").await.unwrap();
        println!("{res:#?}");
        assert!(res.is_some());
    }
}
