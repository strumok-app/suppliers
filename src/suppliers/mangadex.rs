use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use indexmap::IndexMap;
use serde::Deserialize;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils,
};

use super::{ContentSupplier, MangaPagesLoader};

const API_URL: &str = "https://api.mangadex.org";
const COVERS_URL: &str = "https://uploads.mangadex.org/covers";
const CHANNEL_PAGE_SIZE: usize = 50;
const CHAPTERS_LIMIT: usize = 500;

#[derive(Default)]
pub struct MangaDexContentSupplier;

impl ContentSupplier for MangaDexContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Manga]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".into(), "en".into()]
    }

    async fn search(&self, query: String) -> anyhow::Result<Vec<ContentInfo>> {
        let res_json = utils::create_client()
            .get(format!("{API_URL}/manga"))
            .query(&[
                ("title", query.as_str()),
                ("includes[]", "cover_art"),
                ("hasAvailableChapters", "true"),
            ])
            .query(&[("limit", CHANNEL_PAGE_SIZE)])
            .send()
            .await?
            .text()
            .await?;

        let search_res: MangaDexSearchResponse = serde_json::from_str(&res_json)?;
        Ok(search_res.into())
    }

    async fn load_channel(&self, channel: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let query = match get_channels_map().get(&channel) {
            Some(query) => query,
            None => return Err(anyhow!("Unknow channel")),
        };

        let search_res: MangaDexSearchResponse = utils::create_client()
            .get(format!("{API_URL}/manga"))
            .query(query)
            .query(&[("offset", (page as usize) * CHANNEL_PAGE_SIZE)])
            .send()
            .await?
            .json()
            .await?;

        Ok(search_res.into())
    }

    async fn get_content_details(
        &self,
        id: String,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let res: MangaDexSingeItemResponse = utils::create_client()
            .get(format!("{API_URL}/manga/{id}"))
            .query(&[("includes[]", "cover_art"), ("includes[]", "author")])
            .send()
            .await?
            .json()
            .await?;

        Ok(res.data.and_then(|d| d.into()))
    }

    async fn load_media_items(
        &self,
        id: String,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let mut requests_left = 30usize;
        let mut last_offset = 0usize;
        let cleint = utils::create_client();
        let mut media_items: IndexMap<String, ContentMediaItem> = IndexMap::new();
        while requests_left > 0 {
            let res_str = cleint
                .get(format!("{API_URL}/manga/{id}/feed"))
                .query(&[
                    ("includes[]", "scanlation_group"),
                    ("order[volume]", "asc"),
                    ("order[chapter]", "asc"),
                    ("offset", last_offset.to_string().as_str()),
                    ("limit", CHAPTERS_LIMIT.to_string().as_str()),
                ])
                .send()
                .await?
                .text()
                .await?;

            let res: MangaDexChaptersResponse = serde_json::from_str(&res_str)?;

            res.data.iter().for_each(|item| {
                let id = &item.id;
                let attributes = &item.attributes;

                let chapter = lookup_chapter(attributes);

                let num = media_items.len() as u32;
                let media_item = media_items.entry(chapter.into()).or_insert_with(|| {
                    let volume = lookup_volume(attributes);

                    ContentMediaItem {
                        number: num,
                        title: chapter.into(),
                        section: Some(volume.into()),
                        image: None,
                        sources: Some(vec![]),
                        params: vec![],
                    }
                });

                let sources = media_item.sources.as_mut().unwrap();

                let page_numbers = lookup_page_nambers(attributes);
                let translation_lang = lookup_translation_lang(attributes);
                let scanlation_group = lookup_scanlation_group(&item.relationships);

                sources.push(ContentMediaItemSource::Manga {
                    description: format!("[{translation_lang}] {scanlation_group}"),
                    headers: None,
                    page_numbers,
                    pages: None,
                    params: vec![id.to_owned()],
                });
            });

            if res.limit + res.offset >= res.total {
                break;
            }

            last_offset += CHAPTERS_LIMIT;
            requests_left -= 1;
        }

        Ok(media_items.into_values().collect())
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Err(anyhow!("Unimplemented"))
    }
}

impl MangaPagesLoader for MangaDexContentSupplier {
    async fn load_pages(&self, _id: String, params: Vec<String>) -> anyhow::Result<Vec<String>> {
        if params.is_empty() {
            return Err(anyhow!("Chapter id expected"));
        }

        let chapter_id = &params[0];

        #[derive(Deserialize)]
        struct Chapter {
            hash: String,
            data: Vec<String>,
        }

        #[derive(Deserialize)]
        struct ChapterServerRes {
            #[serde(rename = "baseUrl")]
            base_url: String,
            chapter: Chapter,
        }

        let chapter_server_res_str = utils::create_client()
            .get(format!("{API_URL}/at-home/server/{chapter_id}"))
            .send()
            .await?
            .text()
            .await?;

        // println!("{chapter_server_res_str}");

        let chapter_server_res: ChapterServerRes = serde_json::from_str(&chapter_server_res_str)?;

        let base_url = chapter_server_res.base_url;
        let chapter_hash = chapter_server_res.chapter.hash;

        let pages = chapter_server_res
            .chapter
            .data
            .iter()
            .map(|file_hash| format!("{base_url}/data/{chapter_hash}/{file_hash}"))
            .collect();

        Ok(pages)
    }
}

#[derive(Deserialize)]
struct MangaDexSingeItemResponse {
    data: Option<MangaDexItem>,
}

#[derive(Deserialize)]
struct MangaDexSearchResponse {
    data: Vec<MangaDexItem>,
}

#[derive(Deserialize)]
struct MangaDexItem {
    id: String,
    attributes: HashMap<String, serde_json::Value>,
    relationships: Vec<MangaDexRelationship>,
}

#[derive(Deserialize)]
struct MangaDexRelationship {
    r#type: String,
    attributes: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct MangaDexChaptersResponse {
    limit: usize,
    offset: usize,
    total: usize,
    data: Vec<MangaDexItem>,
}

impl From<MangaDexSearchResponse> for Vec<ContentInfo> {
    fn from(value: MangaDexSearchResponse) -> Self {
        value
            .data
            .into_iter()
            .filter_map(move |item| {
                let id = item.id;
                let title = lookup_title(&item.attributes)?;
                let file_name = lookup_cover_file_name(&item.relationships)?;
                let image = format!("{COVERS_URL}/{id}/{file_name}.512.jpg");

                Some(ContentInfo {
                    id,
                    title,
                    image,
                    secondary_title: None,
                })
            })
            .collect()
    }
}

impl From<MangaDexItem> for Option<ContentDetails> {
    fn from(value: MangaDexItem) -> Self {
        let id = value.id;
        let attributes = &value.attributes;
        let relationships = &value.relationships;

        let title = lookup_title(attributes)?;
        let original_title = lookup_original_title(attributes);
        let file_name = lookup_cover_file_name(relationships)?;
        let image = format!("{COVERS_URL}/{id}/{file_name}.512.jpg");
        let description = lookup_description(attributes)?;
        let additional_info = lookup_additional_info(attributes, relationships);

        Some(ContentDetails {
            title,
            original_title,
            image,
            description,
            media_type: MediaType::Manga,
            additional_info,
            similar: Vec::default(),
            params: Vec::default(),
            media_items: None,
        })
    }
}

fn lookup_title(attributes: &HashMap<String, serde_json::Value>) -> Option<String> {
    attributes
        .get("title")?
        .as_object()?
        .get("en")
        .as_ref()
        .and_then(|v| v.as_str())
        .map(|v| v.into())
}

fn lookup_description(attributes: &HashMap<String, serde_json::Value>) -> Option<String> {
    attributes
        .get("description")?
        .as_object()?
        .get("en")
        .as_ref()
        .and_then(|v| v.as_str())
        .map(|v| v.into())
}

fn lookup_original_title(attributes: &HashMap<String, serde_json::Value>) -> Option<String> {
    let original_lang = attributes.get("originalLanguage")?.as_str()?;
    let alt_titles = attributes.get("altTitles")?.as_array()?;

    alt_titles
        .iter()
        .filter_map(|t| t.get(original_lang)?.as_str())
        .map(String::from)
        .next()
}

fn lookup_cover_file_name(rels: &[MangaDexRelationship]) -> Option<&str> {
    rels.iter()
        .filter(|r| r.r#type == "cover_art")
        .filter_map(|r| {
            r.attributes
                .as_ref()
                .and_then(|a| a.get("fileName")?.as_str())
        })
        .next()
}

fn lookup_additional_info(
    attributes: &HashMap<String, serde_json::Value>,
    rels: &[MangaDexRelationship],
) -> Vec<String> {
    let author = lookup_author(rels).map(|v| format!("Author: {v}"));

    let year = attributes.get("year").and_then(|v| {
        let v_num = v.as_u64()?;
        Some(format!("Year: {v_num}"))
    });

    let status = attributes.get("status").and_then(|v| {
        let v_str = v.as_str()?;
        Some(format!("Status: {v_str}"))
    });

    let genres = lookup_genres(attributes).map(|v| format!("Genres: {v}"));

    vec![author, year, status, genres]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
}

fn lookup_genres(attributes: &HashMap<String, serde_json::Value>) -> Option<String> {
    let tags = attributes.get("tags")?.as_array()?;

    let genres = tags
        .iter()
        .filter_map(|tag| {
            let attr = tag.get("attributes")?.as_object()?;
            let group = attr.get("group")?;

            if group != "genre" {
                None
            } else {
                attr.get("name")?.get("en")?.as_str()
            }
        })
        .collect::<Vec<_>>();

    if genres.is_empty() {
        None
    } else {
        Some(genres.join(", "))
    }
}

fn lookup_author(rels: &[MangaDexRelationship]) -> Option<&str> {
    rels.iter()
        .filter(|r| r.r#type == "author")
        .filter_map(|r| r.attributes.as_ref().and_then(|a| a.get("name")?.as_str()))
        .next()
}

fn lookup_chapter(attributes: &HashMap<String, serde_json::Value>) -> &str {
    attributes
        .get("chapter")
        .unwrap()
        .as_str()
        .unwrap_or("Oneshot")
}

fn lookup_volume(attributes: &HashMap<String, serde_json::Value>) -> &str {
    attributes
        .get("volume")
        .unwrap()
        .as_str()
        .unwrap_or("No Volume")
}

fn lookup_page_nambers(attributes: &HashMap<String, serde_json::Value>) -> u32 {
    attributes
        .get("pages")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
}

fn lookup_translation_lang(attributes: &HashMap<String, serde_json::Value>) -> &str {
    attributes
        .get("translatedLanguage")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
}

fn lookup_scanlation_group(rels: &[MangaDexRelationship]) -> &str {
    rels.iter()
        .filter(|&r| r.r#type == "scanlation_group")
        .filter_map(|r| r.attributes.as_ref().and_then(|a| a.get("name")?.as_str()))
        .next()
        .unwrap_or("Unknown")
}

fn get_channels_map() -> &'static IndexMap<String, Vec<(&'static str, &'static str)>> {
    static CHANNELS_MAP: OnceLock<IndexMap<String, Vec<(&str, &str)>>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            (
                "Latest Updates".into(),
                vec![
                    ("order[createdAt]", "desc"),
                    ("includes[]", "cover_art"),
                    ("hasAvailableChapters", "true"),
                ],
            ),
            (
                "Popular Titles".into(),
                vec![
                    ("order[followedCount]", "desc"),
                    ("includes[]", "cover_art"),
                    ("hasAvailableChapters", "true"),
                ],
            ),
        ])
    })
}

#[cfg(test)]
mod tests {

    use std::vec;

    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = MangaDexContentSupplier
            .load_channel("Popular Titles".into(), 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = MangaDexContentSupplier
            .search("Dr Stone".into())
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_get_content_details() {
        let res = MangaDexContentSupplier
            .get_content_details("cfc3d743-bd89-48e2-991f-63e680cc4edf".into(), vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = MangaDexContentSupplier
            .load_media_items(
                "c1e284bc-0436-42fe-b571-fa35a94279ce".into(),
                vec![],
                vec![],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test()]
    async fn should_load_pages() {
        let res = MangaDexContentSupplier
            .load_pages(
                "c1e284bc-0436-42fe-b571-fa35a94279ce".into(),
                vec!["1fe13d15-982f-402b-8120-91f717f886b8".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}")
    }
}
