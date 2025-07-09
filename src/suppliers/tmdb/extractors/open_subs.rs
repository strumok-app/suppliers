use anyhow::Ok;
use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{models::ContentMediaItemSource, utils};

use super::SourceParams;

const STREIO_URL: &str = "https://opensubtitles.stremio.homes";

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
    let imdb_id = match &params.imdb_id {
        Some(v) => v,
        None => return Ok(vec![]),
    };

    let langs_str = langs.join("|");
    let base_url =
        format!("{STREIO_URL}/{langs_str}/ai-translated=true|from=all|Cauto-adjustment=true");

    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{base_url}/subtitles/series/{imdb_id}:{s}:{e}.json")
        }
        None => format!("{base_url}/subtitles/movie/{imdb_id}.json"),
    };

    

    let res_str = utils::create_client_builder()
        .build()?
        .get(link)
        .send()
        .await?
        .text()
        .await?;

    

    #[derive(Debug, Deserialize)]
    struct SubtitleRes {
        ai_translated: bool,
        lang: String,
        url: String,
    }

    #[derive(Debug, Deserialize)]
    struct ServerRes {
        subtitles: Vec<SubtitleRes>,
    }

    let res: ServerRes = serde_json::from_str(&res_str)?;

    let subs: Vec<_> = res
        .subtitles
        .into_iter()
        .enumerate()
        .map(|(idx, sub)| {
            let num = idx + 1;
            let lang = sub.lang;
            let mut description = format!("[open sub] {num}. {lang}");
            if sub.ai_translated {
                description = format!("{description} (AI)")
            }

            ContentMediaItemSource::Subtitle {
                link: sub.url,
                description,
                headers: None,
            }
        })
        .collect();

    Ok(subs)
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;
    #[tokio::test]
    async fn should_extract_movie() {
        let res = extract(
            &SourceParams {
                id: 280,
                imdb_id: Some("tt0103064".to_owned()),
                ep: None,
            },
            &["en".to_owned(), "uk".to_owned()],
        )
        .await;

        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_extract_tv() {
        let res = extract(
            &SourceParams {
                id: 655,
                imdb_id: Some("tt0092455".to_owned()),
                ep: Some(Episode { e: 1, s: 1 }),
            },
            &["en".to_owned(), "uk".to_owned()],
        )
        .await;

        println!("{res:#?}")
    }
}
