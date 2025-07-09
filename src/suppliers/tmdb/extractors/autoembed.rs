use anyhow::Ok;
use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{create_json_client, lang},
};

use super::SourceParams;

const BACKEND_URL: &str = "https://tom.autoembed.cc";
const URL: &str = "https://autoembed.cc";

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
            let s = ep.s;
            let e = ep.e;
            format!("{BACKEND_URL}/api/getVideoSource?type=tv&id={id}/{s}/{e}")
        }
        None => format!("{BACKEND_URL}/api/getVideoSource?type=movie&id={id}"),
    };

    #[derive(Debug, Deserialize)]
    struct ServerResSubtitle {
        file: String,
        label: String,
    }

    #[derive(Debug, Deserialize)]
    struct ServerRes {
        #[serde(rename = "videoSource")]
        video_source: String,
        subtitles: Vec<ServerResSubtitle>,
    }

    let server_res_str = create_json_client()
        .get(link)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;

    

    let server_res: ServerRes = serde_json::from_str(&server_res_str)?;

    let mut sources = vec![ContentMediaItemSource::Video {
        link: server_res.video_source,
        description: "autoembed".to_owned(),
        headers: None,
    }];

    server_res
        .subtitles
        .into_iter()
        .enumerate()
        .for_each(|(i, sub)| {
            let name = sub.label;
            let num = i + 1;

            if lang::is_allowed(langs, &name) {
                sources.push(ContentMediaItemSource::Subtitle {
                    link: sub.file,
                    description: format!("[autoembed] {num}. {name}"),
                    headers: None,
                });
            }
        });

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
