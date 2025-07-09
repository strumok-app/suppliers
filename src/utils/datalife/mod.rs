mod playlist_html;
mod tests;

use anyhow::anyhow;
use indexmap::IndexMap;
use std::collections::BTreeMap;

use reqwest::{self, RequestBuilder};
use serde::Deserialize;

use super::html::DOMProcessor;
use crate::models::ContentMediaItem;

pub fn search_request(url: &str, query: &str) -> RequestBuilder {
    let client = super::create_client();

    client.post(format!("{url}/index.php")).form(&[
        ("do", "search"),
        ("subaction", "search"),
        ("story", query),
        ("sortby", "date"),
        ("resorder", "desc"),
    ])
}

pub fn get_channel_url(
    channels_map: &IndexMap<&'static str, String>,
    channel: &str,
    page: u16,
) -> anyhow::Result<String> {
    match channels_map.get(channel) {
        Some(url) => {
            if url.ends_with("/page/") {
                Ok(format!("{url}{page}"))
            } else {
                Ok(url.into())
            }
        }
        _ => Err(anyhow!("unknown channel")),
    }
}

pub fn extract_id_from_url(url: &str, mut id: String) -> String {
    if id.len() <= (url.len() + 5) {
        return String::new();
    }

    id.drain((url.len() + 1)..(id.len() - 5)).collect()
}

pub fn format_id_from_url(url: &str, id: &str) -> String {
    format!("{url}/{id}.html")
}

pub async fn load_ajax_playlist(
    playlist_req: reqwest::RequestBuilder,
) -> anyhow::Result<Vec<ContentMediaItem>> {
    const ALLOWED_VIDEO_HOSTS: &[&str] = &["ashdi", "tortuga", "moonanime", "monstro"];

    #[derive(Deserialize, Debug)]
    struct AjaxPlaylistResponse {
        response: String,
    }

    let res_text = playlist_req
        .header("X-Requested-With", "XMLHttpRequest")
        .send()
        .await?
        .text()
        .await?;

    

    let res: AjaxPlaylistResponse = serde_json::from_str(&res_text)?;

    let html_fragment = scraper::Html::parse_fragment(&res.response);
    let root = &html_fragment.root_element();

    let playlist = playlist_html::AjaxPlaylistProcessor::new().process(root);

    let mut sorted_media_items: BTreeMap<u32, ContentMediaItem> = BTreeMap::new();

    for video in playlist.videos {
        if !ALLOWED_VIDEO_HOSTS
            .iter()
            .any(|&host| video.file.contains(host))
        {
            continue;
        }

        let media_item =
            sorted_media_items
                .entry(video.number)
                .or_insert_with(|| ContentMediaItem {
                    title: video.name,
                    section: None,
                    image: None,
                    sources: None,
                    params: vec![],
                });

        let description = playlist
            .lables
            .iter()
            .filter(|&l| video.id.starts_with(&l.id))
            .map(|l| l.label.to_owned())
            .collect::<Vec<String>>()
            .join(" ");

        media_item.params.push(description);

        let mut file = video.file;
        if file.starts_with("//") {
            file.insert_str(0, "https:");
        }
        media_item.params.push(file);
    }

    Ok(sorted_media_items.into_values().collect())
}
