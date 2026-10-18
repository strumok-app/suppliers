use std::collections::HashMap;

use anyhow::anyhow;
use base64::{Engine, prelude::BASE64_STANDARD};
use futures::future::BoxFuture;
use log::warn;
use rc4::{KeyInit, Rc4, StreamCipher, consts::U32};
use reqwest::Client;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource, suppliers::tmdb::extractors::SourceParams, utils::create_client,
};

// Player: https://player.vidzee.wtf/embed/tv/{id}/{s}/{e}
// Stream logic lives in https://player.vidzee.wtf/assets/Ads-*.js (obfuscator.io),
// decryption in https://player.vidzee.wtf/assets/streams-*.js (embedded AssemblyScript wasm).
//
// wasm `decrypt(data, hostname)`: hostname is only checked against an allow-list,
// the payload is RC4 keyed with a static 32-byte key, first 2048 keystream bytes dropped.
//
// How to extract the key (if the wasm gets rebuilt):
// 1. Find the streams module name in `Ads-*.js`: `await import("./streams-XXXX.js")`
//    (bundle is obfuscator.io-encrypted, search for `wasmDecrypt` / `.c=="string"`).
// 2. In `streams-XXXX.js` base64-decode the `const R="AGFzbQ..."` string into `streams.wasm`.
// 3. Disassemble: `npx -p wabt wasm2wat streams.wasm -o streams.wat`.
// 4. The exported `decrypt` func: hostname check -> call to KSA func -> `i32.const 2048`
//    drop loop -> XOR loop. Drop count is that constant (RC4_DROP).
// 5. In the KSA func (standard RC4 key schedule) find the key byte lookup:
//      local.get 0  i32.const 32  i32.rem_s       ;; i % key_len
//      i32.const 1124  i32.load  i32.add          ;; Uint8Array.dataStart pointer
//      i32.load8_u                                ;; key[i % key_len]
//    The address loaded (1124 here) is the `dataStart` field of the key Uint8Array
//    (layout: buffer@+0, dataStart@+4, byteLength@+8, length@+12).
// 6. Find the `(data (i32.const ...))` segment covering that address and read the
//    pointer (little-endian), here `20 04 00 00` = 1056.
// 7. Find the segment covering 1056: `(data (i32.const 1048) "\01\00\00\00 \00\00\00\e4\f9...")`.
//    First 8 bytes are the ArrayBuffer header (rtId=1, rtSize=32), the next 32 bytes
//    are the key. wat prints printable bytes as ASCII (`}` = 0x7d, `n` = 0x6e, ...).
const RC4_KEY: [u8; 32] = [
    0xe4, 0xf9, 0xb2, 0x7d, 0x8c, 0x1a, 0x6e, 0xf5, 0x03, 0x7d, 0xb9, 0x8a, 0xc5, 0x4e, 0x21, 0xf0,
    0xb9, 0xd6, 0xc3, 0xa7, 0x81, 0xfe, 0x42, 0xad, 0x65, 0xc0, 0xe9, 0xb7, 0x3f, 0x14, 0x8a, 0x2d,
];
const RC4_DROP: usize = 2048;

const CORE_URL: &str = "https://core.vidzee.wtf";
const PLAYER_URL: &str = "https://player.vidzee.wtf";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let client = create_client();
    let media_path = media_path(params);

    let languages = load_languages(client, &media_path)
        .await
        .unwrap_or_else(|err| {
            warn!("[vidzee] failed to load languages: {err}");
            vec![]
        });

    let servers = servers(&languages);

    let servers_itr = servers.iter().map(|(server, label)| {
        let media_path = &media_path;
        async move {
            match load_server(client, media_path, server, label).await {
                Ok(r) => r,
                Err(err) => {
                    warn!("[vidzee] server '{server}' failed: {err}");
                    None
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

fn media_path(params: &SourceParams) -> String {
    match &params.ep {
        Some(ep) => format!("tv/{}/{}/{}", params.id, ep.s, ep.e),
        None => format!("movie/{}", params.id),
    }
}

// Mirrors the player's server list: (server id, label)
fn servers(languages: &[String]) -> Vec<(String, String)> {
    let mut servers = vec![("dcloud".to_string(), "Dcloud".to_string())];

    servers.extend(
        languages
            .iter()
            .map(|lang| (format!("v4:{lang}"), format!("Acme {lang}"))),
    );

    servers.extend([
        ("tik".to_string(), "TCloud".to_string()),
        ("ipcloud".to_string(), "IPcloud".to_string()),
        ("v6:Hindi".to_string(), "Hindi".to_string()),
        ("v6:English".to_string(), "English".to_string()),
    ]);

    servers
}

async fn load_languages(client: &Client, media_path: &str) -> anyhow::Result<Vec<String>> {
    #[derive(Deserialize, Debug)]
    struct LanguagesRes {
        #[serde(default)]
        languages: Vec<String>,
    }

    let res: LanguagesRes = client
        .get(format!("{CORE_URL}/streams/languages/{media_path}"))
        .header("Referer", PLAYER_URL)
        .send()
        .await?
        .json()
        .await?;

    Ok(res
        .languages
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .collect())
}

#[derive(Deserialize, Debug)]
struct StreamRes {
    url: Option<String>,
    language: Option<String>,
    headers: Option<HashMap<String, String>>,
    c: Option<String>,
    error: Option<String>,
}

async fn load_server(
    client: &Client,
    media_path: &str,
    server: &str,
    label: &str,
) -> anyhow::Result<Option<ContentMediaItemSource>> {
    let res = client
        .get(format!("{CORE_URL}/streams/{media_path}"))
        .query(&[("s", server), ("e", "1")])
        .header("Referer", PLAYER_URL)
        .send()
        .await?;

    let status = res.status();
    let body = res.text().await?;

    let mut stream: StreamRes = serde_json::from_str(&body)
        .map_err(|err| anyhow!("unexpected response ({status}): {err}"))?;

    if let Some(err) = stream.error {
        return Err(anyhow!("{err} ({status})"));
    }

    if stream.url.is_none()
        && let Some(c) = &stream.c
    {
        stream = decrypt_stream(c)?;
    }

    let Some(url) = stream.url.filter(|u| !u.is_empty()) else {
        return Ok(None);
    };

    let language = stream.language.unwrap_or_else(|| "Auto".to_string());

    let mut headers = HashMap::from([
        ("Origin".to_string(), PLAYER_URL.to_string()),
        ("Referer".to_string(), format!("{PLAYER_URL}/")),
    ]);
    headers.extend(stream.headers.unwrap_or_default());

    Ok(Some(ContentMediaItemSource::Video {
        link: url,
        description: format!("[Vidzee] {label} - {language}"),
        headers: Some(headers),
        hls_proxy: false,
    }))
}

fn decrypt_stream(c: &str) -> anyhow::Result<StreamRes> {
    let mut data = BASE64_STANDARD.decode(c)?;

    let mut cipher = Rc4::<U32>::new(&RC4_KEY.into());
    let mut drop = [0u8; RC4_DROP];
    cipher.apply_keystream(&mut drop);
    cipher.apply_keystream(&mut data);

    let stream = serde_json::from_slice(&data)?;
    Ok(stream)
}

#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test]
    fn vidzee_should_decrypt_stream() {
        let c = "JHd/O54eLA5vxsCtWd93Lje/bCkNsUmiXpP5HAUxlG1BGMmDoAioYOHugRkUURC3E7cir1z3tnhQEOMJr9uKkeqtQdL7x7u9upXCZre+F4mxTTq5ZajNq0dBaOZzCpAM0TK9jqOGpb/WUbhTefTBqftJuVdcp+NbjCEVKjSYwl0+DLSU+Cjsgy8=";
        let res = decrypt_stream(c).unwrap();

        assert_eq!(res.language.as_deref(), Some("Auto"));
        assert_eq!(
            res.url.as_deref(),
            Some(
                "https://cdn2.fastcloud.live/tv/oAvlGULAN1-C4L4BWBV5yWoiDcd_skGfF8svwR8LpsA.ou9Gqeda/bigtits.m3u8"
            )
        );
    }

    #[test_log::test(tokio::test)]
    async fn vidzee_should_load_source() {
        let res = extract(&SourceParams {
            // id: 655,
            id: 1399,
            imdb_id: None, //Some("tt18259086".into()),
            // ep: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }
}
