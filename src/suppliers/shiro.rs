use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock, PoisonError},
    time::{Duration, SystemTime},
};

use anyhow::anyhow;
use indexmap::IndexMap;
use regex::Regex;
use reqwest::{StatusCode, Url, header};
use serde::Deserialize;
use serde_json::json;

use crate::{
    models::{ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType},
    suppliers::ContentSupplier,
    utils::{
        self,
        anilist::{self, AnimeEpisodes, BrowseFilter, StreamingEpisode},
    },
};

const URL: &str = "https://shiro.so";
const WATCH_COOKIE: &str = "shiro_watch";
/// Players keep sending the cookie for every segment during playback, so a
/// cached cookie is only reused while it would outlive a long movie.
const WATCH_COOKIE_MIN_TTL: Duration = Duration::from_secs(3 * 60 * 60);

pub struct ShiroContentSupplier {
    // Filters are built lazily so season-based channels follow the current date.
    channels_map: IndexMap<&'static str, fn() -> BrowseFilter>,
    // No cookie store: the watch cookie has to be read from the response
    // explicitly so it can be forwarded to the player, and a store could keep
    // an older cookie around and suppress `Set-Cookie`.
    client: reqwest::Client,
    // Shared across calls (the supplier lives for the whole app session).
    // Never held across `.await`.
    watch_cookie: Mutex<Option<WatchCookie>>,
}

impl Default for ShiroContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("Trending", BrowseFilter::trending as fn() -> BrowseFilter),
                ("Popular This Season", BrowseFilter::popular_this_season),
                ("Upcoming Next Season", BrowseFilter::upcoming_next_season),
                ("Airing", BrowseFilter::airing),
                ("All Time Popular", BrowseFilter::all_time_popular),
                ("Top Rated", BrowseFilter::top_rated),
            ]),
            client: utils::create_client_builder()
                .cookie_store(false)
                .build()
                .unwrap(),
            watch_cookie: Mutex::new(None),
        }
    }
}

impl ContentSupplier for ShiroContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        self.channels_map.keys().map(|&s| s.to_string()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Anime]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["en".into()]
    }

    // shiro.so has no search backend of its own: it queries AniList directly
    // and identifies titles by AniList id (`/anime/{anilist_id}-{slug}`).
    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        anilist::search_anime(query, page).await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let filter = match self.channels_map.get(channel) {
            Some(build_filter) => build_filter(),
            None => return Err(anyhow!("unknown channel")),
        };

        anilist::browse_anime(&filter, page).await
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        anilist::get_anime(id).await
    }

    async fn load_media_items(
        &self,
        id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let media = anilist::get_anime_episodes(id)
            .await?
            .ok_or_else(|| anyhow!("[shiro] anime {id} not found"))?;

        Ok(Self::aired_episodes(&media))
    }

    /// `id` is the AniList id, `params` are `[episode_number, mal_id]` as
    /// produced by [`Self::aired_episodes`].
    async fn load_media_item_sources(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        let [episode, mal_id] = params.as_slice() else {
            return Err(anyhow!("[shiro] expected [episode, mal_id] params"));
        };

        let anilist_id: u32 = id.parse()?;
        let episode: u32 = episode.parse()?;
        // Titles without a MAL entry: the site sends `null` in that case too.
        let mal_id: Option<u32> = mal_id.parse().ok();

        let (mut cookie, cached) = match self.cached_watch_cookie() {
            Some(cookie) => (cookie, true),
            None => (self.refresh_watch_cookie(anilist_id, episode).await?, false),
        };

        let mut res = self
            .request_episode(&cookie, anilist_id, mal_id, episode)
            .await?;

        // A cached cookie can be revoked server-side before it expires:
        // retry once with a fresh one.
        if cached && res.status() == StatusCode::FORBIDDEN {
            cookie = self.refresh_watch_cookie(anilist_id, episode).await?;
            res = self
                .request_episode(&cookie, anilist_id, mal_id, episode)
                .await?;
        }

        // Errors (403/429/...) still come with a JSON `{status, reason}` body.
        let status = res.status();
        let episode_res: EpisodeResponse = res
            .json()
            .await
            .map_err(|err| anyhow!("[shiro] /api/episode responded {status}: {err}"))?;

        if episode_res.status != "ready" {
            return Err(anyhow!(
                "[shiro] episode {episode} of {anilist_id} is {} ({})",
                episode_res.status,
                episode_res.reason.as_deref().unwrap_or("no reason"),
            ));
        }

        Self::to_media_item_sources(episode_res, &cookie)
    }
}

#[derive(Deserialize, Debug)]
struct EpisodeResponse {
    /// `ready` or `unavailable`.
    status: String,
    reason: Option<String>,
    variants: Option<Vec<EpisodeVariant>>,
}

/// Audio/subtitle flavour of an episode: `sub`, `hsub` (hardsub) or `dub`.
#[derive(Deserialize, Debug)]
struct EpisodeVariant {
    label: String,
    #[serde(default)]
    sources: Vec<EpisodeSource>,
}

/// A single server (named after fruits on the site, e.g. "Plum", "Lemon").
#[derive(Deserialize, Debug)]
struct EpisodeSource {
    label: String,
    /// Site-relative HLS playlist, e.g. `/stream/<token>/index.m3u8?k=...`.
    url: String,
    #[serde(default)]
    tracks: Vec<EpisodeTrack>,
}

/// External subtitle file (`vtt` or `ass`).
#[derive(Deserialize, Debug)]
struct EpisodeTrack {
    label: String,
    src: String,
}

/// `shiro_watch` session cookie, see [`ShiroContentSupplier::refresh_watch_cookie`].
#[derive(Debug)]
struct WatchCookie {
    /// `shiro_watch=<value>`, ready to be sent as a `Cookie` header.
    value: String,
    // Wall-clock time on purpose: `Instant` does not advance while a phone
    // is asleep, which would make an expired cookie look fresh.
    expires_at: SystemTime,
}

impl WatchCookie {
    /// Parses a `Set-Cookie` header value. Returns `None` for other cookies.
    /// Without `Max-Age` the lifetime is unknown, so the cookie is treated as
    /// already expired: it is used for the current request but never reused.
    fn parse(set_cookie: &str, now: SystemTime) -> Option<Self> {
        let mut parts = set_cookie.split(';').map(str::trim);
        let value = parts
            .next()
            .filter(|cookie| cookie.starts_with(&format!("{WATCH_COOKIE}=")))?;

        let max_age = parts
            .filter_map(|attr| attr.split_once('='))
            .find(|(name, _)| name.eq_ignore_ascii_case("max-age"))
            .and_then(|(_, secs)| secs.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or_default();

        Some(Self {
            value: value.to_owned(),
            expires_at: now + max_age,
        })
    }

    fn is_reusable(&self, now: SystemTime) -> bool {
        now + WATCH_COOKIE_MIN_TTL < self.expires_at
    }
}

impl ShiroContentSupplier {
    fn cached_watch_cookie(&self) -> Option<String> {
        self.watch_cookie
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|cookie| cookie.is_reusable(SystemTime::now()))
            .map(|cookie| cookie.value.clone())
    }

    /// Visiting any watch page (`/anime/{id}/{episode}`) issues a `shiro_watch`
    /// session cookie (valid for 24h, not bound to the title or User-Agent).
    /// It is required by `/api/episode` and by every `/stream/...` request.
    ///
    /// The new cookie replaces the cached one. Concurrent callers may both
    /// refresh at the same time; that is harmless as every issued cookie works.
    async fn refresh_watch_cookie(&self, anilist_id: u32, episode: u32) -> anyhow::Result<String> {
        let res = self
            .client
            .head(format!("{URL}/anime/{anilist_id}/{episode}"))
            .send()
            .await?
            .error_for_status()?;

        let now = SystemTime::now();
        let cookie = res
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find_map(|value| WatchCookie::parse(value, now))
            .ok_or_else(|| anyhow!("[shiro] {WATCH_COOKIE} cookie was not issued"))?;

        let value = cookie.value.clone();
        *self
            .watch_cookie
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(cookie);

        Ok(value)
    }

    async fn request_episode(
        &self,
        cookie: &str,
        anilist_id: u32,
        mal_id: Option<u32>,
        episode: u32,
    ) -> anyhow::Result<reqwest::Response> {
        let res = self
            .client
            .post(format!("{URL}/api/episode"))
            .header(header::COOKIE, cookie)
            .header(
                header::REFERER,
                format!("{URL}/anime/{anilist_id}/{episode}"),
            )
            // Without `first`/`prefer` the API returns every variant and server
            // at once; the site only uses them to show the first stream sooner.
            .json(&json!({
                "anilistId": anilist_id,
                "malId": mal_id,
                "episode": episode,
            }))
            .send()
            .await?;

        Ok(res)
    }

    fn to_media_item_sources(
        episode_res: EpisodeResponse,
        cookie: &str,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        let base = Url::parse(URL)?;

        // Playlists, nested playlists, segments and subtitles are all served from
        // `/stream/` and answer 403 without the cookie (or without a User-Agent).
        let headers = HashMap::from([
            ("Cookie".to_owned(), cookie.to_owned()),
            ("User-Agent".to_owned(), utils::get_user_agent().to_owned()),
        ]);

        let mut videos = vec![];
        let mut subtitles = vec![];

        // The player picks the first source by default, so dubs go first.
        // Stable sort: the rest keep the site's order (`sub`, `hsub`).
        let mut variants = episode_res.variants.unwrap_or_default();
        variants.sort_by_key(|variant| !variant.label.eq_ignore_ascii_case("dub"));

        for variant in variants {
            for source in variant.sources {
                let description = format!("[{}] {}", variant.label, source.label);

                for track in source.tracks {
                    subtitles.push(ContentMediaItemSource::Subtitle {
                        link: base.join(&track.src)?.to_string(),
                        description: format!("{description} - {}", track.label),
                        headers: Some(headers.clone()),
                    });
                }

                videos.push(ContentMediaItemSource::Video {
                    link: base.join(&source.url)?.to_string(),
                    description,
                    headers: Some(headers.clone()),
                    // Segments are disguised as `file.jpg` and also need the
                    // cookie, so playback goes through the app's HLS proxy.
                    hls_proxy: true,
                });
            }
        }

        videos.append(&mut subtitles);
        Ok(videos)
    }

    /// Port of shiro.so's `airedEpisodes`: the site has no episode list endpoint
    /// and derives it from AniList metadata instead.
    ///
    /// AniList has no single "episodes aired so far" field, so the count is
    /// inferred from three independent signals and the most optimistic one wins:
    ///
    /// | signal                  | reliable for                         |
    /// |-------------------------|--------------------------------------|
    /// | `nextAiringEpisode - 1` | currently airing shows               |
    /// | `episodes` + `FINISHED` | completed shows (no airing schedule) |
    /// | `streamingEpisodes`     | shows with legal streams (e.g. CR)   |
    ///
    /// The result is then capped by `episodes` (the planned total) when known.
    ///
    /// Every item's params are `[episode_number, mal_id]`, which is what
    /// `POST /api/episode` expects alongside the AniList id.
    fn aired_episodes(media: &AnimeEpisodes) -> Vec<ContentMediaItem> {
        // Extracts the number from streaming titles such as "Episode 12 - Title"
        // or "Ep. 3". Same pattern as the site uses.
        static EPISODE_NUMBER_RE: OnceLock<Regex> = OnceLock::new();
        let episode_number_re = EPISODE_NUMBER_RE
            .get_or_init(|| Regex::new(r"(?i)\b(?:episode|ep\.?)\s*(\d+)").unwrap());

        // Planned episode total. `None` for ongoing shows with an unknown length
        // (e.g. One Piece); `0` is treated the same way.
        let total = media.episodes.filter(|&n| n > 0);

        // Per-episode metadata from official streaming sites. Often missing,
        // incomplete or out of order, so it is only used for titles/thumbnails
        // and as a lower bound for the aired count.
        let streaming = media.streaming_episodes.as_deref().unwrap_or_default();

        // Index streaming entries by episode number. Titles are parsed first since
        // list order can't be trusted; position in the list is the fallback.
        let by_number: HashMap<u32, &StreamingEpisode> = streaming
            .iter()
            .enumerate()
            .map(|(idx, ep)| {
                let number = ep
                    .title
                    .as_deref()
                    .and_then(|title| episode_number_re.captures(title))
                    .and_then(|caps| caps[1].parse().ok())
                    .unwrap_or(idx as u32 + 1);
                (number, ep)
            })
            .collect();

        let max_streaming = by_number.keys().copied().max().unwrap_or(0);

        // Streaming metadata is ignored when it claims more episodes than the
        // title has: it is then inconsistent with AniList's own total and likely
        // describes a different cut/season.
        // With an unknown total there is nothing to check against, so trust it.
        let trust_streaming = match total {
            None => true,
            Some(total) => streaming.len() as u32 <= total && max_streaming <= total,
        };

        // Signal 1: if episode N airs next, episodes 1..N-1 are out.
        // Absent for finished/not-yet-released shows.
        let next_airing = media
            .next_airing_episode
            .as_ref()
            .map(|next| next.episode.saturating_sub(1))
            .unwrap_or(0);

        // Signal 2: a finished show has all of its planned episodes out.
        let finished = if media.status.as_deref() == Some("FINISHED") {
            total.unwrap_or(0)
        } else {
            0
        };

        // Signal 3: the highest episode an official stream exists for.
        let streamed = if trust_streaming { max_streaming } else { 0 };

        // Each signal can be missing (0), so take the largest, then never exceed
        // the planned total. Not-yet-released titles end up with 0 episodes.
        let aired = next_airing.max(finished).max(streamed);
        let count = total.map_or(aired, |total| aired.min(total));

        // Required by shiro's `/api/episode` together with the AniList id.
        // Some titles (mostly recent/obscure) have no MAL entry; the site then
        // sends `null`, so an empty param is passed through as-is.
        let mal_id = media.id_mal.map(|id| id.to_string()).unwrap_or_default();

        // Episodes are always numbered 1..=count; streaming metadata only
        // decorates them and never adds/removes entries.
        (1..=count)
            .map(|number| {
                let streaming = trust_streaming.then(|| by_number.get(&number)).flatten();

                ContentMediaItem {
                    title: streaming
                        .and_then(|ep| ep.title.clone())
                        .filter(|title| !title.is_empty())
                        .unwrap_or_else(|| format!("Episode {number}")),
                    section: None,
                    image: streaming.and_then(|ep| ep.thumbnail.clone()),
                    sources: None,
                    params: vec![number.to_string(), mal_id.clone()],
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn shiro_should_search() {
        let res = ShiroContentSupplier::default().search("one piece", 1).await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn shiro_should_load_channel() {
        let supplier = ShiroContentSupplier::default();
        for channel in supplier.get_channels() {
            let res = supplier.load_channel(&channel, 1).await;
            println!(
                "{channel}: {:?}",
                res.map(|items| items
                    .into_iter()
                    .take(3)
                    .map(|i| i.title)
                    .collect::<Vec<_>>())
            );
        }
    }

    #[test_log::test(tokio::test)]
    async fn shiro_should_get_content_details() {
        let res = ShiroContentSupplier::default()
            .get_content_details("21")
            .await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn shiro_should_load_media_items() {
        let res = ShiroContentSupplier::default()
            .load_media_items("21", vec![])
            .await
            .unwrap();
        println!("count: {}", res.len());
        println!("{:#?}", res.first());
        println!("{:#?}", res.last());
    }

    fn media(
        status: &str,
        episodes: Option<u32>,
        next_airing: Option<u32>,
        streaming_titles: &[&str],
    ) -> AnimeEpisodes {
        let streaming: Vec<_> = streaming_titles
            .iter()
            .map(|title| serde_json::json!({"title": title, "thumbnail": format!("thumb:{title}")}))
            .collect();

        serde_json::from_value(serde_json::json!({
            "id": 1,
            "idMal": 2,
            "status": status,
            "episodes": episodes,
            "nextAiringEpisode": next_airing.map(|episode| serde_json::json!({"episode": episode})),
            "bannerImage": "banner",
            "coverImage": null,
            "streamingEpisodes": streaming,
        }))
        .unwrap()
    }

    #[test]
    fn aired_episodes_should_use_total_for_finished() {
        let items = ShiroContentSupplier::aired_episodes(&media("FINISHED", Some(12), None, &[]));
        assert_eq!(items.len(), 12);
        assert_eq!(items[0].title, "Episode 1");
        assert_eq!(items[0].image.as_deref(), Some("banner"));
        assert_eq!(items[11].params, vec!["12", "2"]);
    }

    #[test]
    fn aired_episodes_should_stop_before_next_airing() {
        let items = ShiroContentSupplier::aired_episodes(&media("RELEASING", None, Some(5), &[]));
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn aired_episodes_should_map_streaming_titles_by_number() {
        let items = ShiroContentSupplier::aired_episodes(&media(
            "RELEASING",
            Some(12),
            Some(3),
            &["Episode 2 - Second", "Episode 1 - First"],
        ));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Episode 1 - First");
        assert_eq!(items[1].image.as_deref(), Some("thumb:Episode 2 - Second"));
    }

    #[test]
    fn aired_episodes_should_ignore_streaming_beyond_total() {
        let items = ShiroContentSupplier::aired_episodes(&media(
            "FINISHED",
            Some(1),
            None,
            &["Episode 1 - A", "Episode 2 - B"],
        ));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Episode 1");
    }

    #[test_log::test(tokio::test)]
    async fn shiro_should_load_media_item_sources() {
        // Frieren: Beyond Journey's End, episode 3
        let res = ShiroContentSupplier::default()
            .load_media_item_sources("154587", vec!["3".into(), "52991".into()])
            .await
            .unwrap();
        println!("{res:#?}");

        // The returned link + headers alone must be enough to play the stream.
        let Some(ContentMediaItemSource::Video { link, headers, .. }) = res.first() else {
            panic!("no video sources");
        };
        let mut req = reqwest::Client::new().get(link);
        for (name, value) in headers.clone().unwrap_or_default() {
            req = req.header(name, value);
        }
        let playlist = req.send().await.unwrap().error_for_status().unwrap();
        assert!(playlist.text().await.unwrap().starts_with("#EXTM3U"));
    }

    #[test]
    fn sources_should_put_dub_first() {
        let variant = |label: &str| EpisodeVariant {
            label: label.to_owned(),
            sources: vec![EpisodeSource {
                label: "Plum".to_owned(),
                url: format!("/stream/{label}/index.m3u8"),
                tracks: vec![EpisodeTrack {
                    label: "English".to_owned(),
                    src: format!("/stream/{label}/en.vtt"),
                }],
            }],
        };
        let episode_res = EpisodeResponse {
            status: "ready".to_owned(),
            reason: None,
            variants: Some(vec![variant("sub"), variant("hsub"), variant("dub")]),
        };

        let sources = ShiroContentSupplier::to_media_item_sources(episode_res, "c=1").unwrap();
        let descriptions: Vec<_> = sources
            .iter()
            .map(|source| match source {
                ContentMediaItemSource::Video { description, .. }
                | ContentMediaItemSource::Subtitle { description, .. } => description.as_str(),
                _ => unreachable!(),
            })
            .collect();

        assert_eq!(
            descriptions,
            [
                "[dub] Plum",
                "[sub] Plum",
                "[hsub] Plum",
                "[dub] Plum - English",
                "[sub] Plum - English",
                "[hsub] Plum - English",
            ]
        );
    }

    #[test_log::test(tokio::test)]
    async fn shiro_should_reject_invalid_params() {
        let res = ShiroContentSupplier::default()
            .load_media_item_sources("154587", vec!["3".into()])
            .await;
        assert!(res.is_err());
    }

    #[test_log::test(tokio::test)]
    async fn shiro_should_reuse_watch_cookie() {
        fn cookie_of(sources: &[ContentMediaItemSource]) -> String {
            match sources.first() {
                Some(ContentMediaItemSource::Video {
                    headers: Some(headers),
                    ..
                }) => headers["Cookie"].clone(),
                other => panic!("unexpected source: {other:?}"),
            }
        }

        let supplier = ShiroContentSupplier::default();
        let first = supplier
            .load_media_item_sources("154587", vec!["1".into(), "52991".into()])
            .await
            .unwrap();
        let second = supplier
            .load_media_item_sources("21", vec!["1".into(), "21".into()])
            .await
            .unwrap();

        assert_eq!(cookie_of(&first), cookie_of(&second));
    }

    const SET_COOKIE: &str = "shiro_watch=abc.1790963102.sig; Path=/; Expires=Fri, 02 Oct 2026 17:45:02 GMT; Max-Age=86400; Secure; HttpOnly; SameSite=lax";

    #[test]
    fn watch_cookie_should_parse_set_cookie() {
        let now = SystemTime::UNIX_EPOCH;
        let cookie = WatchCookie::parse(SET_COOKIE, now).unwrap();
        assert_eq!(cookie.value, "shiro_watch=abc.1790963102.sig");
        assert_eq!(cookie.expires_at, now + Duration::from_secs(86400));

        assert!(WatchCookie::parse("other=1; Max-Age=86400", now).is_none());
    }

    #[test]
    fn watch_cookie_should_not_be_reused_close_to_expiry() {
        let now = SystemTime::UNIX_EPOCH;
        let cookie = WatchCookie::parse(SET_COOKIE, now).unwrap();

        assert!(cookie.is_reusable(now));
        assert!(cookie.is_reusable(now + Duration::from_secs(20 * 60 * 60)));
        assert!(!cookie.is_reusable(now + Duration::from_secs(22 * 60 * 60)));
    }

    #[test]
    fn watch_cookie_without_max_age_should_not_be_reused() {
        let now = SystemTime::UNIX_EPOCH;
        let cookie = WatchCookie::parse("shiro_watch=abc; Path=/", now).unwrap();
        assert!(!cookie.is_reusable(now));
    }
}
