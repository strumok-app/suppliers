use std::collections::HashMap;

use anyhow::Ok;
use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{models::ContentMediaItemSource, utils::create_json_client};

use super::SourceParams;

const BACKEND_URL: &str = "https://backend.xprime.tv";
const URL: &str = "https://xprime.tv";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = params.id;

    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{BACKEND_URL}/primenet?id={id}&season={s}&episode={e}")
        }
        None => format!("{BACKEND_URL}/primenet?id={id}"),
    };

    #[derive(Debug, Deserialize)]
    struct ServerRes {
        url: String,
    }

    let server_res: ServerRes = create_json_client()
        .get(link)
        .header("Referer", URL)
        .header("Origin", URL)
        .send()
        .await?
        .json()
        .await?;

    // println!("{server_res:#?}");

    Ok(vec![ContentMediaItemSource::Video {
        link: server_res.url,
        description: "xprime".to_owned(),
        headers: Some(HashMap::from([("Referer".to_owned(), URL.to_owned())])),
    }])
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[tokio::test]
    async fn should_extract_tv() {
        let res = extract(&SourceParams {
            id: 655,
            imdb_id: None,
            ep: Some(Episode { e: 1, s: 1 }),
        })
        .await;

        println!("{res:#?}")
    }
}
