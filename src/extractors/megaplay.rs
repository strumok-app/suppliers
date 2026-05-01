use anyhow::{Ok, anyhow};
use std::{collections::HashMap, sync::OnceLock};

use crate::{
    models::ContentMediaItemSource,
    utils::{self, jwp_player::JWPConfig},
};

pub async fn extract(
    url: &str,
    referer: &str,
    title: String,
    hls_proxy: bool,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let host = url
        .split("/")
        .nth(2)
        .ok_or_else(|| anyhow!("[packer_hls] invalid url"))?;

    let iframe_res = utils::create_client()
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    // println!("{iframe_res}");

    static DATA_ID_RE: OnceLock<regex::Regex> = OnceLock::new();
    let data_id_re =
        DATA_ID_RE.get_or_init(|| regex::Regex::new(r#"data-id="(?<id>[^"]+)""#).unwrap());

    let maybe_data_id = data_id_re
        .captures(&iframe_res)
        .and_then(|c| c.name("id").map(|m| m.as_str()));

    if maybe_data_id.is_none() {
        return Ok(vec![]);
    }

    let data_id = maybe_data_id.unwrap();
    let api_url = format!("https://{host}/stream/getSources?id={data_id}&id={data_id}");

    let jwpconfig_str = utils::create_json_client()
        .get(&api_url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    //println!("{jwpconfig_str}");

    let jwpconfig: JWPConfig = serde_json::from_str(&jwpconfig_str)?;

    //dbg!(&jwpconfig);

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
}
