use anyhow::anyhow;
use reqwest::{
    ClientBuilder,
    header::{self, HeaderMap},
};
use serde::Deserialize;
use std::time::Duration;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils,
};

use super::{ContentSupplier, MangaPagesLoader};

const BASE_URL: &str = "https://mangafire.to";
const API_URL: &str = "https://mangafire.to/api";
const PAGE_LIMIT: u16 = 30;

pub struct MangaFireContentSupplier {
    api_client: reqwest::Client,
}

impl Default for MangaFireContentSupplier {
    fn default() -> Self {
        let mut headers = HeaderMap::default();
        headers.insert(header::ACCEPT, "application/json".parse().unwrap());
        headers.insert(header::REFERER, BASE_URL.parse().unwrap());

        Self {
            api_client: ClientBuilder::new()
                .connect_timeout(Duration::from_secs(5))
                .read_timeout(Duration::from_secs(15))
                .default_headers(headers)
                .user_agent(
                    "Mozilla/5.0 (X11; Linux x86_64; rv:138.0) Gecko/20100101 Firefox/138.0",
                )
                .build()
                .unwrap(),
        }
    }
}

impl ContentSupplier for MangaFireContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec!["Trending".to_string()]
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Manga]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["en".into()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let res: MangaFireSearchResponse = self
            .api_client
            .get(format!("{API_URL}/titles"))
            .query(&[
                ("keyword", query),
                ("content_rating[]", "safe"),
                ("content_rating[]", "suggestive"),
                ("order[relevance]", "desc"),
            ])
            .query(&[("page", page), ("limit", PAGE_LIMIT)])
            .send()
            .await?
            .json()
            .await?;

        Ok(res.into())
    }

    async fn load_channel(&self, _: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let res: MangaFireSearchResponse = self
            .api_client
            .get(format!("{API_URL}/top-titles"))
            .query(&[("days", "30")])
            .query(&[("page", page), ("limit", PAGE_LIMIT)])
            .send()
            .await?
            .json()
            .await?;

        Ok(res.into())
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        let res: MangaFireDetailsResponse = self
            .api_client
            .get(format!("{API_URL}/titles/{id}"))
            .send()
            .await?
            .json()
            .await?;

        Ok(res.data.map(|d| d.into()))
    }

    async fn load_media_items(
        &self,
        id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let mut all_items: Vec<ContentMediaItem> = Vec::new();
        let mut page = 1u16;
        let max_pages = 20u16;

        loop {
            let res: MangaFireChaptersResponse = self
                .api_client
                .get(format!("{API_URL}/titles/{id}/chapters"))
                .query(&[("language", "en"), ("sort", "number"), ("order", "asc")])
                .query(&[("page", page), ("limit", 100)])
                .send()
                .await?
                .json()
                .await?;

            for ch in res.items {
                let title = if ch.name.is_empty() {
                    format!("Chapter {}", ch.number)
                } else {
                    format!("Chapter {} - {}", ch.number, ch.name)
                };

                all_items.push(ContentMediaItem {
                    title,
                    section: None,
                    image: None,
                    sources: Some(vec![ContentMediaItemSource::Manga {
                        description: "EN".to_string(),
                        headers: None,
                        pages: None,
                        params: vec![ch.id.to_string()],
                    }]),
                    params: vec![],
                });
            }

            if !res.meta.has_next || page >= max_pages {
                break;
            }
            page += 1;
        }

        Ok(all_items)
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Err(anyhow!("Unimplemented"))
    }
}

#[derive(Debug, Deserialize)]
struct MangaFireSearchResponse {
    items: Vec<MangaFireItem>,
}

#[derive(Debug, Deserialize)]
struct MangaFireItem {
    hid: String,
    title: String,
    status: Option<String>,
    poster: MangaFirePoster,
}

#[derive(Debug, Deserialize)]
struct MangaFirePoster {
    small: Option<String>,
    medium: Option<String>,
    large: Option<String>,
}

// -- Detail response types --

#[derive(Debug, Deserialize)]
struct MangaFireDetailsResponse {
    data: Option<MangaFireDetailItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MangaFireDetailItem {
    title: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    status: Option<String>,
    poster: MangaFirePoster,
    year: Option<u32>,
    rating: Option<f64>,
    synopsis_html: Option<String>,
    alt_titles: Option<Vec<String>>,
    languages: Option<Vec<String>>,
    genres: Option<Vec<MangaFireTag>>,
    themes: Option<Vec<MangaFireTag>>,
    demographics: Option<Vec<MangaFireTag>>,
    authors: Option<Vec<MangaFireTag>>,
    artists: Option<Vec<MangaFireTag>>,
}

#[derive(Debug, Deserialize)]
struct MangaFireTag {
    title: String,
}

impl From<MangaFireDetailItem> for ContentDetails {
    fn from(item: MangaFireDetailItem) -> Self {
        let image = item
            .poster
            .large
            .or(item.poster.medium)
            .or(item.poster.small)
            .unwrap_or_default();

        let description = item
            .synopsis_html
            .as_deref()
            .map(utils::text::strip_html)
            .unwrap_or_default();

        let original_title = item.alt_titles.and_then(|v| v.into_iter().next());

        let mut additional_info: Vec<String> = Vec::new();

        if let Some(kind) = &item.kind {
            additional_info.push(format!("Type: {kind}"));
        }
        if let Some(status) = &item.status {
            additional_info.push(format!("Status: {status}"));
        }
        if let Some(year) = item.year {
            additional_info.push(format!("Year: {year}"));
        }
        if let Some(rating) = item.rating {
            additional_info.push(format!("Rating: {rating}"));
        }
        if let Some(authors) = &item.authors {
            let names: Vec<_> = authors.iter().map(|a| a.title.as_str()).collect();
            if !names.is_empty() {
                additional_info.push(format!("Author: {}", names.join(", ")));
            }
        }
        if let Some(artists) = &item.artists {
            let names: Vec<_> = artists.iter().map(|a| a.title.as_str()).collect();
            if !names.is_empty() {
                additional_info.push(format!("Artist: {}", names.join(", ")));
            }
        }

        let all_tags: Vec<String> = [&item.genres, &item.themes, &item.demographics]
            .into_iter()
            .filter_map(|opt| opt.as_ref())
            .flat_map(|tags| tags.iter().map(|t| t.title.clone()))
            .collect();
        if !all_tags.is_empty() {
            additional_info.push(format!("Genres: {}", all_tags.join(", ")));
        }

        if let Some(langs) = &item.languages {
            if !langs.is_empty() {
                additional_info.push(format!("Languages: {}", langs.join(", ")));
            }
        }

        let params = item.languages.unwrap_or_default();

        ContentDetails {
            title: item.title,
            original_title,
            image,
            description,
            media_type: MediaType::Manga,
            additional_info,
            similar: Vec::new(),
            media_items: None,
            params,
        }
    }
}

impl MangaPagesLoader for MangaFireContentSupplier {
    async fn load_pages(&self, _id: &str, params: Vec<String>) -> anyhow::Result<Vec<String>> {
        if params.is_empty() {
            return Err(anyhow!("Chapter id expected"));
        }

        let chapter_id = &params[0];

        let res: MangaFirePagesResponse = self
            .api_client
            .get(format!("{API_URL}/chapters/{chapter_id}"))
            .send()
            .await?
            .json()
            .await?;

        let pages = res.data.pages.into_iter().map(|p| p.url).collect();

        Ok(pages)
    }
}

// -- Chapter response types --

#[derive(Debug, Deserialize)]
struct MangaFireChaptersResponse {
    items: Vec<MangaFireChapter>,
    meta: MangaFirePaginationMeta,
}

#[derive(Debug, Deserialize)]
struct MangaFireChapter {
    id: u64,
    number: f64,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MangaFirePaginationMeta {
    has_next: bool,
}

// -- Pages response types --

#[derive(Debug, Deserialize)]
struct MangaFirePagesResponse {
    data: MangaFirePagesData,
}

#[derive(Debug, Deserialize)]
struct MangaFirePagesData {
    pages: Vec<MangaFirePage>,
}

#[derive(Debug, Deserialize)]
struct MangaFirePage {
    url: String,
}

impl From<MangaFireSearchResponse> for Vec<ContentInfo> {
    fn from(value: MangaFireSearchResponse) -> Self {
        value
            .items
            .into_iter()
            .map(|item| {
                let id = item.hid.clone();
                let image = item
                    .poster
                    .medium
                    .or(item.poster.large)
                    .or(item.poster.small)
                    .unwrap_or_default();

                ContentInfo {
                    id,
                    title: item.title,
                    secondary_title: item.status,
                    image,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mangafire_should_search() {
        let res = MangaFireContentSupplier::default()
            .search("one piece", 1)
            .await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn mangafire_should_load_channel() {
        let res = MangaFireContentSupplier::default()
            .load_channel("Trending", 1)
            .await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn mangafire_should_get_content_details() {
        let res = MangaFireContentSupplier::default()
            .get_content_details("mj46z")
            .await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn mangafire_should_load_media_items() {
        let res = MangaFireContentSupplier::default()
            .load_media_items("dkw", vec![])
            .await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn mangafire_should_load_pages() {
        let res = MangaFireContentSupplier::default()
            .load_pages("", vec!["9073444".to_string()])
            .await;
        println!("{res:#?}");
    }
}
