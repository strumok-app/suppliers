use std::collections::HashMap;

use base64::{Engine, prelude::BASE64_STANDARD};
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, crypto_js, jwp_player},
};

const KEYS_URL: &str =
    "https://raw.githubusercontent.com/yogesh-hacker/MegacloudKeys/refs/heads/main/keys.json";

const REFERER: &str = "https://megacloud.blog/";

pub async fn extract(
    url: &str,
    _referer: &str,
    prefix: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = url
        .rsplit_once("/")
        .and_then(|(_, r)| r.split_once("?").map(|(l, _)| l).or(Some(r)))
        .unwrap();

    let key = get_key().await?;

    let sources_res_str = utils::create_client()
        .get(format!(
            "https://megacloud.blog/embed-2/v3/e-1/getSources?id={id}"
        ))
        .send()
        .await?
        .text()
        .await?;

    // println!("{sources_res_str}");

    #[derive(Debug, Deserialize)]
    struct EnctypredSource {
        sources: String,
        tracks: Vec<jwp_player::Track>,
    }

    let sources_res: EnctypredSource = serde_json::from_str(&sources_res_str)?;

    // decrypt_aes(key, iv, ct);
    let b64_decodes = BASE64_STANDARD.decode(sources_res.sources.as_bytes())?;
    let decryptes_source = crypto_js::decrypt_aes_no_salt(key.as_bytes(), &b64_decodes)?;

    let jwp_sources: Vec<jwp_player::Source> = serde_json::from_str(&decryptes_source)?;

    let jwp_config = jwp_player::JWPConfig {
        sources: jwp_sources,
        tracks: sources_res.tracks,
    };

    Ok(jwp_config.to_media_item_sources(
        prefix,
        Some(HashMap::from([("Referer".into(), REFERER.into())])),
    ))
}

pub async fn get_key() -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct MegacloudKeysResponse {
        mega: String,
    }

    let res: MegacloudKeysResponse = utils::create_client()
        .get(KEYS_URL)
        .send()
        .await?
        .json()
        .await?;

    Ok(res.mega)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_episode1_load_by_link() {
        let link = "https://megacloud.blog/embed-2/v3/e-1/PN9QqotdYAT6?k=1";
        let sources = extract(link, "https://hianime.to", "Megacloud")
            .await
            .unwrap();
        println!("{sources:#?}")
    }
}
