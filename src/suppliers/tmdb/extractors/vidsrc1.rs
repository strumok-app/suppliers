use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use futures::future::BoxFuture;
use md5::Digest;
use regex::Regex;
use serde::Deserialize;
use sha2::Sha256;

use crate::{
    models::ContentMediaItemSource,
    suppliers::tmdb::extractors::SourceParams,
    utils::{create_client, crypto},
};

const URL: &str = "https://vidsrc.cc";
const SECRET_PREFIX: &str = "zh&72ciO39tgH5";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let tmdb = params.id;

    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{URL}/v2/embed/tv/{tmdb}/{s}/{e}?autoPlay=false")
        }
        None => format!("{URL}/v2/embed/movie/{tmdb}?autoPlay=false"),
    };

    let client = create_client();
    let iframe_html = client
        .get(link)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;

    // println!("{iframe_html}");

    static IFRAME_RE: OnceLock<Regex> = OnceLock::new();
    let iframe_re =
        IFRAME_RE.get_or_init(|| Regex::new(r#"var\s+(\w+)\s*=\s*(?:"([^"]*)"|(\w+));"#).unwrap());

    let mut variables: HashMap<String, String> = HashMap::new();

    iframe_re.captures_iter(&iframe_html).for_each(|cap| {
        let key = cap.get(1).unwrap().as_str().to_owned();
        let val = cap.get(2).or(cap.get(3)).unwrap().as_str().to_string();

        variables.insert(key, val);
    });

    // println!("{variables:?}");

    let v = variables
        .get("v")
        .ok_or_else(|| anyhow!("[vidsrc_cc] v varaible not found"))?;
    let user_id = variables
        .get("userId")
        .ok_or_else(|| anyhow!("[vidsrc_cc] userId varaible not found"))?;
    let imdb_id = variables
        .get("imdbId")
        .ok_or_else(|| anyhow!("[vidsrc_cc] imdbId varaible not found"))?;
    let movie_id = variables
        .get("movieId")
        .ok_or_else(|| anyhow!("[vidsrc_cc] movieId varaible not found"))?;
    let movie_type = variables
        .get("movieType")
        .ok_or_else(|| anyhow!("[vidsrc_cc] movieType varaible not found"))?;

    let vrf = generate_vrf(movie_id, &format!("{SECRET_PREFIX}_{user_id}"))?;

    // println!("vrf: {vrf}");

    let mut api_link = format!(
        "{URL}/api/{tmdb}/servers?id={tmdb}&v={v}&vrf={vrf}&imdbId={imdb_id}&type={movie_type}"
    );

    if let Some(ep) = &params.ep {
        api_link = format!("{}&season={}&episode={}", api_link, ep.s, ep.e);
    }

    println!("{api_link}");

    #[derive(Deserialize, Debug)]
    struct ApiServer {
        name: String,
        hash: String,
    }

    #[derive(Deserialize, Debug)]
    struct ApiServersResponse {
        data: Vec<ApiServer>,
    }

    let api_res_str = client
        .get(api_link)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;

    println!("{api_res_str}");

    let api_res: ApiServersResponse = serde_json::from_str(&api_res_str)?;

    // println!("{api_res:?}");

    let hash = api_res
        .data
        .into_iter()
        .filter(|srv| srv.name.to_lowercase() == "vidplay")
        .map(|srv| srv.hash)
        .next()
        .ok_or_else(|| anyhow!("[vidsrc_cc] vidplay server not found"))?;

    let source_link = format!("{URL}/api/source/{hash}");

    #[derive(Deserialize, Debug)]
    struct ApiSource {
        r#type: String,
        source: String,
    }

    #[derive(Deserialize, Debug)]
    struct ApiSourceResponse {
        data: ApiSource,
    }

    let source_res_str = client
        .get(source_link)
        .header("Referer", URL)
        .send()
        .await?
        .text()
        .await?;

    // println!("{source_res_str}");

    let source_res: ApiSourceResponse = serde_json::from_str(&source_res_str)?;

    if source_res.data.r#type != "hls" {
        return Err(anyhow!("[vidsrc_cc] No HLS stream found"));
    }

    Ok(vec![ContentMediaItemSource::Video {
        link: source_res.data.source,
        description: "[vidsrc_cc] VidPlay".to_string(),
        headers: Some(HashMap::from([("Referer".to_string(), URL.to_string())])),
    }])
}

// var encrypt_alg = "AES-CBC";
// var hash_alg = "SHA-256";
// var _0x157a1d = ["encrypt"];
// function base64(input) {
//   const output = btoa(input);
//   return output.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
// }
// async function encrypt(text, key) {
//   const textEnc = new TextEncoder();
//   const text_to_enc = textEnc.encode(text);
//   const enc_key = await crypto.subtle.digest(hash_alg, textEnc.encode(key));
//   const opt1 = {
//     name: encrypt_alg
//   };
//   const key_bytes = await crypto.subtle.importKey("raw", enc_key, opt1, false, _0x157a1d);
//   const iv = new Uint8Array(16);
//   const opt2 = {
//     name: encrypt_alg,
//     iv: iv
//   };
//   const result = await crypto.subtle.encrypt(opt2, key_bytes, text_to_enc);
//   return base64(String.fromCharCode(...new Uint8Array(result)));
// }
// await encrypt(movieId, "secret_" + userId);
//
fn generate_vrf(text: &str, pass: &str) -> anyhow::Result<String> {
    let key = Sha256::digest(pass.as_bytes());
    let iv = [0u8; 16];

    let ct = crypto::encrypt_aes(&key, &iv, text.as_bytes())?;

    let out = BASE64_URL_SAFE_NO_PAD.encode(&ct);

    Ok(out)
}

#[cfg(test)]
mod test {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_load_source() {
        let res = extract(&SourceParams {
            // id: 655,
            id: 385687,
            imdb_id: None, //Some("tt18259086".into()),
            ep: None,
            // ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }
}
