use core::str;
use std::{collections::BTreeMap, sync::OnceLock};

use anyhow::anyhow;
use indexmap::IndexMap;
use log::{error, warn};
use serde::Deserialize;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{self, html},
};

use super::ContentSupplier;

const URL: &str = "https://mangafire.to";

#[derive(Default)]
pub struct MangaFireContentSupplier;

impl ContentSupplier for MangaFireContentSupplier {
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
        vec!["en".into(), "ja".into()]
    }

    async fn search(&self, query: String) -> anyhow::Result<Vec<ContentInfo>> {
        utils::scrap_page(
            utils::create_client()
                .get(format!("{URL}/filter"))
                .query(&[("keyword", query)]),
            content_info_items_processor(),
        )
        .await
    }

    async fn load_channel(&self, channel: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = match get_channels_map().get(channel.as_str()) {
            Some(url) => format!("{url}?={page}"),
            None => return Err(anyhow!("unknown channel")),
        };

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: String,
        langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{URL}/manga/{id}");

        let maybe_details =
            utils::scrap_page(utils::create_client().get(url), content_details_processor()).await;

        let mut details = match maybe_details {
            Ok(d) => d,
            _ => {
                warn!("[mangafire] failed to fetch details for id: {id}");
                return Ok(None);
            }
        };

        details.params = langs;

        Ok(Some(details))
    }

    async fn load_media_items(
        &self,
        id: String,
        langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let actual_id = id
            .split_once(".")
            .map(|(_, actual_id)| actual_id)
            .ok_or_else(|| anyhow!("[mangafire] invalid id"))?;

        let client = utils::create_json_client();
        let mut media_items: BTreeMap<Key, ContentMediaItem> = BTreeMap::new();

        for lang in langs {
            let volumes = match load_volumes(&client, actual_id, &lang).await {
                Ok(items) => items,
                Err(err) => {
                    error!("[mangafire] fail to fetch chaptes for {lang} and {id}: {err}");
                    vec![]
                }
            };

            for volume in volumes {
                let num = media_items.len() as u32;
                let media_item =
                    media_items
                        .entry(volume.key)
                        .or_insert_with(|| ContentMediaItem {
                            number: num,
                            title: volume.title,
                            section: None,
                            image: None,
                            sources: None,
                            params: vec![],
                        });

                media_item.params.push(lang.clone());
                media_item.params.push(volume.id);
            }
        }

        Ok(media_items.into_values().collect())
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() % 2 != 0 {
            return Err(anyhow!("[mangafire] invalid params"));
        }

        let client = utils::create_json_client();
        let mut sources: Vec<_> = vec![];

        for chunk in params.chunks(2) {
            let lang = &chunk[0];
            let id = &chunk[1];

            match load_volume(&client, lang, id).await {
                Ok(source) => sources.push(source),
                Err(err) => {
                    println!("[mangafire] fail to load source for lang {lang} id: {id}: {err}")
                }
            }
        }

        Ok(sources)
    }
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    num: u16,
    sub_num: u8,
}

impl Key {
    fn from_str(s: &str) -> Self {
        s.split_once(".")
            .map(|(a, b)| Self {
                num: a.parse::<u16>().unwrap_or_default(),
                sub_num: b.parse::<u8>().unwrap_or_default(),
            })
            .unwrap_or_else(|| Self {
                num: s.parse::<u16>().unwrap_or_default(),
                sub_num: 0,
            })
    }
}

#[derive(Debug)]
struct Volume {
    id: String,
    title: String,
    key: Key,
}

async fn load_volumes(
    client: &reqwest::Client,
    id: &str,
    lang: &str,
) -> anyhow::Result<Vec<Volume>> {
    let url = format!("{URL}/ajax/read/{id}/volume/{lang}");

    #[derive(Deserialize, Debug)]
    struct ChaptersResult {
        html: String,
    }

    #[derive(Deserialize, Debug)]
    struct ChaptersRes {
        status: u16,
        result: ChaptersResult,
    }

    let res: ChaptersRes = client
        .get(url)
        .header("Referer", URL)
        .send()
        .await?
        .json()
        .await?;

    if res.status != 200 {
        return Err(anyhow!(
            "[mangafire] fail to fetch volume id: {id}, lang: {lang}"
        ));
    }

    let doc = scraper::Html::parse_fragment(&res.result.html);
    let root = doc.root_element();

    let volume_selector = scraper::Selector::parse("li a").unwrap();

    let volumes = root
        .select(&volume_selector)
        .filter_map(|el| {
            let num = el.attr("data-number")?;
            let id = el.attr("data-id")?;

            Some(Volume {
                id: id.to_owned(),
                key: Key::from_str(num),
                title: format!("Volume {num}"),
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Ok(volumes)
}

async fn load_volume(
    client: &reqwest::Client,
    lang: &str,
    id: &str,
) -> anyhow::Result<ContentMediaItemSource> {
    #[derive(Deserialize, Debug)]
    struct VolumeResult {
        images: Vec<Vec<serde_json::Value>>,
    }

    #[derive(Deserialize, Debug)]
    struct VolumeRes {
        status: u16,
        result: VolumeResult,
    }

    let res: VolumeRes = client
        .get(format!("{URL}/ajax/read/volume/{id}"))
        .send()
        .await?
        .json()
        .await?;

    if res.status != 200 {
        return Err(anyhow!(
            "[mangafire] fail to fetch volume id: {id}, lang: {lang}"
        ));
    }

    let pages: Vec<_> = res
        .result
        .images
        .into_iter()
        .filter_map(|i| i.first().and_then(|v| v.as_str()).map(|s| s.to_owned()))
        .collect();

    Ok(ContentMediaItemSource::Manga {
        description: lang.to_owned(),
        headers: None,
        page_numbers: pages.len() as u32,
        pages: Some(pages),
        params: vec![],
    })
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::ItemsProcessor::new(".original .unit", content_info_processor()))
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map(extract_id)
            .in_scope("a.poster")
            .unwrap_or_default()
            .into(),
        title: html::text_value(".info > a"),
        secondary_title: html::default_value(),
        image: html::attr_value(".poster img", "src"),
    }
    .into()
}

fn content_details_processor() -> &'static html::ContentDetailsProcessor {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ContentDetailsProcessor> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| html::ContentDetailsProcessor {
        media_type: MediaType::Manga,
        title: html::text_value(".manga-detail .info > h1"),
        original_title: html::default_value(),
        image: html::attr_value(".manga-detail .detail-bg > img", "src"),
        description: html::TextValue::new()
            .all_nodes()
            .map(|s| html::strip_html(&s))
            .in_scope("#synopsis .modal-content")
            .unwrap_or_default()
            .into(),
        additional_info: html::flatten(vec![
            html::TextValue::new()
                .map(|s| vec![s])
                .in_scope(".info > h6")
                .unwrap_or_default()
                .into(),
            html::items_processor(
                ".manga-detail .min-info span",
                html::TextValue::new()
                    .all_nodes()
                    .map(|s| html::sanitize_text(&s))
                    .into(),
            ),
            html::items_processor(
                ".manga-detail .sidebar .meta div",
                html::TextValue::new()
                    .all_nodes()
                    .map(|s| html::sanitize_text(&s))
                    .into(),
            ),
        ]),
        similar: html::items_processor(
            ".container .sidebar .side-manga .body .unit",
            html::ContentInfoProcessor {
                id: html::AttrValue::new("href").map(extract_id).into(),
                title: html::text_value(".info h6"),
                secondary_title: html::default_value(),
                image: html::AttrValue::new("src")
                    .map(|l| l.replace("@100", ""))
                    .in_scope(".poster img")
                    .unwrap_or_default()
                    .into(),
            }
            .into(),
        ),
        params: html::default_value(),
    })
}

fn extract_id(link: String) -> String {
    link.rsplit_once("/")
        .map(|(_, id)| String::from(id))
        .unwrap_or_default()
}

fn get_channels_map() -> &'static IndexMap<&'static str, String> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, String>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("New Release", format!("{URL}/newest")),
            ("Updated", format!("{URL}/updated")),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_search() {
        let res = MangaFireContentSupplier.search("onepunch".into()).await;
        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_load_channel() {
        let res = MangaFireContentSupplier
            .load_channel("Updated".into(), 2)
            .await;
        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_get_content_details() {
        let res = MangaFireContentSupplier
            .get_content_details("one-punch-mann.oo4".into(), vec!["en".into()])
            .await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = MangaFireContentSupplier
            .load_media_items(
                "one-punch-mann.oo4".into(),
                vec!["en".into(), "ja".into()],
                vec![],
            )
            .await;
        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_load_media_item_sources() {
        let res = MangaFireContentSupplier
            .load_media_item_sources(
                "one-punch-mann.oo4".into(),
                vec![],
                vec!["en".into(), "633".into(), "ja".into(), "126226".into()],
            )
            .await;
        println!("{res:#?}");
    }
}
