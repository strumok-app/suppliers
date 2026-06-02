use std::collections::HashMap;

use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, enc_dec_app},
};

use super::SourceParams;

const VIDLINK_URL: &str = "https://vidlink.pro";
const VIDLINK_API: &str = "https://vidlink.pro/api/b";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb_id = params.id.to_string();

    // Encrypt tmdb_id via enc-dec.app
    let encrypted = enc_dec_app::vidlink_enc(&tmdb_id).await?;

    // Build vidlink API url
    let url = match &params.ep {
        Some(ep) => format!("{VIDLINK_API}/tv/{encrypted}/{}/{}", ep.s, ep.e),
        None => format!("{VIDLINK_API}/movie/{encrypted}"),
    };

    let client = utils::create_client();
    let res_str = client
        .get(&url)
        .header("Origin", VIDLINK_URL)
        .header("Referer", format!("{VIDLINK_URL}/"))
        .send()
        .await?
        .text()
        .await?;

    // println!("{res_str}");

    let res: VidlinkResponse = serde_json::from_str(&res_str)?;

    let mut sources: Vec<ContentMediaItemSource> = vec![];

    // Process HLS stream
    let stream = res.stream;
    sources.push(ContentMediaItemSource::Video {
        link: stream.playlist,
        description: format!("[Vidlink] {}", res.source_id),
        headers: Some(HashMap::from([
            ("Origin".to_string(), VIDLINK_URL.to_string()),
            ("Referer".to_string(), format!("{VIDLINK_URL}/")),
        ])),
        hls_proxy: false,
    });

    // Process captions
    if let Some(captions) = stream.captions {
        for caption in captions {
            sources.push(ContentMediaItemSource::Subtitle {
                link: caption.url,
                description: format!("[Vidlink] {}", caption.language),
                headers: None,
            });
        }
    }

    Ok(sources)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VidlinkResponse {
    source_id: String,
    stream: VidlinkStream,
}

#[derive(Debug, Deserialize)]
struct VidlinkStream {
    playlist: String,
    captions: Option<Vec<VidlinkCaption>>,
}

#[derive(Debug, Deserialize)]
struct VidlinkCaption {
    url: String,
    language: String,
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_extract_tv() {
        let res = extract(&SourceParams {
            id: 105248,
            imdb_id: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_extract_movie() {
        let res = extract(&SourceParams {
            id: 533535,
            imdb_id: None,
            ep: None,
        })
        .await;
        println!("{res:#?}")
    }
}
