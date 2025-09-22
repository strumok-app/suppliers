use std::{collections::HashMap, sync::OnceLock};

use anyhow::Ok;
use log::warn;
use regex::Regex;

use crate::{
    models::ContentMediaItemSource,
    utils::{create_client, jwp_player},
};

const URL: &str = "https://megacloud.blog/";

// regex's for the following key obfuscation methods
// <meta name="_gg_fb" content="${CLIENTKEY}">                                                          || meta tag
// <!-- _is_th:${CLIENTKEY} -->                                                                         || comment
// <script>window._lk_db = {x: "${CLIENTKEY_P1}",y: "${CLIENTKEY_P2}",Z: "${CLIENTKEY_P3}"};</script>   || 3 part key in script (eval would work)
// <div data-dpi="${CLIENTKEY}"><\/div>                                                                 || div tag
// <script nonce="${CLIENTKEY}">                                                                        || nonce value
// <script>window._xy_ws = "${CLIENTKEY}";<\/script>

pub async fn extract(
    url: &str,
    referer: &str,
    prefix: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let client = create_client();

    let id = url
        .rsplit_once("/")
        .and_then(|(_, r)| r.split_once("?").map(|(l, _)| l).or(Some(r)))
        .unwrap();

    let html = client
        .get(url)
        .header("Referer", referer)
        .send()
        .await?
        .text()
        .await?;

    // println!("{html}");

    let key = match try_extract_key(html) {
        Some(k) => k,
        None => {
            warn!("[maegacloud3] key not found");
            return Ok(vec![]);
        }
    };

    println!("{key:?}");

    let sources_res_str = client
        .get(format!("{URL}embed-2/v3/e-1/getSources?id={id}&_k={key}"))
        .header("Referer", url)
        .send()
        .await?
        .text()
        .await?;

    // println!("{sources_res_str}");

    let jwp_config: jwp_player::JWPConfig = serde_json::from_str(&sources_res_str)?;

    Ok(jwp_config.to_media_item_sources(
        prefix,
        Some(HashMap::from([("Referer".into(), URL.into())])),
    ))
}

fn try_extract_key(html: String) -> Option<String> {
    let mut key: Option<String>;

    key = try_extract_key_1(&html);

    if key.is_some() {
        return key;
    }

    key = try_extract_key_2(&html);
    if key.is_some() {
        return key;
    }

    key = try_extract_key_3(&html);
    if key.is_some() {
        return key;
    }

    key = try_extract_key_4(&html);
    if key.is_some() {
        return key;
    }

    key = try_extract_key_5(&html);
    if key.is_some() {
        return key;
    }

    try_extract_key_6(&html)
}

fn try_extract_key_1(html: &str) -> Option<String> {
    static REG_EX: OnceLock<Regex> = OnceLock::new();

    REG_EX
        .get_or_init(|| Regex::new(r#"<meta name="_gg_fb" content="([a-zA-Z0-9]+)">"#).unwrap())
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn try_extract_key_2(html: &str) -> Option<String> {
    static REG_EX: OnceLock<Regex> = OnceLock::new();

    REG_EX
        .get_or_init(|| Regex::new(r#"<!--\s+_is_th:([0-9a-zA-Z]+)\s+-->"#).unwrap())
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn try_extract_key_3(html: &str) -> Option<String> {
    static REG_EX: OnceLock<Regex> = OnceLock::new();

    fn get_idx(p: &str) -> usize {
        match p {
            "x" | "X" => 0,
            "y" | "Y" => 1,
            _ => 2,
        }
    }

    REG_EX
        .get_or_init(|| Regex::new(r#"<script>window\._lk_db\s+=\s+\{([xyzXYZ]):\s+["'']([a-zA-Z0-9]+)["''],\s+([xyzXYZ]):\s+["'']([a-zA-Z0-9]+)["''],\s+([xyzXYZ]):\s+["'']([a-zA-Z0-9]+)["'']\};</script>"#).unwrap())
        .captures(html)
        .and_then(|c| {
            let mut parts: [&str; 3] = Default::default();

            let p1p= c.get(1)?.as_str();
            let p1= c.get(2)?.as_str();
            parts[get_idx(p1p)] = p1;

            let p2p= c.get(3)?.as_str();
            let p2= c.get(4)?.as_str();
            parts[get_idx(p2p)] = p2;

            let p3p= c.get(5)?.as_str();
            let p3= c.get(6)?.as_str();
            parts[get_idx(p3p)] = p3;

            Some(parts.join(""))
        })
}

fn try_extract_key_4(html: &str) -> Option<String> {
    static REG_EX: OnceLock<Regex> = OnceLock::new();

    REG_EX
        .get_or_init(|| Regex::new(r#"<div\s+data-dpi="([0-9a-zA-Z]+)".*></div>"#).unwrap())
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn try_extract_key_5(html: &str) -> Option<String> {
    static REG_EX: OnceLock<Regex> = OnceLock::new();

    REG_EX
        .get_or_init(|| Regex::new(r#"<script nonce="([0-9a-zA-Z]+)">"#).unwrap())
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn try_extract_key_6(html: &str) -> Option<String> {
    static REG_EX: OnceLock<Regex> = OnceLock::new();

    REG_EX
        .get_or_init(|| {
            Regex::new(r#"<script>window._xy_ws\s*=\s*['"`]([0-9a-zA-Z]+)['"`];</script>"#).unwrap()
        })
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_try_extract_1() {
        let res = try_extract_key_1(r#"<meta name="_gg_fb" content="aB0">"#);
        assert_eq!(res, Some("aB0".to_string()))
    }

    #[test]
    fn should_try_extract_2() {
        let res = try_extract_key_2(r#"<!-- _is_th:aB0 --> "#);
        assert_eq!(res, Some("aB0".to_string()))
    }

    #[test]
    fn should_try_extract_3() {
        let res =
            try_extract_key_3(r#"<script>window._lk_db = {x: "a", y: "B", Z: "0"};</script>"#);
        assert_eq!(res, Some("aB0".to_string()))
    }

    #[test]
    fn should_try_extract_4() {
        let res = try_extract_key_4(r#"<div data-dpi="aB0"></div>"#);
        assert_eq!(res, Some("aB0".to_string()))
    }

    #[test]
    fn should_try_extract_5() {
        let res = try_extract_key_5(r#"<script nonce="aB0">"#);
        assert_eq!(res, Some("aB0".to_string()))
    }

    #[test]
    fn should_try_extract_6() {
        let res = try_extract_key_6(r#"<script>window._xy_ws = "aB0";</script>"#);
        assert_eq!(res, Some("aB0".to_string()))
    }

    #[test_log::test(tokio::test)]
    async fn should_episode1_load_by_link() {
        let link = "https://megacloud.blog/embed-2/v3/e-1/heQfyQKzZW3S?k=1";
        let sources = extract(link, "https://hianime.to", "Megacloud")
            .await
            .unwrap();
        println!("{sources:#?}")
    }
}
