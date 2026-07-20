use std::collections::HashMap;

use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use futures::future::BoxFuture;
use log::warn;
use reqwest::Client;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    suppliers::tmdb::URL,
    utils::{create_json_client, crypto},
};

use super::SourceParams;

const SITE_URL: &str = "https://vidrock.ru";
const BACKEND_URL: &str = "https://vidrock.ru/api";

const ENC_KEY: &str = "7f3e9c2a8b5d1f4e6a9c3b7d2e5f8a1c4b6d9e2f5a8c1b4d7e9f2a5c8b1d4e7f";

#[derive(Debug, Deserialize)]
struct ServerSource {
    url: Option<String>,
    language: Option<String>,
}

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let link = match &params.ep {
        Some(ep) => format!("{}/tv/{}/{}/{}", BACKEND_URL, params.id, ep.s, ep.e),
        None => format!("{}/movie/{}", BACKEND_URL, params.id),
    };

    let client = create_json_client();
    // println!("{link}");
    let res_str = client
        .get(link)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;
    // println!("{res_str}");

    let res: HashMap<String, ServerSource> = serde_json::from_str(&res_str)?;

    let mut result: Vec<ContentMediaItemSource> = vec![];

    for (name, source) in res.iter() {
        let url = match &source.url {
            Some(url) => url,
            None => continue,
        };

        let language = source.language.as_ref().map_or("unknown", |s| s.as_str());

        match decrypt_url(url) {
            Ok(decrypted_url) => {
                if decrypted_url.contains("/playlist") {
                    let source_name = format!("{name}. {language}");
                    match load_playlist(client, &decrypted_url, &source_name).await {
                        Ok(mut vidstore) => result.append(&mut vidstore),
                        Err(e) => warn!("[vidrocks] fail to load source {decrypted_url}: {e}"),
                    }
                    continue;
                }

                // if lang::is_allowed(language) {
                result.push(ContentMediaItemSource::Video {
                    link: decrypted_url.to_owned(),
                    description: format!("[Vidrocks] {name}. {language}"),
                    headers: Some(HashMap::from([
                        ("Referer".to_owned(), SITE_URL.to_owned()),
                        ("Origin".to_owned(), SITE_URL.to_owned()),
                    ])),
                    hls_proxy: true,
                })
            }
            Err(err) => warn!("[vidrock] server {source:?} decryot utl failed: {err}"),
        }
    }

    Ok(result)
}

async fn load_playlist(
    client: &Client,
    url: &str,
    source_name: &str,
) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    #[derive(Deserialize, Debug)]
    struct PlaylistItem {
        resolution: u16,
        url: String,
    }

    let res_str = client
        .get(url)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;

    // println!("{res_str}");

    let playlist: Vec<PlaylistItem> = serde_json::from_str(&res_str)?;

    // println!("{playlist:?}");

    let items: Vec<_> = playlist
        .into_iter()
        .rev()
        .map(|item| ContentMediaItemSource::Video {
            link: item.url,
            description: format!("[Vidrocks] 1. {} - {}", source_name, item.resolution),
            headers: Some(HashMap::from([("Referer".to_owned(), SITE_URL.to_owned())])),
            hls_proxy: true,
        })
        .collect();

    Ok(items)
}

// xQ = "7f3e9c2a8b5d1f4e6a9c3b7d2e5f8a1c4b6d9e2f5a8c1b4d7e9f2a5c8b1d4e7f";
// function bQ(r) {
//     const e = new Uint8Array(r.length / 2);
//     for (let t = 0; t < e.length; t++)
//         e[t] = parseInt(r.substr(t * 2, 2), 16);
//     return e
// }
// function wQ(r) {
//     let e = r.replace(/-/g, "+").replace(/_/g, "/");
//     const t = e.length % 4;
//     if (t === 2)
//         e += "==";
//     else if (t === 3)
//         e += "=";
//     else if (t === 1)
//         throw new Error("Invalid base64url length");
//     const n = atob(e)
//       , s = new Uint8Array(n.length);
//     for (let i = 0; i < n.length; i++)
//         s[i] = n.charCodeAt(i);
//     return s
// }
// let _u = null;
// async function EQ() {
//     if (_u)
//         return _u;
//     const r = bQ(xQ);
//     return _u = await crypto.subtle.importKey("raw", r.buffer.slice(r.byteOffset, r.byteOffset + r.byteLength), {
//         name: "AES-GCM"
//     }, !1, ["decrypt"]),
//     _u
// }
// async function SQ(r) {
//     const e = wQ(r);
//     if (e.length < 28)
//         throw new Error("Ciphertext too short");
//     const t = e.slice(0, 12)
//       , n = e.slice(12)
//       , s = await EQ()
//       , i = t.buffer.slice(t.byteOffset, t.byteOffset + t.byteLength)
//       , a = n.buffer.slice(n.byteOffset, n.byteOffset + n.byteLength)
//       , o = await crypto.subtle.decrypt({
//         name: "AES-GCM",
//         iv: i
//     }, s, a);
//     return new TextDecoder().decode(o)
// }

fn decrypt_url(url: &str) -> anyhow::Result<String> {
    let url_bytes = BASE64_URL_SAFE_NO_PAD.decode(url)?;

    let iv = &url_bytes[..12];
    let ct = &url_bytes[12..];
    let key = hex::decode(ENC_KEY)?;

    let decrypted_url_bytes = crypto::decrypt_aes_gcm(&key, iv, ct)?;
    let decrypted_url = String::from_utf8(decrypted_url_bytes)?;

    Ok(decrypted_url)
}

#[cfg(test)]
mod tests {
    use crate::{suppliers::tmdb::extractors::Episode, utils};

    #[test]
    fn vidrock_decrypt_url1() {
        let res = decrypt_url(
            "WhRfCLIBMUSrMC_QD_URsp3Kyk_YErof5UXEAspf13xG_rC0aSsQd6kJA6JPQFTUtzcy9zmIoksXSIO4HraSuOPYyjPyxyiGRz2A96P9DRQZsShf9tgGVpi9EXg5qrmEKLeehOUNOBcwbn3bIyIBKDo2jk7mKSMbvqdANPUE1YizXqS8clmMm5ZWelUU3IbNUFOq5iVvUr_8RwKV",
        );

        println!("{res:#?}")
    }

    #[test]
    fn vidrock_decrypt_url2() {
        let res = decrypt_url(
            "NZ75OJ2xASa5XUOgX31RrCciDAZu8-zQ3bHPEnLKfaas5pBzUZW3r0EjQ9toNF4n1eLw11mQaWyG0eRhX-ub_CWrWYuCsRSFQLp9IFZNrxKVowA",
        );

        println!("{res:#?}")
    }

    use super::*;
    #[test_log::test(tokio::test)]
    async fn vidrock_should_extract_movies() {
        let res = extract(&SourceParams {
            id: 533535,
            imdb_id: None,
            ep: None,
        })
        .await;

        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn vidrock_should_extract_tv() {
        let res = extract(&SourceParams {
            id: 655,
            imdb_id: None,
            ep: Some(Episode { e: 1, s: 1 }),
        })
        .await;

        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn vidrock_playlist_load() {
        let res = load_playlist(
            utils::create_json_client(),
            "https://streamrk.site/playlist/78f2bdd1999acb93fdfc912b",
            "en",
        )
        .await;
        println!("{res:#?}")
    }
}
