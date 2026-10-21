use std::{collections::HashMap, sync::OnceLock};

use anyhow::anyhow;
use base64::{
    Engine, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use regex::Regex;

use crate::{models::ContentMediaItemSource, utils};

/// Extracts sources from `https://tortuga.tw/vod/{id}` player page.
///
/// Page initializes player with `new TortugaCore({ st, fbu, file, subtitle, ... })`
/// (`/player/js/tor.core.v3.min.js`), where every value is obfuscated (see [`decode`]).
/// Decoded values:
/// - `file` - signed HLS link (`https://calypso.tortuga.tw/t/{token}/{token}/index.m3u8`)
/// - `fbu` - fallback JSON map `{ "{token}": "https://calypso.tortuga.tw/hls/.../index.m3u8" }`
/// - `subtitle` - PlayerJS list format: `[label]url,[label]url` or plain `url`
/// - `st` - session token `v3~{issued}~{expires}~{client ip /24}~...`, used by player to refresh links
pub async fn extract(url: &str, description: &str) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let html = utils::create_client().get(url).send().await?.text().await?;

    static FIELD_RE: OnceLock<Regex> = OnceLock::new();
    let field_re = FIELD_RE.get_or_init(|| {
        Regex::new(r#"\b(?<name>file|fbu|subtitle)\s*:\s*"(?<value>[^"]*)""#).unwrap()
    });

    let fields: HashMap<&str, String> = field_re
        .captures_iter(&html)
        .filter_map(|c| {
            let name = c.name("name")?.as_str();
            let value = decode(c.name("value")?.as_str())?;
            Some((name, value))
        })
        .collect();

    let file = fields
        .get("file")
        .filter(|f| f.starts_with("http") || f.starts_with('['))
        .cloned()
        .or_else(|| fallback_link(fields.get("fbu")?))
        .ok_or_else(|| anyhow!("[tortuga] video link not found"))?;

    let mut sources = vec![];

    for (label, link) in parse_list(&file) {
        sources.push(ContentMediaItemSource::Video {
            link: link.to_string(),
            description: match label {
                Some(label) => format!("{description} [{label}]"),
                None => description.to_string(),
            },
            headers: None,
            hls_proxy: false,
        });
    }

    if let Some(subtitle) = fields.get("subtitle") {
        for (label, link) in parse_list(subtitle) {
            sources.push(ContentMediaItemSource::Subtitle {
                link: link.to_string(),
                description: label.unwrap_or(description).to_string(),
                headers: None,
            });
        }
    }

    Ok(sources)
}

/// JS `TortugaCore` value decoder: base64 (padding is unreliable, e.g. `====`),
/// first byte is a seed (encoder uses `unix seconds % 256`, so value changes on every request),
/// each next byte `i` is XORed with `seed + 7 * i + 13`, result is UTF-8.
fn decode(value: &str) -> Option<String> {
    const BASE64_LENIENT: GeneralPurpose = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::RequireNone)
            .with_decode_allow_trailing_bits(true),
    );

    let raw = BASE64_LENIENT.decode(value.trim_end_matches('=')).ok()?;
    let (&seed, data) = raw.split_first()?;

    let decoded = data
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ (seed as usize + 7 * i + 13) as u8)
        .collect();

    String::from_utf8(decoded).ok()
}

/// `fbu` is JSON map `{ "{token}": "{link}" }`
fn fallback_link(fbu: &str) -> Option<String> {
    serde_json::from_str::<HashMap<String, String>>(fbu)
        .ok()?
        .into_values()
        .find(|link| link.starts_with("http"))
}

/// PlayerJS list format: "[label]url,[label]url" or plain "url"
fn parse_list(value: &str) -> impl Iterator<Item = (Option<&str>, &str)> {
    value
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(
            |entry| match entry.strip_prefix('[').and_then(|e| e.split_once(']')) {
                Some((label, link)) => (Some(label), link),
                None => (None, entry),
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_decode() {
        // seed 250 with extra `==` padding, as produced by TortugaCore encoder
        assert_eq!(
            decode("+m96YWxQEB4XWj4sOSsODF4UEeij+/bSh8bY2aGz/LTTktY===").as_deref(),
            Some("https://example.com/hls/index.m3u8")
        );
    }

    #[test]
    fn should_extract_fallback_link() {
        assert_eq!(
            fallback_link(r#"{"token":"https://calypso.tortuga.tw/hls/index.m3u8"}"#).as_deref(),
            Some("https://calypso.tortuga.tw/hls/index.m3u8")
        );
    }

    #[test_log::test(tokio::test)]
    async fn should_extract_tortuga() {
        let res = extract(
            "https://tortuga.tw/vod/119858",
            "ОЗВУЧЕННЯ AMANOGAWA ПЛЕЄР TRG",
        )
        .await;

        println!("{res:#?}");
        assert!(!res.unwrap().is_empty());
    }
}
