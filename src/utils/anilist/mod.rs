mod models;

use anyhow::Ok;
use models::{Date, GetAnimeResponse, SearchMedia, SearchResponse};
use serde_json::json;

use crate::{
    models::{ContentDetails, ContentInfo, MediaType},
    utils,
};

const URL: &str = "https://graphql.anilist.co";

pub async fn search_anime(query: &str) -> anyhow::Result<Vec<ContentInfo>> {
    let gql = include_str!("./queries/search_anime.graphql");
    let variables = json!({"search": query, "page": 1, "per_page": 20,});

    let body = json!({"query": gql, "variables": variables,});

    // println!("{body:#?}");

    let result: SearchResponse = utils::create_json_client()
        .post(URL)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    // println!("{result:#?}");

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

    // println!("{body:#?}");

    let result: GetAnimeResponse = utils::create_json_client()
        .post(URL)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    // println!("{result:#?}");

    let details = result.data.and_then(|data| data.media).map(|media| {
        let title = media.title;

        let status = media.status;
        let start_date: String = media.start_date.into();
        let score = media.average_score;
        let genres = media.genres.join(",");

        let mut additional_info = vec![
            score.to_string(),
            format!("Status: {status}"),
            format!("Start date: {start_date}"),
            format!("Genres: {genres}"),
        ];

        if let Some(country) = media.country_of_origin {
            additional_info.push(format!("Country: {country}"));
        }

        ContentDetails {
            title: title.english.or(title.romaji).unwrap_or_default(),
            original_title: title.native,
            image: media.cover_image.extra_large,
            description: media.description,
            additional_info,
            similar: media
                .relations
                .edges
                .into_iter()
                .map(|m| m.node.into())
                .collect(),
            media_items: None,
            media_type: MediaType::Video,
            params: vec![],
        }
    });

    Ok(details)
}

impl From<Date> for String {
    fn from(value: Date) -> Self {
        let day = value.day;
        let month = value.month;
        let year = value.year;

        format!("{year}-{month}-{day}")
    }
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
            image: media.cover_image.large,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn should_search() {
        let res = search_anime("frieren").await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_get_by_id() {
        let res = get_anime("21").await;
        // let res = get_anime("154587").await;
        println!("{res:#?}")
    }
}
