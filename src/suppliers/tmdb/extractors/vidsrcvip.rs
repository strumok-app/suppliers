use std::{collections::HashMap, sync::OnceLock};

use base64::{prelude::BASE64_STANDARD, Engine};
use futures::future::BoxFuture;
use log::error;
use regex::Regex;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{create_json_client, lang},
};

use super::SourceParams;

const SITE_URL: &str = "https://vidsrc.vip";
const BACKEND_URL: &str = "https://api2.vidsrc.vip";
const HSL1_PROXY: &str = "https://hls2.vid1.site";
const MEGACDN_SERVER: &str = "f12.megacdn.co";

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
    struct ServerSource {
        url: Option<String>,
        language: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ServerSources {
        source1: Option<ServerSource>,
        source2: Option<ServerSource>,
        source3: Option<ServerSource>,
        source4: Option<ServerSource>,
        source5: Option<ServerSource>,
    }

    let res_str = create_json_client().get(link).send().await?.text().await?;

    let res: ServerSources = serde_json::from_str(&res_str)?;

    let sources = vec![
        res.source1,
        res.source2,
        res.source3,
        res.source4,
        res.source5,
    ];

    let result: Vec<_> = sources
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(idx, source)| {
            let num = idx + 1;
            let mut url = source.url?;
            let language = source.language.as_ref().map_or("unknown", |s| s.as_str());

            if url.starts_with(HSL1_PROXY) {
                url = unwrap_hls1_proxy(&url)?;
            }

            if lang::is_allowed(langs, language) {
                Some(ContentMediaItemSource::Video {
                    link: url,
                    description: format!("{num}. vidsrc ({language})"),
                    headers: Some(HashMap::from([
                        ("Referer".to_owned(), SITE_URL.to_owned()),
                        ("Origin".to_owned(), SITE_URL.to_owned()),
                    ])),
                })
            } else {
                None
            }
        })
        .collect();

    Ok(result)
}

fn unwrap_hls1_proxy(url_str: &str) -> Option<String> {
    let url = match url::Url::parse(url_str) {
        Ok(url) => url,
        Err(_) => {
            error!("[vidsrc.vip] failed to parse {HSL1_PROXY} url {url_str}");
            return None;
        }
    };

    static MEGACDN_RE: OnceLock<Regex> = OnceLock::new();
    let megacdn_re = MEGACDN_RE.get_or_init(|| Regex::new(r"f\d+\.megacdn\.co").unwrap());

    url.query_pairs()
        .find(|(name, _)| name == "url")
        .map(|(_, value)| megacdn_re.replace(&value, MEGACDN_SERVER).to_string())
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
    #[tokio::test]
    async fn should_extract_movies() {
        let res = extract(
            &SourceParams {
                id: 655,
                imdb_id: None,
                ep: None,
            },
            &["en".to_owned()],
        )
        .await;

        println!("{res:#?}")
    }

    #[tokio::test]
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
