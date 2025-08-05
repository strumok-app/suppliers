use std::sync::OnceLock;

use futures::future::BoxFuture;
use log::{info, warn};
use regex::Regex;

use crate::{
    extractors::{player4u, streamwish},
    models::ContentMediaItemSource,
    utils,
};

use super::SourceParams;

const URL: &str = "https://www.2embed.cc";
const REF_URL: &str = "https://streamsrcs.2embed.cc/";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let maybe_imdb_id = params.imdb_id.as_ref();
    let tmdb_id = params.id.to_string();
    let id = maybe_imdb_id.unwrap_or(&tmdb_id);
    let url = match &params.ep {
        Some(ep) => {
            let e = ep.e;
            let s = ep.s;
            format!("{URL}/embedtv/{id}&s={s}&e={e}")
        }
        None => format!("{URL}/embed/{id}"),
    };

    let res = utils::create_client()
        .post(&url)
        .header("Referer", &url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("pls=pls")
        .send()
        .await?
        .text()
        .await?;

    if let Some(items) = try_extract_player4u(&res).await {
        return Ok(items);
    }

    if let Some(items) = try_extract_streamwish(&res).await {
        return Ok(items);
    }

    Ok(vec![])
}

async fn try_extract_player4u(res: &str) -> Option<Vec<ContentMediaItemSource>> {
    static PLAYER4U_ID_RE: OnceLock<Regex> = OnceLock::new();
    let maybe_id = PLAYER4U_ID_RE
        .get_or_init(|| Regex::new(r"'(?<id>.*?player4u.*?)'").unwrap())
        .captures(res)
        .and_then(|m| m.name("id"))
        .map(|m| m.as_str());

    let id = match maybe_id {
        Some(id) => id,
        None => {
            info!("[two_embed] No player4u id found");
            return None;
        }
    };

    match player4u::extract(id, REF_URL, "2e").await {
        Ok(items) => Some(items),
        Err(e) => {
            warn!("[two_embed] player4u failed: {e:#?}");
            None
        }
    }
}

async fn try_extract_streamwish(res: &str) -> Option<Vec<ContentMediaItemSource>> {
    static STREAM_WISH_ID_RE: OnceLock<Regex> = OnceLock::new();
    let maybe_id = STREAM_WISH_ID_RE
        .get_or_init(|| Regex::new(r"swish\?id=(?<id>[\w\d]+)").unwrap())
        .captures(res)
        .and_then(|m| m.name("id"))
        .map(|m| m.as_str());

    let id = match maybe_id {
        Some(id) => id,
        None => {
            info!("[two_embed] No stream wish id found");
            return None;
        }
    };

    let player_url = streamwish::PLAYER_URL;
    let stw_result = streamwish::extract(
        format!("{player_url}/e/{id}").as_str(),
        REF_URL,
        "Two Embed",
    )
    .await;

    match stw_result {
        Ok(items) => Some(items),
        Err(e) => {
            warn!("[two_embed] streamwish failed: {e:#?}");
            None
        }
    }
}
#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[tokio::test]
    async fn should_load_tv_show_imdb() {
        let res = extract(&SourceParams {
            id: 60735,
            imdb_id: Some("tt3107288".to_string()),
            ep: Some(Episode { e: 1, s: 1 }),
        })
        .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_tv_show() {
        let res = extract(&SourceParams {
            id: 609681,
            imdb_id: None,
            ep: Some(Episode { e: 1, s: 1 }),
        })
        .await;
        println!("{res:#?}")
    }
}
