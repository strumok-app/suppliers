use std::{collections::BTreeMap, sync::OnceLock};

use anyhow::Ok;
use regex::Regex;
use serde::Deserialize;

use crate::models::{ContentMediaItem, ContentMediaItemSource};

use super::extract_digits;

#[derive(Deserialize, Debug)]
pub struct PlayerJSFile {
    title: String,
    folder: Option<Vec<PlayerJSFile>>,
    poster: Option<String>,
    file: Option<String>,
    subtitle: Option<String>,
}

pub async fn load_and_parse_playerjs(
    url: &String,
    startegy: fn(&Vec<PlayerJSFile>) -> Vec<ContentMediaItem>,
) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
    let html = super::create_client().get(url).send().await?.text().await?;

    let maybe_file = extract_playerjs_playlist(&html);

    if maybe_file.is_none() {
        println!("PlayerJS playlist not found");
        return Ok(vec![]);
    }

    let file = maybe_file.unwrap();

    if file.starts_with("[{") {
        let playerjs_file: Vec<PlayerJSFile> = serde_json::from_str(file)?;
        Ok(startegy(&playerjs_file))
    } else {
        Ok(vec![ContentMediaItem {
            number: 0,
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
    description: String,
    url: String,
) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
    let html = super::create_client().get(url).send().await?.text().await?;

    let maybe_file = extract_playerjs_playlist(&html);

    if maybe_file.is_none() {
        println!("PlayerJS playlist not found");
        return Ok(vec![]);
    }

    let file = maybe_file.unwrap();
    if file.starts_with("[{") {
        let playerjs_file: Vec<PlayerJSFile> = serde_json::from_str(file)?;
        let mut result: Vec<ContentMediaItemSource> = vec![];
        for file in playerjs_file {
            populate_sources(&mut result, &description, &file);
        }
        Ok(result)
    } else {
        Ok(vec![ContentMediaItemSource::Video {
            description,
            headers: None,
            link: String::from(file),
        }])
    }
}

pub fn extract_playerjs_playlist(content: &String) -> Option<&str> {
    static PLAYER_JS_FILE_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let re =
        PLAYER_JS_FILE_REGEXP.get_or_init(|| Regex::new(r#"file:\s?['"](?<file>.+)['"]"#).unwrap());

    re.captures(&content)
        .map(|c| c.name("file"))
        .flatten()
        .map(|m| m.as_str())
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

pub fn convert_strategy_dub(playerjs_playlist: &Vec<PlayerJSFile>) -> Vec<ContentMediaItem> {
    let mut media_items: Vec<ContentMediaItem> = vec![];

    for dub in playerjs_playlist {
        let mut sources: Vec<ContentMediaItemSource> = vec![];
        populate_sources(&mut sources, &dub.title, dub);

        media_items.push(ContentMediaItem {
            number: media_items.len() as u32,
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
    let number = media_items.len();

    let item = media_items.entry(id).or_insert_with(|| {
        let title = episode.title.trim();
        let section = season.title.trim();

        ContentMediaItem {
            title: String::from(title),
            section: Some(String::from(section)),
            number: number as u32,
            image: episode.poster.clone(),
            sources: Some(vec![]),
            params: vec![],
        }
    });

    let sources = item.sources.as_mut().unwrap();

    populate_sources(sources, &dub.title, src);
}

fn populate_sources(sources: &mut Vec<ContentMediaItemSource>, title: &String, src: &PlayerJSFile) {
    if let Some(file) = src.file.as_ref() {
        sources.push(ContentMediaItemSource::Video {
            link: file.clone(),
            description: String::from(title.trim()),
            headers: None,
        });
    }

    if let Some(subtitle) = src.subtitle.as_ref() {
        if !subtitle.is_empty() {
            populate_subtitle(sources, subtitle, title);
        }
    }
}

fn populate_subtitle(
    sources: &mut Vec<ContentMediaItemSource>,
    url: &String,
    default_title: &String,
) {
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
            link: url.clone(),
            description: String::from(default_title.trim()),
            headers: None,
        });
    }
}

fn default_season_episode_id(season: &String, episode: &String) -> u32 {
    extract_digits(season) * 10000 + extract_digits(episode)
}
