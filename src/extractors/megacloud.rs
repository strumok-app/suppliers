use std::sync::OnceLock;

use anyhow::anyhow;
use base64::{prelude::BASE64_STANDARD, Engine};
use cached::proc_macro::cached;
use regex::Regex;
use serde::Deserialize;

use crate::{
    models::ContentMediaItemSource,
    utils::{
        self, crypto_js,
        jwp_player::{self, JWPConfig},
    },
};

const URL: &str = "https://megacloud.tv";
const SCRIPT_URL: &str = "https://megacloud.tv/js/player/a/prod/e1-player.min.js";

pub async fn extract(url: &str, prefix: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let id = url
        .rsplit_once("?")
        .and_then(|(s, _)| s.rsplit_once("/"))
        .map(|(_, s)| s)
        .ok_or(anyhow!("[megacloud] no id found in link"))?;

    #[derive(Deserialize, Debug)]
    struct GetSourcesResponse {
        sources: serde_json::Value,
        tracks: Vec<jwp_player::Track>,
    }

    //println!("{id}");

    let get_sources_response_str = utils::create_client()
        .get(format!("{URL}/embed-2/ajax/e-1/getSources"))
        .query(&[("id", id)])
        .header("Referer", url)
        .send()
        .await?
        .text()
        .await?;

    //println!("{get_sources_response_str}");

    let get_sources_response: GetSourcesResponse = serde_json::from_str(&get_sources_response_str)?;

    let sources: Vec<jwp_player::Source> = match &get_sources_response.sources {
        serde_json::Value::String(encrypted) => {
            let keys = get_or_load_keys().await?;
            let (password, data) = extract_password_and_data(&keys, encrypted);

            let ct = BASE64_STANDARD.decode(data.as_bytes())?;

            let decyrpted_sources =
                crypto_js::decrypt_aes(password.as_bytes(), &ct[8..16], &ct[16..])?;

            println!("{decyrpted_sources:?}");

            serde_json::from_str(&decyrpted_sources)?
        }
        serde_json::Value::Array(_) => serde_json::from_value(get_sources_response.sources)?,
        _ => return Err(anyhow!("No source found")),
    };

    let jwp_config = JWPConfig {
        sources,
        tracks: get_sources_response.tracks,
    };

    Ok(jwp_config.to_media_item_sources(prefix, None))
}

fn extract_password_and_data(keys: &[(i32, i32)], encrypted: &str) -> (String, String) {
    let mut chars: Vec<_> = encrypted.chars().collect();
    let mut password = String::new();

    let mut cur_index: usize = 0;
    for (a, b) in keys {
        let start: usize = (*a as usize) + cur_index;
        let end: usize = start + (*b as usize);

        (start..end).for_each(|i| {
            let char = chars[i];
            password.push(char);
            chars[i] = '\0';
        });

        cur_index += *b as usize;
    }

    (password, chars.iter().filter(|&ch| *ch != '\0').collect())
}

#[cached(time = 3600, sync_writes = true, result = true)]
async fn get_or_load_keys() -> anyhow::Result<Vec<(i32, i32)>> {
    static KEYS_RE: OnceLock<Regex> = OnceLock::new();
    let keys_re = KEYS_RE.get_or_init(|| {
        Regex::new(r#"case\s*0x[0-9a-f]+:\s*\w+\s*=\s*(\w+)\s*,\s*\w+\s*=\s*(\w+);"#).unwrap()
    });

    fn extract_key(key_name: &str, script: &str) -> Option<i32> {
        let re = Regex::new(format!(",{key_name}=((?:0x)?([0-9a-fa-f]+))").as_str()).unwrap();

        re.captures(script)
            .and_then(|m| Some(m.get(1)?.as_str()))
            .and_then(|s| i32::from_str_radix(&s[2..], 16).ok())
    }

    let script = utils::create_client()
        .get(SCRIPT_URL)
        .send()
        .await?
        .text()
        .await?;

    let res: Vec<_> = keys_re
        .captures_iter(&script)
        .filter_map(|m| Some((m.get(1)?.as_str(), m.get(2)?.as_str())))
        .filter(|(_, s)| !s.starts_with("partKey"))
        .filter_map(|(a, b)| Some((extract_key(a, &script)?, extract_key(b, &script)?)))
        .collect();

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shoud_extract_password_and_data() {
        const KEYS: &[(i32, i32)] = &[
            (54, 6),
            (85, 6),
            (100, 6),
            (101, 6),
            (106, 7),
            (134, 7),
            (156, 6),
        ];
        const ENCRYPTED:&str = "U2FsdGVkX19A5ALyV8svWKkUjszjAf9X0H8EtnLgE++xbOtdodmq0udf4XgJottZ+S8yCPC/xggYMwx03zsQpp29M2Z5TKDwYhe5MM46qZlwLvzjrM25gG9uAz1kNSdrkzMmgaQOkwpQkF5ZeLKq564aV6ahlqv5Hx/yG4yZZniYu1IJdXCR5DZ9x3KT/qvvWWGlRS8kzJGLBnjHJj0f2NzptDHnYy/oDgKWRbCgjsap8eM8/Rk096AXDIoSMKrATsxZrvf4MiTOF6CzPRZQffLn1/KDVLN1PsTkr1BgifI8hmyA+UqSBgH7iFD8ds8OZMLyjqTYrOuTf8NRiY/CYRlPgX2ANC2vPDvXA6gMY1QlRuLJ8aCxFCggNSOrfG/chaLhOCFrd0+VxXqDfUWcxwQec5LtYHKP067N5F4siCLmjh3bs6TS1+x7ZFokFTQylZ0yHvTMD56Ldu0J1TSEOYV73hipy/U74PSrnMAQ8j6r4jdGE1Y53QHNwzwrQGTfUg==";
        const PASSWORD: &str = "df4XgJ5TKDwYrM25gGuAz1kNMmgaQOk4yZZniYGlRS8k";
        const DATA: &str = "U2FsdGVkX19A5ALyV8svWKkUjszjAf9X0H8EtnLgE++xbOtdodmq0uottZ+S8yCPC/xggYMwx03zsQpp29M2Zhe5MM46qZlwLvzj9SdrkzwpQkF5ZeLKq564aV6ahlqv5Hx/yGu1IJdXCR5DZ9x3KT/qvvWWzJGLBnjHJj0f2NzptDHnYy/oDgKWRbCgjsap8eM8/Rk096AXDIoSMKrATsxZrvf4MiTOF6CzPRZQffLn1/KDVLN1PsTkr1BgifI8hmyA+UqSBgH7iFD8ds8OZMLyjqTYrOuTf8NRiY/CYRlPgX2ANC2vPDvXA6gMY1QlRuLJ8aCxFCggNSOrfG/chaLhOCFrd0+VxXqDfUWcxwQec5LtYHKP067N5F4siCLmjh3bs6TS1+x7ZFokFTQylZ0yHvTMD56Ldu0J1TSEOYV73hipy/U74PSrnMAQ8j6r4jdGE1Y53QHNwzwrQGTfUg==";

        let (password, data) = extract_password_and_data(KEYS, ENCRYPTED);

        assert_eq!(PASSWORD, password);
        assert_eq!(DATA, data);
    }

    #[test_log::test(tokio::test())]
    async fn should_load_keys() {
        let keys = get_or_load_keys_no_cache().await.unwrap();
        print!("{keys:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_by_link() {
        let link = "https://megacloud.tv/embed-2/e-1/yFqIrMcvbbGb?k=1";
        let sources = extract(link, "Megacloud").await.unwrap();
        println!("{sources:#?}")
    }
}
