use anyhow::{Ok, anyhow};
use std::{collections::HashMap, sync::OnceLock};

use crate::{
    models::ContentMediaItemSource,
    utils::{self, jwp_player::JWPConfig},
};

static DATA_ID_RE: OnceLock<regex::Regex> = OnceLock::new();
static IFRAME_SRC_RE: OnceLock<regex::Regex> = OnceLock::new();

fn find_data_id(html: &str) -> Option<&str> {
    let data_id_re =
        DATA_ID_RE.get_or_init(|| regex::Regex::new(r#"data-id="(?<id>[^"]+)""#).unwrap());
    data_id_re
        .captures(html)
        .and_then(|c| c.name("id").map(|m| m.as_str()))
}

fn find_nested_iframe_src(html: &str) -> Option<&str> {
    let iframe_src_re = IFRAME_SRC_RE
        .get_or_init(|| regex::Regex::new(r#"<iframe[^>]+src="(?<src>[^"]+)""#).unwrap());
    iframe_src_re
        .captures(html)
        .and_then(|c| c.name("src").map(|m| m.as_str()))
}

pub async fn extract(
    url: &str,
    referer: &str,
    title: String,
    hls_proxy: bool,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let host = url
        .split("/")
        .nth(2)
        .ok_or_else(|| anyhow!("[megaplay] invalid url"))?;

    let iframe_res = utils::create_client()
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    let data_id = if let Some(id) = find_data_id(&iframe_res) {
        id.to_string()
    } else if let Some(nested_url) = find_nested_iframe_src(&iframe_res) {
        let nested_res = utils::create_client()
            .get(nested_url)
            .header("Referer", &format!("https://{host}/"))
            .send()
            .await?
            .text()
            .await?;

        match find_data_id(&nested_res) {
            Some(id) => id.to_string(),
            None => return Ok(vec![]),
        }
    } else {
        return Ok(vec![]);
    };

    let api_url = format!("https://{host}/stream/getSources?id={data_id}&id={data_id}");

    let jwpconfig_str = utils::create_json_client()
        .get(&api_url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    let jwpconfig: JWPConfig = serde_json::from_str(&jwpconfig_str)?;

    Ok(jwpconfig.to_media_item_sources(
        &title,
        Some(HashMap::from([(
            "Referer".to_string(),
            format!("https://{host}/"),
        )])),
        hls_proxy,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_megaplay_extractor() {
        let url = "https://megaplay.buzz/stream/s-1/L3F1cTNLWThmTnV5MWxSU3RWNUVVdz09";
        let referer = "https://anikototv.to/watch/sakamoto-days-sfdxz/ep-5";

        let res = extract(url, referer, "Test".to_string(), true).await;

        println!("{res:#?}");
    }

    #[tokio::test]
    async fn test_megaplay_neste_extractor() {
        let url = "https://megaplay.buzz/stream/s-2/373559/sub?autostart=true";
        let referer = "https://anikototv.to/";

        let res = extract(url, referer, "Test".to_string(), true).await;

        println!("{res:#?}");
    }
}
