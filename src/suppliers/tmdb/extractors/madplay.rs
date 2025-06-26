// https://madplay.site/api/playsrc?id=1855&season=1&episode=1
// https://madplay.site/api/subtitle?id=1855&season=1&episode=1

use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{models::ContentMediaItemSource, utils};

use super::SourceParams;

pub const BACKEND_URL: &str = "https://madplay.site";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params, langs))
}

pub async fn extract(
    params: &SourceParams,
    _langs: &[String],
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = params.id;

    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{BACKEND_URL}/api/playsrc?id={id}&season={s}&episode={e}")
        }
        None => format!("{BACKEND_URL}/api/playsrc?id={id}"),
    };

    #[derive(Debug, Deserialize)]
    struct ServerRes {
        file: String,
    }

    let res_str = utils::create_json_client()
        .get(link)
        .send()
        .await?
        .text()
        .await?;

    // println!("{res_str}");

    let files: Vec<ServerRes> = serde_json::from_str(&res_str)?;

    // println!("{res:#?}");

    let sources: Vec<_> = files
        .into_iter()
        .enumerate()
        .map(|(idx, file)| {
            let num = idx + 1;
            ContentMediaItemSource::Video {
                link: file.file,
                description: format!("{num}. madplay"),
                headers: None,
            }
        })
        .collect();

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

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
