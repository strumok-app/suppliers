use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use base64::{prelude::BASE64_STANDARD, Engine};
use scraper::Selector;
use serde::Deserialize;
use url::Url;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, crypto, jwp_player},
};

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let (iv, sec_key, dec_key, data) = parse_iframe(url).await?;

    let ajax_params_bytes = crypto::decrypt_base64_aes(&sec_key, &iv, &data)?;

    let ajax_params_str = String::from_utf8(ajax_params_bytes)?;
    let ajax_params = ajax_params_str
        .split_once("&")
        .map(|(_, v)| v)
        .ok_or_else(|| anyhow!("[gogstream] no ajax params"))?;

    let url_parsed = Url::parse(url)?;
    let id = url_parsed
        .query_pairs()
        .find(|(key, _)| key == "id")
        .map(|(_, v)| v)
        .ok_or_else(|| anyhow!("[gogostream] id not found in url"))?;

    let encoded_id_bytes = crypto::encrypt_aes(&sec_key, &iv, id.as_bytes())?;
    let encoded_id = BASE64_STANDARD.encode(encoded_id_bytes);

    let host = url_parsed.host().unwrap().to_string();
    let links_url =
        format!("https://{host}/encrypt-ajax.php?id={encoded_id}&{ajax_params}&alias={id}");

    let links_res_str = utils::create_client()
        .get(links_url)
        .header("X-Requested-With", "XMLHttpRequest")
        .send()
        .await?
        .text()
        .await?;

    #[derive(Deserialize)]
    struct LinksRes {
        data: String,
    }

    let links_res: LinksRes = serde_json::from_str(&links_res_str)?;

    let links_config_bytes = crypto::decrypt_base64_aes(&dec_key, &iv, links_res.data.as_bytes())?;

    #[derive(Deserialize)]
    struct LinksConfig {
        source: Vec<jwp_player::Source>,
        source_bk: Vec<jwp_player::Source>,
        track: Vec<jwp_player::Track>,
    }

    let links_config: LinksConfig = serde_json::from_slice(&links_config_bytes)?;

    let mut sources = vec![];
    sources.extend(links_config.source);
    sources.extend(links_config.source_bk);

    let jwp_config = jwp_player::JWPConfig {
        sources,
        tracks: links_config.track,
    };

    Ok(jwp_config.to_media_item_sources(
        prefix,
        Some(HashMap::from([("Referer".into(), url.into())])),
    ))
}

async fn parse_iframe(url: &str) -> anyhow::Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
    static IV_SELECTOR: OnceLock<Selector> = OnceLock::new();
    static SEC_KEY_SELECTOR: OnceLock<Selector> = OnceLock::new();
    static DEC_KEY_SELECTOR: OnceLock<Selector> = OnceLock::new();
    static DATA_SELECTOR: OnceLock<Selector> = OnceLock::new();

    let iframe = utils::create_client().get(url).send().await?.text().await?;

    let iframe_document = scraper::Html::parse_document(&iframe);

    let maybe_iv = iframe_document
        .select(IV_SELECTOR.get_or_init(|| Selector::parse("div.wrapper").unwrap()))
        .filter_map(|el| el.attr("class"))
        .filter_map(|classes| find_key(classes, "container-"))
        .next();

    if maybe_iv.is_none() {
        return Err(anyhow!("[gogostream] no encyption params"));
    }

    let maybe_sec_key = iframe_document
        .select(SEC_KEY_SELECTOR.get_or_init(|| Selector::parse("body").unwrap()))
        .filter_map(|el| el.attr("class"))
        .filter_map(|classes| find_key(classes, "container-"))
        .next();

    if maybe_sec_key.is_none() {
        return Err(anyhow!("[gogostream] no encyption params"));
    }

    let maybe_dec_key = iframe_document
        .select(DEC_KEY_SELECTOR.get_or_init(|| Selector::parse("div.videocontent").unwrap()))
        .filter_map(|el| el.attr("class"))
        .filter_map(|classes| find_key(classes, "videocontent-"))
        .next();

    if maybe_dec_key.is_none() {
        return Err(anyhow!("[gogostream] no encyption params"));
    }

    let maybe_data = iframe_document
        .select(DATA_SELECTOR.get_or_init(|| Selector::parse("script[data-value]").unwrap()))
        .filter_map(|el| el.attr("data-value"))
        .next();

    if maybe_data.is_none() {
        return Err(anyhow!("[gogostream] no encyption params"));
    }

    Ok((
        maybe_iv.unwrap().as_bytes().to_vec(),
        maybe_sec_key.unwrap().as_bytes().to_vec(),
        maybe_dec_key.unwrap().as_bytes().to_vec(),
        maybe_data.unwrap().as_bytes().to_vec(),
    ))
}

fn find_key<'a>(classes: &'a str, prefix: &'a str) -> Option<&'a str> {
    classes.split_once(prefix).map(|(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_load_sources() {
        let result = extract("https://s3embtaku.pro/embedplus?id=MTIzOTc3&token=tjMJsk7ukddplzOXoxnUcQ&expires=1734193918", "gogo").await.unwrap();
        println!("{result:#?}")
    }
}
