use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use futures::future::BoxFuture;
use log::warn;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    models::ContentMediaItemSource,
    utils::{self, create_json_client, enc_dec_app::ENC_DEC_APP_URL},
};

use super::SourceParams;

const VIDFAST_URL: &str = "https://vidfast.vc";

// enc-dec.app request/response types for vidfast

#[derive(Debug, Serialize)]
struct DecRequest {
    text: String,
}

#[derive(Debug, Deserialize)]
struct EncStage1Result {
    stage1: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct EncStage2Result {
    servers: String,
    stream: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct Server {
    data: String,
}

#[derive(Debug, Deserialize)]
struct DecResponse<T> {
    result: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamResult {
    url: String,
    no_referrer: bool,
    tracks: Option<Vec<Track>>,
}

#[derive(Debug, Deserialize)]
struct Track {
    file: String,
    label: String,
}

// enc-dec.app API helpers

async fn vidfast_enc<T: serde::de::DeserializeOwned>(text: &str, stage: u8) -> anyhow::Result<T> {
    let url = format!("{ENC_DEC_APP_URL}/api/enc-vidfast");

    let res_str = create_json_client()
        .get(url)
        .query(&[("text", text), ("stage", &stage.to_string())])
        .send()
        .await?
        .text()
        .await?;

    let res: DecResponse<T> = serde_json::from_str(&res_str)
        .map_err(|err| anyhow!("[vidfast] enc stage {stage} failed: {err}, response: {res_str}"))?;

    Ok(res.result)
}

async fn vidfast_dec_servers(text: &str) -> anyhow::Result<Vec<Server>> {
    let url = format!("{ENC_DEC_APP_URL}/api/dec-vidfast");

    let res_str = create_json_client()
        .post(url)
        .json(&DecRequest {
            text: text.to_string(),
        })
        .send()
        .await?
        .text()
        .await?;

    let res: DecResponse<Vec<Server>> = serde_json::from_str(&res_str)?;

    Ok(res.result)
}

async fn vidfast_dec_stream(text: &str) -> anyhow::Result<StreamResult> {
    let url = format!("{ENC_DEC_APP_URL}/api/dec-vidfast");

    let res_str = create_json_client()
        .post(url)
        .json(&DecRequest {
            text: text.to_string(),
        })
        .send()
        .await?
        .text()
        .await?;

    let res: DecResponse<StreamResult> = serde_json::from_str(&res_str)?;

    Ok(res.result)
}

// Extractor

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb_id = params.id;

    // Fetch page content to extract encrypted text
    let page_url = match &params.ep {
        Some(ep) => format!("{VIDFAST_URL}/tv/{tmdb_id}/{}/{}/", ep.s, ep.e),
        None => format!("{VIDFAST_URL}/movie/{tmdb_id}/"),
    };

    let client = utils::create_client();
    let page_html = client
        .get(&page_url)
        .header("Referer", format!("{VIDFAST_URL}/"))
        .send()
        .await?
        .text()
        .await?;

    // Extract encrypted text from page
    let text = extract_text(&page_html)?;

    // Stage 1: get servers token URL and CSRF token
    let stage1: EncStage1Result = vidfast_enc(&text, 1).await?;

    let stage1_text = client
        .post(&stage1.stage1)
        .header("Referer", format!("{VIDFAST_URL}/"))
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-CSRF-Token", &stage1.token)
        .send()
        .await?
        .text()
        .await?;

    // Stage 2: get servers/stream/token URLs
    let enc_result: EncStage2Result = vidfast_enc(&stage1_text, 2).await?;

    let token = enc_result.token;
    let servers_url = enc_result.servers;
    let stream_url = enc_result.stream;

    // POST to servers URL with CSRF token
    let servers_encrypted = client
        .post(&servers_url)
        .header("Referer", format!("{VIDFAST_URL}/"))
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-CSRF-Token", &token)
        .send()
        .await?
        .text()
        .await?;

    // Decrypt servers list
    let servers = vidfast_dec_servers(&servers_encrypted).await?;

    // Process each server in parallel
    let server_futures = servers.into_iter().enumerate().map(|(idx, server)| {
        let stream_url = &stream_url;
        let token = &token;
        async move {
            match load_server_stream(client, stream_url, &server.data, token, idx).await {
                Ok(server_sources) => server_sources,
                Err(err) => {
                    warn!("[vidfast] server {} failed: {err}", idx + 1);
                    vec![]
                }
            }
        }
    });

    let sources = futures::future::join_all(server_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    Ok(sources)
}

fn extract_text(html: &str) -> anyhow::Result<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"\\"(?:en|token)\\":\\"([^"\\]+)\\""#).unwrap());

    let caps = re
        .captures(html)
        .ok_or_else(|| anyhow!("[vidfast] failed to extract text from page"))?;

    Ok(caps[1].to_string())
}

async fn load_server_stream(
    client: &reqwest::Client,
    stream_url: &str,
    data: &str,
    token: &str,
    idx: usize,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    // POST to stream/{data} to get encrypted stream
    let url = format!("{stream_url}/{data}");
    let stream_encrypted = client
        .post(&url)
        .header("Referer", format!("{VIDFAST_URL}/"))
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-CSRF-Token", token)
        .send()
        .await?
        .text()
        .await?;

    // Decrypt stream data
    let stream_data = vidfast_dec_stream(&stream_encrypted).await?;

    // println!("{stream_data:?}");

    let mut sources: Vec<ContentMediaItemSource> = vec![];

    let headers = if stream_data.no_referrer {
        None
    } else {
        Some(HashMap::from([
            ("Origin".to_string(), VIDFAST_URL.to_string()),
            ("Referer".to_string(), format!("{VIDFAST_URL}/")),
        ]))
    };

    sources.push(ContentMediaItemSource::Video {
        link: stream_data.url,
        description: format!("[VidFast] Server {}", idx + 1),
        headers,
        hls_proxy: false,
    });

    if let Some(tracks) = stream_data.tracks {
        for track in tracks {
            sources.push(ContentMediaItemSource::Subtitle {
                link: track.file,
                description: format!("[VidFast] {}", track.label),
                headers: None,
            });
        }
    }

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn vidfast_should_extract_tv() {
        let res = extract(&SourceParams {
            id: 1399,
            imdb_id: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn vidfast_should_extract_movie() {
        let res = extract(&SourceParams {
            id: 533535,
            imdb_id: None,
            ep: None,
        })
        .await;
        println!("{res:#?}")
    }
}
