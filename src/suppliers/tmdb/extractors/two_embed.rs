use std::sync::OnceLock;

use anyhow::Ok;
use futures::future::BoxFuture;
use log::info;
use regex::Regex;

use crate::{extractors::packer_hls, models::ContentMediaItemSource, utils};

use super::SourceParams;

const URL: &str = "https://www.2embed.cc";
const PLAYER_URL: &str = "https://uqloads.xyz";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = match &params.imdb_id {
        Some(id) => id,
        None => return Ok(vec![]),
    };

    let url = match &params.ep {
        Some(ep) => {
            let e = ep.e;
            let s = ep.s;
            format!("{URL}/embedtv/{id}&s={s}&e={e}")
        }
        None => format!("{URL}/embed/{id}"),
    };

    let res = utils::create_client()
        .get(&url)
        .header("Referer", &url)
        .send()
        .await?
        .text()
        .await?;

    // println!("{res}")

    static STREAM_WISH_ID_RE: OnceLock<Regex> = OnceLock::new();
    let maybe_id = STREAM_WISH_ID_RE
        .get_or_init(|| Regex::new(r"swish\?id=(?<id>[\w\d]+)").unwrap())
        .captures(&res)
        .and_then(|m| m.name("id"))
        .map(|m| m.as_str());

    let id = match maybe_id {
        Some(id) => id,
        None => {
            info!("[two_embed] No stream wish id found");
            return Ok(vec![]);
        }
    };

    let player_url = format!("{PLAYER_URL}/e/{id}");

    return packer_hls::extract(&player_url, &url, "Two Embed".to_string(), true).await;
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn two_embed_should_load_movie() {
        let res = extract(&SourceParams {
            id: 0,
            imdb_id: Some("tt0088247".to_string()),
            ep: None,
        })
        .await;
        println!("{res:#?}")
    }
}
