use std::{collections::BTreeMap, sync::OnceLock};

use base64::{Engine, prelude::BASE64_STANDARD_NO_PAD};
use log::{error, warn};
use regex::Regex;
use reqwest::RequestBuilder;
use serde::Deserialize;

use crate::models::{ContentMediaItem, ContentMediaItemSource};

#[derive(Deserialize, Debug)]
pub struct PlayerJSFile {
    pub title: String,
    pub folder: Option<Vec<PlayerJSFile>>,
    pub poster: Option<String>,
    pub file: Option<String>,
    pub subtitle: Option<String>,
}

pub async fn load_and_parse_playerjs(
    request_builder: RequestBuilder,
    startegy: fn(&Vec<PlayerJSFile>) -> Vec<ContentMediaItem>,
) -> anyhow::Result<Vec<ContentMediaItem>> {
    let html = request_builder.send().await?.text().await?;

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
        let mut sources = vec![ContentMediaItemSource::Video {
            description: String::from("Default"),
            headers: None,
            link: String::from(file),
            hls_proxy: false,
        }];

        if let Some(subtitle) = extract_playerjs_subtitle(&html) {
            populate_subtitle(&mut sources, subtitle, "Default");
        }

        Ok(vec![ContentMediaItem {
            title: String::new(),
            section: None,
            image: None,
            sources: Some(sources),
            params: vec![],
        }])
    }
}

/// Extract flat sources structure from playerjs (no multiple episodes, sesons, dubs expected)
pub async fn load_and_parse_playerjs_sources(
    request_builder: RequestBuilder,
    description: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let html = request_builder.send().await?.text().await?;

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
    } else {
        // `file` value formats are detected by prefix:
        // - `[{...}]` - JSON playlist (handled above)
        // - `http...` - plain link
        // - anything else - obfuscated link: base64 of the reversed link
        //   (legacy tortuga.tw format, current one is handled by `extractors::tortuga`)
        if file.starts_with("http") {
            populate_video_sources(&mut result, description, file);
        } else {
            // padding is unreliable (can be `====`), so strip it and decode without padding
            let file = file.trim_end_matches("=");
            let decoded_b64 = BASE64_STANDARD_NO_PAD.decode(file)?;
            // decoded bytes are the link in reverse order
            let decoded_b64 = decoded_b64.into_iter().rev().collect();

            let decoded_file = String::from_utf8(decoded_b64)?;
            populate_video_sources(&mut result, description, &decoded_file);
        }

        // flat playlists keep subtitles in a separate top-level `subtitle` field
        if let Some(subtitle) = extract_playerjs_subtitle(&html) {
            populate_subtitle(&mut result, subtitle, description);
        }
    }
    Ok(result)
}

pub fn extract_playerjs_playlist(content: &str) -> Option<&str> {
    extract_playerjs_field(content, "file")
}

pub fn extract_playerjs_subtitle(content: &str) -> Option<&str> {
    extract_playerjs_field(content, "subtitle").filter(|s| !s.is_empty())
}

/// Extracts top-level PlayerJS config field value (`name:'...'` or `name:"..."`).
/// Quoted JSON keys (`"file":`) are not matched, so nested playlist fields are skipped.
fn extract_playerjs_field<'a>(content: &'a str, field: &str) -> Option<&'a str> {
    static PLAYER_JS_FIELD_REGEXP: OnceLock<Regex> = OnceLock::new();
    let re = PLAYER_JS_FIELD_REGEXP.get_or_init(|| {
        Regex::new(
            r#"\b(?<name>file|subtitle)\s*:\s*(?:'(?<sq>(?:[^'\\]|\\.)*)'|"(?<dq>(?:[^"\\]|\\.)*)")"#,
        )
        .unwrap()
    });

    re.captures_iter(content)
        .find(|c| &c["name"] == field)
        .and_then(|c| c.name("sq").or_else(|| c.name("dq")))
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

                hls_proxy: false,
            });
        }
    } else {
        sources.push(ContentMediaItemSource::Video {
            link: file.to_owned(),
            description: String::from(title.trim()),
            headers: None,
            hls_proxy: false,
        });
    }
}

/// PlayerJS subtitle list format: "[label]url,[label]url" or plain "url"
fn populate_subtitle(
    sources: &mut Vec<ContentMediaItemSource>,
    subtitle: &str,
    default_title: &str,
) {
    for entry in subtitle.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (label, link) = match entry.strip_prefix('[').and_then(|e| e.split_once(']')) {
            Some((label, link)) => (label, link),
            None => (default_title.trim(), entry),
        };

        sources.push(ContentMediaItemSource::Subtitle {
            link: link.to_owned(),
            description: label.to_owned(),
            headers: None,
        });
    }
}

pub fn default_season_episode_id(season: &str, episode: &str) -> u32 {
    super::text::extract_digits(season) * 10000 + super::text::extract_digits(episode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_extract_file_and_subtitle_on_same_line() {
        let html = r#"new Playerjs({id:"p",file:'https://a/index.m3u8',subtitle:"[English]https://a/en.vtt,[Italian]https://a/it.vtt"});"#;

        assert_eq!(
            extract_playerjs_playlist(html),
            Some("https://a/index.m3u8")
        );
        assert_eq!(
            extract_playerjs_subtitle(html),
            Some("[English]https://a/en.vtt,[Italian]https://a/it.vtt")
        );
    }

    #[test]
    fn should_skip_nested_json_fields_and_empty_subtitle() {
        let html = r#"file:'[{"title":"Ім\'я","file":"https://a/1.m3u8","subtitle":"https://a/1.vtt"}]',
            subtitle:"","#;

        assert_eq!(
            extract_playerjs_playlist(html),
            Some(r#"[{"title":"Ім\'я","file":"https://a/1.m3u8","subtitle":"https://a/1.vtt"}]"#)
        );
        assert_eq!(extract_playerjs_subtitle(html), None);
    }

    #[test]
    fn should_populate_subtitle_list() {
        let mut sources = vec![];
        populate_subtitle(
            &mut sources,
            "[English]https://a/en.vtt,[Italian]https://a/it.vtt",
            "Default",
        );
        populate_subtitle(&mut sources, "https://a/plain.vtt", " Dub ");

        let parsed: Vec<_> = sources
            .iter()
            .map(|s| match s {
                ContentMediaItemSource::Subtitle {
                    link, description, ..
                } => (description.as_str(), link.as_str()),
                _ => panic!("subtitle expected"),
            })
            .collect();

        assert_eq!(
            parsed,
            vec![
                ("English", "https://a/en.vtt"),
                ("Italian", "https://a/it.vtt"),
                ("Dub", "https://a/plain.vtt"),
            ]
        );
    }
}
