use std::collections::HashMap;

use anyhow::anyhow;
use base64::{Engine, prelude::BASE64_STANDARD};
use futures::future::BoxFuture;
use log::warn;
use reqwest::Client;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    suppliers::tmdb::URL,
    utils::{create_json_client, lang},
};

use super::SourceParams;

const SITE_URL: &str = "https://vidrock.net";
const BACKEND_URL: &str = "https://vidrock.net/api";

#[derive(Debug, Deserialize)]
struct ServerSource {
    url: Option<String>,
    language: Option<String>,
}

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params, langs))
}

pub async fn extract(
    params: &SourceParams,
    langs: &[String],
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = params.id;

    let link = match &params.ep {
        Some(ep) => {
            let hash = calc_tv_show_hash(id, ep.s, ep.e);
            format!("{BACKEND_URL}/tv/{hash}")
        }
        None => {
            let hash = calc_movie_hash(id);
            format!("{BACKEND_URL}/movie/{hash}")
        }
    };

    #[derive(Debug, Deserialize)]
    struct ServerSources {
        source1: Option<ServerSource>,
        source2: Option<ServerSource>,
        source3: Option<ServerSource>,
        source4: Option<ServerSource>,
        source5: Option<ServerSource>,
    }

    let client = create_json_client();
    let res_str = client.get(link).send().await?.text().await?;
    // println!("{res_str}");

    let res: ServerSources = serde_json::from_str(&res_str)?;

    let mut result: Vec<ContentMediaItemSource> = vec![];
    if let Some(source) = res.source1 {
        match load_vidstor_playlist(client, source).await {
            Ok(mut vidstore) => result.append(&mut vidstore),
            Err(e) => warn!("[vidrocks] fail to load source: {e}"),
        }
    }

    let sources = vec![res.source2, res.source3, res.source4, res.source5];

    sources
        .into_iter()
        .flatten()
        .enumerate()
        .for_each(|(idx, source)| {
            let num = idx + 2;
            let url = match source.url {
                Some(url) => url,
                None => return,
            };
            let language = source.language.as_ref().map_or("unknown", |s| s.as_str());

            if lang::is_allowed(langs, language) {
                result.push(ContentMediaItemSource::Video {
                    link: url,
                    description: format!("[Vidrocks] {num}. {language}"),
                    headers: Some(HashMap::from([
                        ("Referer".to_owned(), SITE_URL.to_owned()),
                        ("Origin".to_owned(), SITE_URL.to_owned()),
                    ])),
                })
            }
        });

    Ok(result)
}

async fn load_vidstor_playlist(
    client: &Client,
    server_source: ServerSource,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let url = server_source.url.ok_or_else(|| anyhow!("url == null"))?;

    #[derive(Deserialize, Debug)]
    struct PlaylistItem {
        resolution: u16,
        url: String,
    }

    let res_str = client
        .get(&url)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;

    // println!("{res_str}");

    let playlist: Vec<PlaylistItem> = serde_json::from_str(&res_str)?;

    // println!("{playlist:?}");

    let items: Vec<_> = playlist
        .into_iter()
        .rev()
        .map(|item| ContentMediaItemSource::Video {
            link: item.url,
            description: format!(
                "[Vidrocks] 1. {} - {}",
                server_source
                    .language
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or("vidstore".to_string()),
                item.resolution
            ),
            headers: Some(HashMap::from([("Referer".to_owned(), SITE_URL.to_owned())])),
        })
        .collect();

    Ok(items)
}

fn calc_movie_hash(id: u32) -> String {
    const ENCODING: [u8; 10] = [b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j'];

    let a: Vec<u8> = id
        .to_string()
        .chars()
        .map(|ch| -> u8 {
            let idx = ch.to_digit(10).unwrap() as usize;
            ENCODING[idx]
        })
        .rev()
        .collect();

    let b = BASE64_STANDARD.encode(a);

    BASE64_STANDARD.encode(&b)
}

fn calc_tv_show_hash(id: u32, s: u32, e: u32) -> String {
    let a = format!("{id}-{s}-{e}");
    let b: Vec<u8> = a.bytes().rev().collect();
    let c = BASE64_STANDARD.encode(&b);

    BASE64_STANDARD.encode(&c)
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;
    #[test_log::test(tokio::test)]
    async fn should_extract_movies() {
        let res = extract(
            &SourceParams {
                id: 533535,
                imdb_id: None,
                ep: None,
            },
            &["en".to_owned()],
        )
        .await;

        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_extract_tv() {
        let res = extract(
            &SourceParams {
                id: 655,
                imdb_id: None,
                ep: Some(Episode { e: 1, s: 1 }),
            },
            &["en".to_owned()],
        )
        .await;

        println!("{res:#?}")
    }
}
