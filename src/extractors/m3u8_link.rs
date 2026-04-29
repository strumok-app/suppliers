use std::{collections::HashMap, sync::OnceLock};

use crate::{models::ContentMediaItemSource, utils};

pub async fn extract(
    url: &str,
    referer: &str,
    title: String,
    hls_proxy: bool,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let iframe_res = utils::create_client()
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    // println!("{iframe_res}");

    static M3U8_REGEX: OnceLock<regex::Regex> = OnceLock::new();
    let m3u8_re =
        M3U8_REGEX.get_or_init(|| regex::Regex::new(r#"(?P<url>https?://[^"']+\.m3u8)"#).unwrap());

    let maybe_playlist = m3u8_re
        .captures(&iframe_res)
        .map(|c| c.name("url").unwrap().as_str());

    match maybe_playlist {
        Some(url) => Ok(vec![ContentMediaItemSource::Video {
            link: url.to_string(),
            description: title,
            headers: Some(HashMap::from([("Referer".to_string(), url.to_string())])),
            hls_proxy: hls_proxy,
        }]),
        None => Ok(vec![]),
    }
}
