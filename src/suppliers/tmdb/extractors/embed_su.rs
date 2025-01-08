use std::sync::OnceLock;

use base64::{
    prelude::{BASE64_STANDARD, BASE64_STANDARD_NO_PAD},
    Engine,
};
use futures::future::BoxFuture;
use log::warn;
use regex::Regex;
use serde::Deserialize;

use crate::{models::ContentMediaItemSource, utils};

use super::SourceParams;

const URL: &str = "https://embed.su";

#[derive(Deserialize, Debug)]
struct Server {
    name: String,
    hash: String,
}

pub fn extract_boxed(
    params: &SourceParams,
) -> BoxFuture<anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb_id = params.id;
    let url = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{URL}/embed/tv/{tmdb_id}/{s}/{e}")
        }
        None => format!("{URL}/embed/movie/{tmdb_id}"),
    };

    let html = utils::create_client().get(url).send().await?.text().await?;

    static B64_CODE_RE: OnceLock<Regex> = OnceLock::new();
    let b64code = B64_CODE_RE
        .get_or_init(|| Regex::new(r#"atob\(`([a-zA-Z0-9=]+)`\)"#).unwrap())
        .captures(&html)
        .and_then(|m| m.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| anyhow::anyhow!("No base64 code found"))?;

    let v_config_bytes = BASE64_STANDARD.decode(b64code)?;

    #[derive(Deserialize, Debug)]
    struct VConfig {
        hash: String,
    }

    let config: VConfig = serde_json::from_slice(&v_config_bytes)?;
    let servers_loaders_itr = extract_server_hash(&config.hash)?
        .into_iter()
        .enumerate()
        .map(|(idx, s)| async move {
            match load_server(&s, idx + 1).await {
                Ok(s) => s,
                Err(err) => {
                    warn!("[embed_su] failed to extract server {s:#?}: {err}");
                    vec![]
                }
            }
        });

    let result = futures::future::join_all(servers_loaders_itr)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(result)
}

async fn load_server(server: &Server, idx: usize) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let hash = &server.hash;
    let name = &server.name;

    #[derive(Deserialize, Debug)]
    struct Subtitle {
        label: String,
        file: String,
    }

    #[derive(Deserialize, Debug)]
    struct SourceResponse {
        source: String,
        subtitles: Vec<Subtitle>,
    }

    let res: SourceResponse = utils::create_client()
        .get(format!("{URL}/api/e/{hash}"))
        .header("Referer", URL)
        .send()
        .await?
        .json()
        .await?;

    static STRIP_PROXY_RE: OnceLock<Regex> = OnceLock::new();
    let striped_url = STRIP_PROXY_RE
        .get_or_init(|| Regex::new(r#"[a-z\.0-9]+/api/proxy/[a-z0-9]+/"#).unwrap())
        .replace(&res.source, "");

    let mut result: Vec<ContentMediaItemSource> = vec![ContentMediaItemSource::Video {
        link: striped_url.into(),
        description: format!("Embed.su {idx}. {name}"),
        headers: None,
    }];

    for subtitle in res.subtitles {
        if !subtitle.file.is_empty() {
            let lang = subtitle.label;
            result.push(ContentMediaItemSource::Subtitle {
                link: subtitle.file,
                description: format!("[embed_su] {idx}. {lang}"),
                headers: None,
            });
        }
    }

    Ok(result)
}

fn extract_server_hash(hash: &str) -> anyhow::Result<Vec<Server>> {
    let bytes = BASE64_STANDARD_NO_PAD.decode(hash)?;
    let s = String::from_utf8(bytes)?;

    let s = s
        .split(".")
        .flat_map(|s| s.chars().rev())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();

    let bytes = BASE64_STANDARD_NO_PAD.decode(&s)?;
    let result: Vec<Server> = serde_json::from_slice(&bytes)?;

    Ok(result)
}

#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[tokio::test]
    async fn should_extract_tv() {
        let res = extract(&SourceParams {
            id: 253,
            imdb_id: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;

        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_extract_movie() {
        let res = extract(&SourceParams {
            id: 310131,
            imdb_id: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;

        println!("{res:#?}")
    }

    #[test]
    fn should_extract_server_hash() {
        let v_config_hash = "Tm5aRTVtYlZvNFEycDBWMUJCVFROTE0zZzFRbFZLT0ZsdWVHdGxRVUl5UW5jaWZWMC5XM3NpYm1GdFpTSTZJblpwY0dWeUlpd2lhR0Z6YUNJNkltWlJjRFZDU0hSa1l6Qk9MVw";

        let server_hashes = extract_server_hash(v_config_hash).unwrap();

        assert_eq!(1, server_hashes.len());
        assert_eq!(
            "fQp5BHtdc0N-cgdNfmZ8CjtWPAM3K3x5BUJ8YnxkeAB2Bw",
            server_hashes.first().unwrap().hash
        );
    }
}
