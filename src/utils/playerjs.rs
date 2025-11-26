use std::{collections::BTreeMap, sync::OnceLock};

use base64::{Engine, prelude::BASE64_STANDARD_NO_PAD};
use log::{error, warn};
use regex::Regex;
use serde::Deserialize;

use crate::models::{ContentMediaItem, ContentMediaItemSource};

#[derive(Deserialize, Debug)]
pub struct PlayerJSFile {
    title: String,
    folder: Option<Vec<PlayerJSFile>>,
    poster: Option<String>,
    file: Option<String>,
    subtitle: Option<String>,
}

pub async fn load_and_parse_playerjs(
    url: &str,
    startegy: fn(&Vec<PlayerJSFile>) -> Vec<ContentMediaItem>,
) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
    let html = super::create_client().get(url).send().await?.text().await?;

    let maybe_file = extract_playerjs_playlist(&html);

    if maybe_file.is_none() {
        error!("PlayerJS playlist not found");
        return Ok(vec![]);
    }

    let file = maybe_file.unwrap();

    if file.starts_with("[{") {
        let playerjs_file: Vec<PlayerJSFile> = serde_json::from_str(file)?;
        Ok(startegy(&playerjs_file))
    } else {
        Ok(vec![ContentMediaItem {
            title: String::new(),
            section: None,
            image: None,
            sources: Some(vec![ContentMediaItemSource::Video {
                description: String::from("Default"),
                headers: None,
                link: String::from(file),
            }]),
            params: vec![],
        }])
    }
}

/// Extract flat sources structure from playerjs (no multiple episodes, sesons, dubs expected)
pub async fn load_and_parse_playerjs_sources(
    description: &str,
    url: &str,
) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
    let html = super::create_client().get(url).send().await?.text().await?;

    let maybe_file = extract_playerjs_playlist(&html);

    if maybe_file.is_none() {
        warn!("PlayerJS playlist not found");
        return Ok(vec![]);
    }

    let mut result: Vec<ContentMediaItemSource> = vec![];
    let file = maybe_file.unwrap();
    if file.starts_with("[{") {
        let playerjs_file: Vec<PlayerJSFile> = serde_json::from_str(file)?;
        for file in playerjs_file {
            populate_sources(&mut result, description, &file);
        }
    } else if file.starts_with("http") {
        populate_video_sources(&mut result, description, file);
    } else {
        let file = file.trim_end_matches("=");
        let decoded_b64 = BASE64_STANDARD_NO_PAD.decode(file)?;
        let decoded_b64 = decoded_b64.into_iter().rev().collect();

        let decoded_file = String::from_utf8(decoded_b64)?;
        populate_video_sources(&mut result, description, &decoded_file);
    }
    Ok(result)
}

pub fn extract_playerjs_playlist(content: &str) -> Option<&str> {
    static PLAYER_JS_FILE_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let re =
        PLAYER_JS_FILE_REGEXP.get_or_init(|| Regex::new(r#"file:\s?['"](?<file>.+)['"]"#).unwrap());

    re.captures(content)
        .and_then(|c| c.name("file"))
        .map(|m| m.as_str())
}

pub fn convert_strategy_season_dub_ep(
    playerjs_playlist: &Vec<PlayerJSFile>,
) -> Vec<ContentMediaItem> {
    let mut sorted_media_items: BTreeMap<u32, ContentMediaItem> = BTreeMap::new();

    for season in playerjs_playlist {
        for dub in season.folder.iter().flatten() {
            for episode in dub.folder.iter().flatten() {
                populate_media_items_map(season, episode, dub, episode, &mut sorted_media_items);
            }
        }
    }

    sorted_media_items.into_values().collect()
}

pub fn convert_strategy_season_ep_dub(
    playerjs_playlist: &Vec<PlayerJSFile>,
) -> Vec<ContentMediaItem> {
    let mut sorted_media_items: BTreeMap<u32, ContentMediaItem> = BTreeMap::new();

    for season in playerjs_playlist {
        for episode in season.folder.iter().flatten() {
            for dub in episode.folder.iter().flatten() {
                populate_media_items_map(season, episode, dub, dub, &mut sorted_media_items);
            }
        }
    }

    sorted_media_items.into_values().collect()
}

pub fn convert_strategy_dub_season_ep(
    playerjs_playlist: &Vec<PlayerJSFile>,
) -> Vec<ContentMediaItem> {
    let mut sorted_media_items: BTreeMap<u32, ContentMediaItem> = BTreeMap::new();

    for dub in playerjs_playlist {
        for season in dub.folder.iter().flatten() {
            for episode in season.folder.iter().flatten() {
                populate_media_items_map(season, episode, dub, episode, &mut sorted_media_items);
            }
        }
    }

    sorted_media_items.into_values().collect()
}

pub fn convert_strategy_dub(playerjs_playlist: &Vec<PlayerJSFile>) -> Vec<ContentMediaItem> {
    let mut media_items: Vec<ContentMediaItem> = vec![];

    for dub in playerjs_playlist {
        let mut sources: Vec<ContentMediaItemSource> = vec![];
        populate_sources(&mut sources, &dub.title, dub);

        media_items.push(ContentMediaItem {
            title: String::new(),
            section: None,
            image: None,
            params: vec![],
            sources: Some(sources),
        });
    }

    media_items
}

fn populate_media_items_map(
    season: &PlayerJSFile,
    episode: &PlayerJSFile,
    dub: &PlayerJSFile,
    src: &PlayerJSFile,
    media_items: &mut BTreeMap<u32, ContentMediaItem>,
) {
    let id = default_season_episode_id(&season.title, &episode.title);

    let item = media_items.entry(id).or_insert_with(|| {
        let title = episode.title.trim();
        let section = season.title.trim();

        ContentMediaItem {
            title: String::from(title),
            section: Some(String::from(section)),
            image: episode.poster.clone(),
            sources: Some(vec![]),
            params: vec![],
        }
    });

    let sources = item.sources.as_mut().unwrap();

    populate_sources(sources, &dub.title, src);
}

fn populate_sources(sources: &mut Vec<ContentMediaItemSource>, title: &str, src: &PlayerJSFile) {
    if let Some(file) = src.file.as_ref() {
        populate_video_sources(sources, title, file)
    }

    if let Some(subtitle) = src.subtitle.as_ref()
        && !subtitle.is_empty()
    {
        populate_subtitle(sources, subtitle, title);
    }
}

fn populate_video_sources(sources: &mut Vec<ContentMediaItemSource>, title: &str, file: &str) {
    if file.starts_with("[") {
        for quality_and_link in file.split(",") {
            let quality_ends = quality_and_link.find("]").unwrap_or(0) + 1;
            let quality = &quality_and_link[0..quality_ends];
            let link = &quality_and_link[quality_ends..];

            sources.push(ContentMediaItemSource::Video {
                link: link.to_owned(),
                description: format!("{quality}{title}"),
                headers: None,
            });
        }
    } else {
        sources.push(ContentMediaItemSource::Video {
            link: file.to_owned(),
            description: String::from(title.trim()),
            headers: None,
        });
    }
}

fn populate_subtitle(sources: &mut Vec<ContentMediaItemSource>, url: &str, default_title: &str) {
    static PLAYER_JS_SUBTITLE_REGEXP: OnceLock<regex::Regex> = OnceLock::new();

    if url.starts_with("[") {
        let re = PLAYER_JS_SUBTITLE_REGEXP
            .get_or_init(|| Regex::new(r#"^\[(?<label>[^\]]+)\](?<url>.*)"#).unwrap());

        if let Some(captures) = re.captures(url) {
            let label = captures.name("label").unwrap().as_str();
            let url = captures.name("url").unwrap().as_str();

            sources.push(ContentMediaItemSource::Subtitle {
                link: String::from(url),
                description: String::from(label),
                headers: None,
            });
        }
    } else {
        sources.push(ContentMediaItemSource::Subtitle {
            link: url.into(),
            description: String::from(default_title.trim()),
            headers: None,
        });
    }
}

fn default_season_episode_id(season: &str, episode: &str) -> u32 {
    super::text::extract_digits(season) * 10000 + super::text::extract_digits(episode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn shoudl_extact_tortuga() {
        let res = load_and_parse_playerjs_sources(
            "ОЗВУЧЕННЯ FANVOXUA ПЛЕЄР TRG",
            "https://tortuga.tw/vod/119859",
        )
        .await;

        println!("{res:?}");
    }
}
