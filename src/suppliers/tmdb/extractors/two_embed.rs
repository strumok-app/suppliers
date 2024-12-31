use std::sync::OnceLock;

use log::info;
use regex::Regex;

use crate::{extractors::streamwish, models::ContentMediaItemSource, utils};

use super::SourceParams;

const URL: &str = "https://www.2embed.cc";
const PLAYER_URL: &str = "https://uqloads.xyz";
const REF_URL: &str = "https://streamsrcs.2embed.cc/";

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb_id = params.id;
    let url = match params.episode {
        Some(ep) => {
            let s = params.season.unwrap_or(0);
            format!("{URL}/embedtv/{tmdb_id}&s={s}&e={ep}")
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

    println!("{PLAYER_URL}/e/{id}");

    streamwish::extract(
        format!("{PLAYER_URL}/e/{id}").as_str(),
        REF_URL,
        "[Two Embed]",
    )
    .await
}
#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn should_load_movie() {
        let res = extract(&SourceParams {
            id: 609681,
            imdb_id: None,
            episode: None,
            season: None,
        })
        .await;
        println!("{res:#?}")
    }
}
