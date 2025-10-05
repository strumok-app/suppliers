use std::sync::OnceLock;

use anyhow::Result;
use regex::Regex;

use crate::utils::create_client;

/// Represents an audio group entry from an HLS m3u8 file
#[derive(Debug, Clone)]
pub struct AudioGroup {
    pub name: String,
    pub src: String,
}

/// Extracts audio groups from an HLS m3u8 stream
/// Returns a list of audio group playlists, or a single master stream if no audio groups found
pub async fn extract_audio_groups(url: &str) -> Result<Vec<AudioGroup>> {
    let content = create_client().get(url).send().await?.text().await?;

    // Parse the m3u8 content for audio groups
    let audio_groups = parse_audio_groups(&content, url)?;

    // If no audio groups found, return the master stream
    if audio_groups.is_empty() {
        return Ok(vec![AudioGroup {
            name: "Master Stream".to_string(),
            src: url.to_string(),
        }]);
    }

    Ok(audio_groups)
}

/// Parses m3u8 content to find EXT-X-MEDIA audio group tags
fn parse_audio_groups(content: &str, base_url: &str) -> Result<Vec<AudioGroup>> {
    let mut audio_groups = Vec::new();

    // Regex to match EXT-X-MEDIA audio tags
    // Pattern matches: #EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="group_audio",NAME="Japanese",DEFAULT=YES,LANGUAGE="ja",CHANNELS="2",URI="audio/0_ja/playlist.m3u8"
    static AUDIO_REGEX: OnceLock<Regex> = OnceLock::new();
    let audio_regex = AUDIO_REGEX.get_or_init(|| Regex::new(
        r#"#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="[^"]+",NAME="(?<name>[^"]+)",DEFAULT=(YES|NO),LANGUAGE="([^"]+)",CHANNELS="[^"]+",URI="(?<uri>[^"]+)""#
    ).unwrap());

    for line in content.lines() {
        let line = line.trim();

        if let Some(captures) = audio_regex.captures(line) {
            let name = captures.name("name").unwrap().as_str().to_string();

            let uri = captures.name("uri").unwrap().as_str().to_string();

            // Resolve relative URIs against the base URL
            let resolved_uri = if uri.starts_with("http") {
                uri
            } else {
                resolve_uri(base_url, &uri)
            };

            audio_groups.push(AudioGroup {
                name,
                src: resolved_uri,
            });
        }
    }

    Ok(audio_groups)
}

/// Resolves a relative URI against a base URL
fn resolve_uri(base_url: &str, relative_path: &str) -> String {
    if let Ok(base) = url::Url::parse(base_url)
        && let Ok(resolved) = base.join(relative_path)
    {
        return resolved.to_string();
    }

    // Fallback: simple string concatenation for relative paths
    if base_url.ends_with('/') {
        format!("{}{}", base_url, relative_path)
    } else if let Some(pos) = base_url.rfind('/') {
        format!("{}/{}", &base_url[..pos], relative_path)
    } else {
        format!("{}/{}", base_url, relative_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_audio_groups_with_audio_tags() {
        let m3u8_content = r#"#EXTM3U8
#EXT-X-VERSION:3
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="group_audio",NAME="Japanese",DEFAULT=YES,LANGUAGE="ja",CHANNELS="2",URI="audio/0_ja/playlist.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="group_audio",NAME="English",DEFAULT=NO,LANGUAGE="en",CHANNELS="2",URI="audio/1_en/playlist.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1280000,CODECS="avc1.64001f,mp4a.40.2",RESOLUTION=640x360
stream.m3u8"#;

        let audio_groups =
            parse_audio_groups(m3u8_content, "https://example.com/master.m3u8").unwrap();

        assert_eq!(audio_groups.len(), 2);
        assert_eq!(audio_groups[0].name, "Japanese");
        assert_eq!(audio_groups[1].name, "English");
    }

    #[tokio::test]
    async fn test_extract_audio_groups_no_audio_tags() {
        let m3u8_content = r#"#EXTM3U8
#EXT-X-VERSION:3
#EXT-X-STREAM-INF:BANDWIDTH=1280000,CODECS="avc1.64001f,mp4a.40.2",RESOLUTION=640x360
stream.m3u8"#;

        let audio_groups =
            parse_audio_groups(m3u8_content, "https://example.com/master.m3u8").unwrap();

        assert_eq!(audio_groups.len(), 0);
    }

    #[test]
    fn test_resolve_uri() {
        assert_eq!(
            resolve_uri(
                "https://example.com/master.m3u8",
                "audio/0_ja/playlist.m3u8"
            ),
            "https://example.com/audio/0_ja/playlist.m3u8"
        );

        assert_eq!(
            resolve_uri(
                "https://example.com/path/master.m3u8",
                "audio/0_ja/playlist.m3u8"
            ),
            "https://example.com/path/audio/0_ja/playlist.m3u8"
        );

        assert_eq!(
            resolve_uri(
                "https://example.com/master.m3u8",
                "https://other.com/audio.m3u8"
            ),
            "https://other.com/audio.m3u8"
        );
    }
}
