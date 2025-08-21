use core::str;
use std::collections::HashMap;

use base64::{Engine, prelude::BASE64_STANDARD};
use futures::future::BoxFuture;
use pbkdf2::pbkdf2_hmac_array;
use serde::Deserialize;
use sha2::Sha256;

use crate::{
    models::ContentMediaItemSource,
    utils::{create_json_client, crypto},
};

use super::SourceParams;

const URL: &str = "https://player.vidpro.top";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = &params.id;
    let link = match &params.ep {
        Some(ep) => {
            let s = ep.s;
            let e = ep.e;
            format!("{URL}/api/server?id={id}&sr=1&ep={e}&ss={s}")
        }
        None => format!("{URL}/api/server?id={id}&sr=1"),
    };

    let api_client = create_json_client();

    #[derive(Deserialize)]
    struct ApiResponse {
        data: String,
    }

    let api_res: ApiResponse = api_client
        .get(&link)
        .header("Referer", URL)
        .send()
        .await?
        .json()
        .await?;

    let decoded_data = BASE64_STANDARD.decode(api_res.data)?;

    // let test = String::from_utf8(decoded_data);
    // println!("{test:?}");
    //
    #[derive(Deserialize, Debug)]
    struct EncryptedResponse {
        iv: String,
        key: String,
        salt: String,
        iterations: u32,
        #[serde(alias = "encryptedData")]
        encrypted_data: String,
    }

    let crypto_params: EncryptedResponse = serde_json::from_slice(&decoded_data)?;

    // println!("{crypto_params:?}");

    let iv = hex::decode(crypto_params.iv)?;
    let password = crypto_params.key.into_bytes();
    let salt = hex::decode(crypto_params.salt)?;

    // decryptWithPassword(e) {
    //     let t = ep().enc.Hex.parse(e.salt)
    //         , a = ep().enc.Hex.parse(e.iv)
    //         , l = e.encryptedData
    //         , n = ep().PBKDF2(e.key, t, {
    //         keySize: 8,
    //         iterations: e.iterations,
    //         hasher: ep().algo.SHA256
    //     })
    //         , o = ep().AES.decrypt(l, n, {
    //         iv: a,
    //         padding: ep().pad.Pkcs7,
    //         mode: ep().mode.CBC
    //     }).toString(ep().enc.Utf8);
    //     if (!o)
    //         throw Error("Decryption failed: Invalid key or malformed data.");
    //     return JSON.parse(o)
    // }

    let key = pbkdf2_hmac_array::<Sha256, 32>(&password, &salt, crypto_params.iterations);

    let ct = BASE64_STANDARD.decode(crypto_params.encrypted_data)?;
    let decrypted_ct = crypto::decrypt_aes(&key, &iv, &ct)?;

    #[derive(Deserialize, Debug)]
    struct DecryptedResponse {
        url: String,
        headers: HashMap<String, String>,
        #[serde(alias = "hasMultiQuality", default)]
        has_multi_quality: bool,
        #[serde(default)]
        quality: Vec<Quality>,
    }

    #[derive(Deserialize, Debug)]
    struct Quality {
        url: String,
        quality: String,
    }

    let decrypted_response: DecryptedResponse = serde_json::from_slice(&decrypted_ct)?;

    // println!("{decrypted_response:?}");

    let headers = if !decrypted_response.headers.is_empty() {
        Some(decrypted_response.headers)
    } else {
        None
    };

    let resutls = if decrypted_response.has_multi_quality {
        decrypted_response
            .quality
            .into_iter()
            .rev()
            .map(|q| ContentMediaItemSource::Video {
                link: format!("{URL}{}", q.url),
                description: format!("[Vidpro] {}", q.quality),
                headers: headers.clone(),
            })
            .collect()
    } else {
        vec![ContentMediaItemSource::Video {
            link: format!("{URL}{}", decrypted_response.url),
            description: "Vidpro".to_string(),
            headers: headers.clone(),
        }]
    };

    Ok(resutls)
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_extract_tv() {
        let result = extract(&SourceParams {
            id: 655,
            imdb_id: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;

        println!("{result:?}");
    }
}
