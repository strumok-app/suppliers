use std::collections::HashMap;

use base64::{prelude::BASE64_STANDARD, Engine};
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, crypto_js, jwp_player},
};

const KEYS_URL: &str =
    "https://raw.githubusercontent.com/itzzzme/megacloud-keys/refs/heads/main/key.txt";

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

    // println!("{id}");

    let key = get_key().await?;

    // println!("{key}");

    let sources_res_str = utils::create_client()
        .get(format!(
            "https://megacloud.blog/embed-2/v2/e-1/getSources?id={id}"
        ))
        .send()
        .await?
        .text()
        .await?;

    println!("{sources_res_str}");

    #[derive(Debug, Deserialize)]
    struct EnctypredSource {
        sources: String,
        tracks: Vec<jwp_player::Track>,
    }

    let sources_res: EnctypredSource = serde_json::from_str(&sources_res_str)?;

    // println!("{sources_res:#?}");

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
    let key = utils::create_client()
        .get(KEYS_URL)
        .send()
        .await?
        .text()
        .await?;

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_episode1_load_by_link() {
        let link = "https://megacloud.blog/embed-2/v2/e-1/sMAztEG3Egnz?k=1&autoPlay=1&oa=0&asi=1";
        let sources = extract(link, "https://hianime.to", "Megacloud")
            .await
            .unwrap();
        println!("{sources:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_episode2_load_by_link() {
        let link = "https://megacloud.blog/embed-2/v2/e-1/bTxOgLYjOz0s?k=1&autoPlay=1&oa=0&asi=1";
        let sources = extract(link, "https://hianime.to", "Megacloud")
            .await
            .unwrap();
        println!("{sources:#?}")
    }
}
