use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use regex::Regex;

use crate::{
    models::ContentMediaItemSource,
    utils::{self, crypto},
};

const URL: &str = "https://primevid.click";
const KEY: &str = "kiemtienmua911ca";
const IV: &str = "$%&'()*+,#oitxtr";

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let hash = match url.split_once("#") {
        Some((_, r)) => r,
        None => return Err(anyhow!("[PrimeVid] hash not found in url")),
    };

    let client = utils::create_client();
    let api_url = format!("{URL}/api/v1/video?id={hash}&w=1960&h=1080&r=primewire.tf");

    let mut res = client
        .get(&api_url)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;

    res.pop();

    let ct = hex::decode(&res)?;

    let pt_bytes = crypto::decrypt_aes128(KEY.as_bytes(), IV.as_bytes(), &ct)?;

    let pt = String::from_utf8(pt_bytes)?;

    static HLS_URL_RE: OnceLock<Regex> = OnceLock::new();
    let hls_url_re = HLS_URL_RE.get_or_init(|| Regex::new(r#"cf":"([^"]+)"#).unwrap());

    let (_, [hls_url]) = match hls_url_re.captures(&pt) {
        Some(cap) => cap.extract(),
        None => return Err(anyhow!("[PrimeVid] ulr not found in decoded response")),
    };

    let final_url = hls_url.replace("\\", "");

    // println!("{final_url}");

    Ok(vec![ContentMediaItemSource::Video {
        link: final_url.to_string(),
        description: prefix.to_string(),
        headers: Some(HashMap::from([("Referer".to_string(), URL.to_string())])),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_extract() {
        let res = extract("https://primevid.click/?api=all#esi1k", "[Primewire]").await;
        println!("{res:?}");
    }
}
