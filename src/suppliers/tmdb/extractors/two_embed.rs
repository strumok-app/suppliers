use std::sync::OnceLock;

use futures::future::BoxFuture;
use log::info;
use regex::Regex;

use crate::{extractors::streamwish, models::ContentMediaItemSource, utils};

use super::SourceParams;

const URL: &str = "https://www.2embed.cc";
const PLAYER_URL: &str = "https://uqloads.xyz";
const REF_URL: &str = "https://streamsrcs.2embed.cc/";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb_id = params.id;
    let url = match &params.ep {
        Some(ep) => {
            let e = ep.e;
            let s = ep.s;
            format!("{URL}/embedtv/{tmdb_id}&s={s}&e={e}")
        }
        None => format!("{URL}/embed/{tmdb_id}"),
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

    // println!("{PLAYER_URL}/e/{id}");

    streamwish::extract(
        format!("{PLAYER_URL}/e/{id}").as_str(),
        REF_URL,
        "Two Embed",
    )
    .await
}
#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[tokio::test]
    async fn should_load_movie() {
        let res = extract(&SourceParams {
            id: 609681,
            imdb_id: None,
            ep: Some(Episode { e: 1, s: 1 }),
        })
        .await;
        println!("{res:#?}")
    }
}
