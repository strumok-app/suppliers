use std::collections::HashMap;

use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, GenericResponse, create_json_client, enc_dec_app::ENC_DEC_APP_URL},
};

use super::SourceParams;

const VIDLINK_URL: &str = "https://vidlink.pro";
const VIDLINK_API: &str = "https://vidlink.pro/api/b";

// enc-dec.app API helper

async fn vidlink_enc(tmdb_id: &str) -> anyhow::Result<String> {
    let url = format!("{ENC_DEC_APP_URL}/api/enc-vidlink?text={tmdb_id}");

    let res_str = create_json_client().get(url).send().await?.text().await?;

    let res: GenericResponse = serde_json::from_str(&res_str)?;

    Ok(res.result)
}

// Extractor

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb_id = params.id.to_string();

    // Encrypt tmdb_id via enc-dec.app
    let encrypted = vidlink_enc(&tmdb_id).await?;

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

    let res: VidlinkResponse = serde_json::from_str(&res_str)?;

    let mut sources: Vec<ContentMediaItemSource> = vec![];

    // Process file streams by quality
    let VidlinkResponse { source_id, stream } = res;

    let mut qualities: Vec<_> = stream.qualities.into_iter().collect();
    qualities.sort_by_key(|(quality, _)| quality.parse::<u16>().unwrap_or_default());
    qualities.reverse();

    for (quality, stream) in qualities {
        let quality_label = if quality.chars().all(|ch| ch.is_ascii_digit()) {
            format!("{quality}p")
        } else {
            quality
        };

        sources.push(ContentMediaItemSource::Video {
            link: stream.url,
            description: format!("[Vidlink] {source_id} - {quality_label}"),
            headers: Some(HashMap::from([
                ("Origin".to_string(), VIDLINK_URL.to_string()),
                ("Referer".to_string(), format!("{VIDLINK_URL}/")),
            ])),
            hls_proxy: false,
        });
    }

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
    qualities: HashMap<String, VidlinkQuality>,
    captions: Option<Vec<VidlinkCaption>>,
}

#[derive(Debug, Deserialize)]
struct VidlinkQuality {
    url: String,
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
    async fn vidlink_should_extract_tv() {
        let res = extract(&SourceParams {
            id: 105248,
            imdb_id: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn vidlink_should_extract_movie() {
        let res = extract(&SourceParams {
            id: 786892,
            imdb_id: None,
            ep: None,
        })
        .await;
        println!("{res:#?}")
    }
}
