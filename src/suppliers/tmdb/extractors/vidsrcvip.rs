use anyhow::Ok;
use base64::{prelude::BASE64_STANDARD, Engine};
use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{create_json_client, lang},
};

use super::SourceParams;

const BACKEND_URL: &str = "https://api2.vidsrc.vip";

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

    // println!("{link}");

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

    // println!("{res_str}");

    let res: ServerSources = serde_json::from_str(&res_str)?;

    // println!("{res:#?}");

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
            let url = source.url?;
            let language = source.language.as_ref().map_or("unknown", |s| s.as_str());

            if lang::is_allowed(langs, language) {
                Some(ContentMediaItemSource::Video {
                    link: url,
                    description: format!("{num}. vidsrc ({language})"),
                    headers: None,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(result)
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
