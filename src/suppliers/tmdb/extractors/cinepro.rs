//! CinePro (OMSS spec) aggregator used by vidplay.to / stream.torrentio.to.
//!
//! `GET /v1/sources/stream` is a Server-Sent Events stream that emits one
//! `partial` event per upstream provider as soon as it resolves, then `done`.
//! The API and its `/server/stream?session=` proxy only check that requests
//! come from the official frontend (Referer or Origin), no cookies or tokens.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use futures::future::BoxFuture;
use log::{debug, warn};
use reqwest::{Url, header};
use serde::Deserialize;

use crate::{models::ContentMediaItemSource, utils};

use super::SourceParams;

const API_URL: &str = "https://s.torrentio.to";
const PROXY_HOST: &str = "s.torrentio.to";
const FRONTEND_URL: &str = "https://stream.vidplay.to";
// All providers usually report within ~15s; anything collected before the
// deadline is still returned.
const STREAM_TIMEOUT: Duration = Duration::from_secs(20);

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    let url = match &params.ep {
        Some(ep) => format!(
            "{API_URL}/v1/sources/stream?type=tv&id={}&s={}&e={}",
            params.id, ep.s, ep.e
        ),
        None => format!("{API_URL}/v1/sources/stream?type=movie&id={}", params.id),
    };

    let mut res = utils::create_client()
        .get(url)
        .header(header::ACCEPT, "text/event-stream")
        .header(header::ORIGIN, FRONTEND_URL)
        .header(header::REFERER, format!("{FRONTEND_URL}/"))
        .timeout(STREAM_TIMEOUT)
        .send()
        .await?
        .error_for_status()?;

    let mut collector = Collector::default();
    let mut buf: Vec<u8> = Vec::new();

    'read: loop {
        let chunk = match res.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(err) => {
                warn!("[cinepro] stream interrupted, keeping partial results: {err}");
                break;
            }
        };
        buf.extend_from_slice(&chunk);

        while let Some(end) = find_event_end(&buf) {
            let raw: Vec<u8> = buf.drain(..end).collect();
            if collector.handle_event(&String::from_utf8_lossy(&raw)) {
                break 'read;
            }
        }
    }

    Ok(collector.into_sources())
}

/// Returns the length of the first complete event (including its blank-line terminator).
fn find_event_end(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n").map(|pos| pos + 2)
}

/// Parses a single SSE event block into `(event name, data)`.
fn parse_event(raw: &str) -> Option<(&str, String)> {
    let mut name = "message";
    let mut data: Option<String> = None;

    for line in raw.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("event:") {
            name = value.trim();
        } else if let Some(value) = line.strip_prefix("data:") {
            let value = value.strip_prefix(' ').unwrap_or(value);
            match &mut data {
                Some(d) => {
                    d.push('\n');
                    d.push_str(value);
                }
                None => data = Some(value.to_owned()),
            }
        }
    }

    data.map(|d| (name, d))
}

#[derive(Default)]
struct Collector {
    videos: Vec<ContentMediaItemSource>,
    subtitles: Vec<ContentMediaItemSource>,
    seen: HashSet<String>,
}

impl Collector {
    /// Handles one SSE event. Returns `true` once the stream is finished.
    fn handle_event(&mut self, raw: &str) -> bool {
        let Some((name, data)) = parse_event(raw) else {
            return false;
        };

        match name {
            "partial" | "done" => match serde_json::from_str::<ProviderResult>(&data) {
                Ok(result) => self.add_result(result),
                Err(err) => warn!("[cinepro] failed to parse '{name}' event: {err}"),
            },
            "error" => warn!("[cinepro] server error: {data}"),
            _ => {}
        }

        name == "done"
    }

    fn add_result(&mut self, result: ProviderResult) {
        for diagnostic in &result.diagnostics {
            debug!("[cinepro] {}", diagnostic.message);
        }

        for source in result.sources {
            if !matches!(source.kind.as_str(), "hls" | "mp4") {
                debug!(
                    "[cinepro] skipping unsupported source type '{}'",
                    source.kind
                );
                continue;
            }

            if !self.seen.insert(source.url.clone()) {
                continue;
            }

            let provider = source
                .provider
                .as_ref()
                .or(result.provider.as_ref())
                .map(Provider::display_name)
                .unwrap_or_else(|| "Unknown".to_owned());

            let mut description = format!("[CinePro] {provider}");
            if let Some(quality) = source.quality.as_deref().filter(|q| !q.is_empty()) {
                description.push_str(&format!(" - {quality}"));
            }
            let audio: Vec<_> = source
                .audio_tracks
                .iter()
                .filter_map(|t| t.label.as_deref())
                .filter(|l| !l.is_empty() && *l != "Original")
                .collect();
            if !audio.is_empty() {
                description.push_str(&format!(" ({})", audio.join(", ")));
            }

            self.videos.push(ContentMediaItemSource::Video {
                headers: proxy_headers(&source.url),
                link: source.url,
                description,
                hls_proxy: false,
            });
        }

        let provider = result
            .provider
            .as_ref()
            .map(Provider::display_name)
            .unwrap_or_else(|| "Unknown".to_owned());

        for subtitle in result.subtitles {
            if !self.seen.insert(subtitle.url.clone()) {
                continue;
            }

            let label = subtitle.label.unwrap_or_else(|| "Unknown".to_owned());
            self.subtitles.push(ContentMediaItemSource::Subtitle {
                headers: proxy_headers(&subtitle.url),
                link: subtitle.url,
                description: format!("[CinePro] {provider} - {label}"),
            });
        }
    }

    fn into_sources(self) -> Vec<ContentMediaItemSource> {
        self.videos.into_iter().chain(self.subtitles).collect()
    }
}

fn proxy_headers(url: &str) -> Option<HashMap<String, String>> {
    let is_proxied = Url::parse(url)
        .ok()
        .is_some_and(|u| u.host_str() == Some(PROXY_HOST));

    is_proxied.then(|| {
        HashMap::from([
            ("Referer".to_owned(), format!("{FRONTEND_URL}/")),
            ("Origin".to_owned(), FRONTEND_URL.to_owned()),
        ])
    })
}

#[derive(Debug, Deserialize)]
struct ProviderResult {
    provider: Option<Provider>,
    #[serde(default)]
    sources: Vec<Source>,
    #[serde(default)]
    subtitles: Vec<Subtitle>,
    #[serde(default)]
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Deserialize)]
struct Provider {
    id: String,
    name: Option<String>,
}

impl Provider {
    /// Provider names are decorated with emojis ("📦 Box"), keep only the text.
    fn display_name(&self) -> String {
        let name = self
            .name
            .as_deref()
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        if name.is_empty() {
            self.id.clone()
        } else {
            name
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    url: String,
    #[serde(rename = "type")]
    kind: String,
    quality: Option<String>,
    #[serde(default)]
    audio_tracks: Vec<AudioTrack>,
    provider: Option<Provider>,
}

#[derive(Debug, Deserialize)]
struct AudioTrack {
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Subtitle {
    url: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    message: String,
}

#[cfg(test)]
mod tests {
    use crate::suppliers::tmdb::extractors::Episode;

    use super::*;

    const SSE_FIXTURE: &str = concat!(
        "event: partial\n",
        r#"data: {"provider":{"id":"vidlink","name":"Chicago"},"sources":[],"subtitles":[]}"#,
        "\n\n",
        "event: partial\n",
        r#"data: {"provider":{"id":"box","name":"📦 Box"},"sources":["#,
        r#"{"url":"https://img1.hscow.com/hls/1/index.m3u8?Expires=1","type":"hls","quality":"1080P","audioTracks":[{"language":"en","label":"English"}],"provider":{"id":"box","name":"📦 Box"}},"#,
        r#"{"url":"https://img1.hscow.com/hls/1/index.m3u8?Expires=1","type":"hls","quality":"720P","provider":{"id":"box","name":"📦 Box"}}"#,
        r#"],"subtitles":[]}"#,
        "\n\n",
        "event: partial\n",
        r#"data: {"provider":{"id":"vidapi","name":"VidApi"},"sources":["#,
        r#"{"url":"https://s.torrentio.to/server/stream?session=a","type":"hls","quality":"720p","audioTracks":[{"label":"Original","language":"Original"}]},"#,
        r#"{"url":"https://s.torrentio.to/server/stream?session=b","type":"mp4","quality":"Auto"},"#,
        r#"{"url":"https://s.torrentio.to/server/stream?session=c","type":"embed"}"#,
        r#"],"subtitles":[{"url":"https://s.torrentio.to/server/stream?session=d","label":"English - SDH","format":"srt"}]}"#,
        "\n\n",
        "event: done\n",
        r#"data: {"sources":[],"subtitles":[],"diagnostics":[{"code":"PROVIDER_ERROR","message":"Chicago: failed"}]}"#,
        "\n\n",
    );

    fn collect(input: &[u8], chunk_size: usize) -> (Vec<ContentMediaItemSource>, bool) {
        let mut collector = Collector::default();
        let mut buf = Vec::new();
        let mut done = false;
        'outer: for chunk in input.chunks(chunk_size) {
            buf.extend_from_slice(chunk);
            while let Some(end) = find_event_end(&buf) {
                let raw: Vec<u8> = buf.drain(..end).collect();
                if collector.handle_event(&String::from_utf8_lossy(&raw)) {
                    done = true;
                    break 'outer;
                }
            }
        }
        (collector.into_sources(), done)
    }

    #[test]
    fn should_parse_sse_stream() {
        // Small chunks split events and multi-byte emojis across reads.
        let (sources, done) = collect(SSE_FIXTURE.as_bytes(), 7);
        assert!(done);
        assert_eq!(sources.len(), 4);

        match &sources[0] {
            ContentMediaItemSource::Video {
                description,
                headers,
                ..
            } => {
                assert_eq!(description, "[CinePro] Box - 1080P (English)");
                assert!(headers.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }

        match &sources[1] {
            ContentMediaItemSource::Video {
                description,
                headers,
                ..
            } => {
                assert_eq!(description, "[CinePro] VidApi - 720p");
                assert_eq!(
                    headers
                        .as_ref()
                        .and_then(|h| h.get("Referer"))
                        .map(String::as_str),
                    Some("https://stream.vidplay.to/")
                );
            }
            other => panic!("unexpected {other:?}"),
        }

        match &sources[2] {
            ContentMediaItemSource::Video { headers, .. } => {
                assert!(headers.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }

        match &sources[3] {
            ContentMediaItemSource::Subtitle {
                description,
                headers,
                ..
            } => {
                assert_eq!(description, "[CinePro] VidApi - English - SDH");
                assert!(headers.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn should_keep_partial_results_without_done() {
        let truncated = SSE_FIXTURE.split("event: done").next().unwrap();
        let (sources, done) = collect(truncated.as_bytes(), 64);
        assert!(!done);
        assert_eq!(sources.len(), 4);
    }

    #[test_log::test(tokio::test)]
    async fn cinepro_should_extract_tv() {
        let res = extract(&SourceParams {
            id: 501,
            imdb_id: None,
            ep: Some(Episode { s: 1, e: 1 }),
        })
        .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn cinepro_should_extract_movie() {
        let res = extract(&SourceParams {
            id: 603,
            imdb_id: None,
            ep: None,
        })
        .await;
        println!("{res:#?}")
    }
}
