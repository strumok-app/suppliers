use std::{collections::HashMap, sync::Arc};

use anyhow::anyhow;
use base64::{Engine, prelude::BASE64_STANDARD};
use futures::future::BoxFuture;
use log::warn;
use md5::Digest;
use reqwest::Client;
use serde::Deserialize;
use sha2::Sha256;

use crate::{
    models::ContentMediaItemSource,
    suppliers::tmdb::extractors::SourceParams,
    utils::{create_client, crypto},
};

const KEY_PASS: &str = "b3f2a9d4c6e1f8a7b";
const CORE_URL: &str = "https://core.vidzee.wtf";
const PLAYER_URL: &str = "https://player.vidzee.wtf";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let client = create_client();

    let api_key = client
        .get(format!("{CORE_URL}/api-key"))
        .send()
        .await?
        .text()
        .await?;

    // println!("{api_key}");

    let key = decrypt_key(&api_key)?;
    let shared_key = Arc::new(key);

    // println!("{key:?}");

    let servers_itr = (0..8).map(|sr| {
        let key = Arc::clone(&shared_key);
        async move {
            match load_server(client, sr, params, &key).await {
                Ok(r) => r,
                Err(err) => {
                    warn!("[vidzee] server '{sr}' failed: {err}");
                    vec![]
                }
            }
        }
    });

    let results = futures::future::join_all(servers_itr)
        .await
        .into_iter()
        .flatten()
        .collect();

    Ok(results)
}

// let ez = "b3f2a9d4c6e1f8a7b";
// let apiKey = ""
// let apiKeyBytes = function (e) {
//   //b64 decode
//   let t = atob(e.replace(/\s+/g, ""));
//   let a = t.length;
//   let r = new Uint8Array(a);
//   for (let e = 0; e < a; e++) {
//     r[e] = t.charCodeAt(e);
//   }
//   return r;
// }(apiKey);
//
// // if (apiKeyBytes.length <= 28) {
// //   return "";
// // }
//
// let a = apiKeyBytes.slice(0, 12);
// let r = apiKeyBytes.slice(12, 28);
// let l = apiKeyBytes.slice(28);
// let ct = new Uint8Array(l.length + r.length);
//
// ct.set(l, 0);
// ct.set(r, l.length);
//
// let n = new TextEncoder();
//
// let key = await crypto.subtle.digest("SHA-256", n.encode(ez));
// let cryptoKey = await crypto.subtle.importKey("raw", key, {
//   name: "AES-GCM"
// }, false, ["decrypt"]);
//
// let c = await crypto.subtle.decrypt({
//   name: "AES-GCM",
//   iv: a,
//   tagLength: 128
// }, cryptoKey, ct);
//
// key = new TextDecoder().decode(c);
//
fn decrypt_key(api_key: &str) -> anyhow::Result<Vec<u8>> {
    let api_key_bytes = BASE64_STANDARD.decode(api_key)?;

    // println!("{api_key_bytes:?}");

    let iv = &api_key_bytes[..12];

    // println!("iv len: {}", iv.len());

    let r = &api_key_bytes[12..28];
    let l = &api_key_bytes[28..];

    let ct = [l, r].concat();
    // println!("ct len: {}", ct.len());

    let key = KEY_PASS.as_bytes();
    let key = Sha256::digest(key);

    // println!("key len: {}", key.len());

    let pt = crypto::decrypt_aes_gcm(&key, iv, &ct)?;

    Ok(pt)
}

async fn load_server(
    client: &Client,
    sr: u8,
    params: &SourceParams,
    key: &[u8],
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = params.id;
    let mut link = format!("{PLAYER_URL}/api/server?id={id}&sr={sr}");

    if let Some(e) = &params.ep {
        link = format!("{}&ss={}&ep={}", link, e.s, e.e);
    }

    let server_res_str = client
        .get(link)
        .header("Referer", PLAYER_URL)
        .send()
        .await?
        .text()
        .await?;

    // println!("{server_res_str}");

    #[derive(Deserialize, Debug)]
    struct ServerUrl {
        r#type: String,
        link: String,
        lang: String,
    }

    #[derive(Deserialize, Debug)]
    struct ServerRes {
        provider: String,
        url: Vec<ServerUrl>,
    }

    let server_res: ServerRes = serde_json::from_str(&server_res_str)?;

    // println!("{server_res:?}");
    let mut res: Vec<ContentMediaItemSource> = vec![];

    for su in server_res.url {
        if su.r#type != "hls" {
            continue;
        }

        let dec_link = decode_link(&su.link, key)?;
        // println!("{dec_link}");
        res.push(ContentMediaItemSource::Video {
            link: dec_link,
            description: format!("[Vidzee] {} - {}", server_res.provider, su.lang),
            headers: Some(HashMap::from([(
                "Referer".to_string(),
                PLAYER_URL.to_string(),
            )])),
        });
    }

    Ok(res)
}
// let [a, r] = atob(e).split(":");
// if (!a || !r) {
//     return "";
// }
// let l = CryptoJS.enc.Base64.parse(a);
// let n = CryptoJS.enc.Utf8.parse(t.padEnd(32, "\0"));
// let o = CryptoJS.AES.decrypt(r, n, {
//     iv: l,
//     mode: s().mode.CBC,
//     padding: s().pad.Pkcs7
// }).toString(s().enc.Utf8);
// if (!o) {
//     return "";
// }
// return o;
fn decode_link(link: &str, pass: &[u8]) -> anyhow::Result<String> {
    let iv_and_ct = BASE64_STANDARD.decode(link)?;
    let sep_idx = iv_and_ct
        .iter()
        .position(|&it| it == b':')
        .ok_or_else(|| anyhow!("IV not found in link"))?;

    let iv_b64 = &iv_and_ct[0..sep_idx];
    let ct = &iv_and_ct[sep_idx + 1..];

    let iv = BASE64_STANDARD.decode(iv_b64)?;
    let ct = BASE64_STANDARD.decode(ct)?;

    let mut key = [0u8; 32];
    key[..pass.len()].copy_from_slice(pass);

    // println!("ct len: {}", ct.len());
    // println!("iv len: {}", iv.len());
    // println!("{} {:?}", key.len(), key);
    let pt = crypto::decrypt_aes(&key, &iv, &ct)?;

    let out = String::from_utf8(pt)?;

    Ok(out)
}

#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_load_source() {
        let res = extract(&SourceParams {
            id: 655,
            // id: 1399,
            imdb_id: None, //Some("tt18259086".into()),
            // ep: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }
}
