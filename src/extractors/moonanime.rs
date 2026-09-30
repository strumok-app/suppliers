use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use base64::{Engine, prelude::BASE64_STANDARD};
use regex::Regex;

use crate::{models::ContentMediaItemSource, utils};

// moonanime.art responds with 400 to any request (iframe, playlists, subtitles) without Accept-Language
const ACCEPT_LANGUAGE: &str = "uk-UA,uk;q=0.9,en-US;q=0.8,en;q=0.7";

/// Extracts sources from `https://moonanime.art/iframe/{hash}/` player page.
///
/// Page contains `atob("...")` blob with obfuscated PlayerJS config, where
/// `file` and `subtitle` values are wrapped in `_0xd("...")` (base64 + XOR with `var k`).
pub async fn extract(url: &str, description: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let html = utils::create_client()
        .get(url)
        .header("Accept-Language", ACCEPT_LANGUAGE)
        .send()
        .await?
        .text()
        .await?;

    static ATOB_RE: OnceLock<Regex> = OnceLock::new();
    let atob_re = ATOB_RE.get_or_init(|| Regex::new(r#"atob\(\s*["']([^"']+)["']\s*\)"#).unwrap());

    let player_js = atob_re
        .captures_iter(&html)
        .filter_map(|c| outer_decode(c.get(1)?.as_str()))
        .find(|js| js.contains("_0xd("))
        .ok_or_else(|| anyhow!("[moonanime] player config not found"))?;

    static KEY_RE: OnceLock<Regex> = OnceLock::new();
    let key_re = KEY_RE.get_or_init(|| Regex::new(r#"var\s+k\s*=\s*["']([^"']+)["']"#).unwrap());

    let key = key_re
        .captures(&player_js)
        .and_then(|c| c.get(1))
        .ok_or_else(|| anyhow!("[moonanime] xor key not found"))?
        .as_str();

    // page comes in (at least) two templates:
    // `file: _0xd("..."), subtitle: _0xd("...")` or `var rawVideo = _0xd("..."); var rawSubtitle = _0xd("...");`
    static FIELD_RE: OnceLock<Regex> = OnceLock::new();
    let field_re = FIELD_RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(?<name>file|video|subtitle)\s*[:=]\s*_0xd\(\s*["'](?<value>[^"']+)["']\s*\)"#,
        )
        .unwrap()
    });

    let headers = HashMap::from([("Accept-Language".to_string(), ACCEPT_LANGUAGE.to_string())]);
    let mut sources = vec![];

    for captures in field_re.captures_iter(&player_js) {
        let Some(value) = xor_decode(&captures["value"], key) else {
            continue;
        };

        // both fields use PlayerJS list format: "[label]url,[label]url" or plain "url"
        for entry in value.split(',').filter(|e| !e.is_empty()) {
            let (label, link) = match entry.strip_prefix('[').and_then(|e| e.split_once(']')) {
                Some((label, link)) => (Some(label), link),
                None => (None, entry),
            };

            let source = if !captures["name"].eq_ignore_ascii_case("subtitle") {
                ContentMediaItemSource::Video {
                    link: link.to_string(),
                    description: match label {
                        Some(label) => format!("[{label}] {description}"),
                        None => description.to_string(),
                    },
                    headers: Some(headers.clone()),
                    hls_proxy: false,
                }
            } else {
                ContentMediaItemSource::Subtitle {
                    link: link.to_string(),
                    description: label.unwrap_or(description).to_string(),
                    headers: Some(headers.clone()),
                }
            };

            sources.push(source);
        }
    }

    Ok(sources)
}

/// First byte - initial state, next 32 bytes - key, rest - data
fn outer_decode(blob: &str) -> Option<String> {
    let raw = BASE64_STANDARD.decode(blob).ok()?;
    if raw.len() < 33 {
        return None;
    }

    let (key, data) = raw[1..].split_at(32);
    let mut state = raw[0];

    let decoded: Vec<u8> = data
        .iter()
        .enumerate()
        .map(|(i, &d)| {
            let k = key[i % 32];
            let out = d ^ k ^ state;
            state = d.wrapping_add(k);
            out
        })
        .collect();

    Some(String::from_utf8_lossy(&decoded).into_owned())
}

/// JS `_0xd`: base64 decode, then XOR with repeating key
fn xor_decode(encoded: &str, key: &str) -> Option<String> {
    let raw = BASE64_STANDARD.decode(encoded).ok()?;
    let key = key.as_bytes();

    let decoded: Vec<u8> = raw
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect();

    Some(String::from_utf8_lossy(&decoded).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_extract_moonanime() {
        let res = extract(
            "https://moonanime.art/iframe/ldn52ntd86att18wi/?player=animeon.club",
            "Test",
        )
        .await;

        println!("{res:#?}");
    }
}
