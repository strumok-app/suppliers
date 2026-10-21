mod models;

use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock, PoisonError},
    time::{Duration, SystemTime},
};

use anyhow::anyhow;
use chrono::{Datelike, Utc};
use indexmap::IndexMap;
use regex::Regex;
use reqwest::{StatusCode, Url, header, redirect};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils,
};

use models::{
    CatalogResponse, EpisodeResponse, Media, MediaData, PageData, RecentEpisodes, StreamingEpisode,
    Title,
};

const URL: &str = "https://anv.to";
const PAGE_SIZE: u16 = 20;
const WATCH_COOKIE: &str = "shiro_watch";
/// Players keep sending the cookie for every segment during playback, so a
/// cached cookie is only reused while it would outlive a long movie.
const WATCH_COOKIE_MIN_TTL: Duration = Duration::from_secs(3 * 60 * 60);

enum Channel {
    /// `BrowseAnime` variables without paging. Built lazily so season-based
    /// channels follow the current date.
    Browse(fn() -> Value),
    /// `/api/recent-episodes`: a single, unpaginated feed.
    RecentlyReleased,
}

pub struct AnvContentSupplier {
    channels_map: IndexMap<&'static str, Channel>,
    // No cookie store: the watch cookie has to be read from the response
    // explicitly so it can be forwarded to the player, and a store could keep
    // an older cookie around and suppress `Set-Cookie`.
    // No redirects: watch pages redirect to their canonical slug URL, but the
    // cookie is already set on the redirect itself.
    client: reqwest::Client,
    // Shared across calls (the supplier lives for the whole app session).
    // Never held across `.await`.
    watch_cookie: Mutex<Option<WatchCookie>>,
}

impl Default for AnvContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                (
                    "Trending",
                    Channel::Browse(|| json!({"sort": ["TRENDING_DESC", "POPULARITY_DESC"]})),
                ),
                ("Recently Released", Channel::RecentlyReleased),
                (
                    "Popular This Season",
                    Channel::Browse(|| {
                        let (season, year) = current_season();
                        json!({"sort": ["POPULARITY_DESC"], "season": season, "seasonYear": year})
                    }),
                ),
                (
                    "Airing",
                    Channel::Browse(|| json!({"sort": ["POPULARITY_DESC"], "status": "RELEASING"})),
                ),
                // The catalog only lists titles the site has (or will have)
                // streams for, so a next-season filter is almost always empty.
                (
                    "Upcoming",
                    Channel::Browse(
                        || json!({"sort": ["POPULARITY_DESC"], "status": "NOT_YET_RELEASED"}),
                    ),
                ),
                (
                    "All Time Popular",
                    Channel::Browse(|| json!({"sort": ["POPULARITY_DESC"]})),
                ),
                (
                    "Top Rated",
                    Channel::Browse(|| json!({"sort": ["SCORE_DESC"]})),
                ),
            ]),
            client: utils::create_client_builder()
                .cookie_store(false)
                .redirect(redirect::Policy::none())
                .build()
                .unwrap(),
            watch_cookie: Mutex::new(None),
        }
    }
}

impl ContentSupplier for AnvContentSupplier {
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

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let res: PageData = self
            .catalog(
                "SearchAnime",
                json!({"search": query, "page": page, "perPage": PAGE_SIZE}),
            )
            .await?;

        Ok(res
            .page
            .media
            .into_iter()
            .map(Self::to_content_info)
            .collect())
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let media = match self.channels_map.get(channel) {
            Some(Channel::Browse(build_variables)) => {
                let mut variables = build_variables();
                variables["page"] = json!(page);
                variables["perPage"] = json!(PAGE_SIZE);

                let res: PageData = self.catalog("BrowseAnime", variables).await?;
                res.page.media
            }
            Some(Channel::RecentlyReleased) => self.recently_released(page).await?,
            None => return Err(anyhow!("unknown channel")),
        };

        Ok(media
            .into_iter()
            .filter(|media| media.is_adult != Some(true))
            .map(Self::to_content_info)
            .collect())
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        Ok(self.get_anime(id).await?.map(Self::to_content_details))
    }

    /// Episodes are already part of [`Self::get_content_details`]; this is
    /// only kept as a standalone way to fetch them.
    async fn load_media_items(
        &self,
        id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let media = self
            .get_anime(id)
            .await?
            .ok_or_else(|| anyhow!("[anv] anime {id} not found"))?;

        Ok(Self::aired_episodes(&media))
    }

    /// `id` is the catalog id, `params` are `[episode_number, mal_id]` as
    /// produced by [`Self::aired_episodes`].
    async fn load_media_item_sources(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        let [episode, mal_id] = params.as_slice() else {
            return Err(anyhow!("[anv] expected [episode, mal_id] params"));
        };

        let anime_id: u64 = id.parse()?;
        let episode: u32 = episode.parse()?;
        // Titles without a MAL entry: the site sends `null` in that case too.
        let mal_id: Option<u32> = mal_id.parse().ok();

        let (mut cookie, cached) = match self.cached_watch_cookie() {
            Some(cookie) => (cookie, true),
            None => (self.refresh_watch_cookie(anime_id, episode).await?, false),
        };

        let mut res = self
            .request_episode(&cookie, anime_id, mal_id, episode)
            .await?;

        // A cached cookie can be revoked server-side before it expires:
        // retry once with a fresh one.
        if cached && res.status() == StatusCode::FORBIDDEN {
            cookie = self.refresh_watch_cookie(anime_id, episode).await?;
            res = self
                .request_episode(&cookie, anime_id, mal_id, episode)
                .await?;
        }

        // Errors (403/429/...) still come with a JSON `{status, reason}` body.
        let status = res.status();
        let episode_res: EpisodeResponse = res
            .json()
            .await
            .map_err(|err| anyhow!("[anv] /api/episode responded {status}: {err}"))?;

        if episode_res.status != "ready" {
            return Err(anyhow!(
                "[anv] episode {episode} of {anime_id} is {} ({})",
                episode_res.status,
                episode_res.reason.as_deref().unwrap_or("no reason"),
            ));
        }

        Self::to_media_item_sources(episode_res, &cookie)
    }
}

/// `shiro_watch` session cookie, see [`AnvContentSupplier::refresh_watch_cookie`].
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

/// Current anime broadcast quarter (Jan-Mar = Winter, Apr-Jun = Spring, ...),
/// as the catalog's `season` and `seasonYear` filter values.
fn current_season() -> (&'static str, i32) {
    let now = Utc::now();
    let season = match now.month() {
        1..=3 => "WINTER",
        4..=6 => "SPRING",
        7..=9 => "SUMMER",
        _ => "FALL",
    };
    (season, now.year())
}

impl AnvContentSupplier {
    /// `POST /api/catalog`: the site's proxy over its own AniList-like GraphQL
    /// catalog. It only accepts named operations (the ones the site uses),
    /// not arbitrary queries.
    async fn catalog<T: DeserializeOwned>(
        &self,
        operation: &str,
        variables: Value,
    ) -> anyhow::Result<T> {
        let res = self
            .client
            .post(format!("{URL}/api/catalog"))
            .header(header::ACCEPT, "application/json")
            .json(&json!({"operation": operation, "variables": variables}))
            .send()
            .await?;

        let status = res.status();
        let body: CatalogResponse<T> = res
            .json()
            .await
            .map_err(|err| anyhow!("[anv] {operation} responded {status}: {err}"))?;

        let error = body
            .errors
            .into_iter()
            .flatten()
            .find_map(|error| error.message)
            .or(body.error);
        if let Some(error) = error {
            return Err(anyhow!("[anv] {operation} failed ({status}): {error}"));
        }

        body.data
            .ok_or_else(|| anyhow!("[anv] {operation} responded {status} without data"))
    }

    /// Accepts both catalog ids and legacy AniList ids.
    async fn get_anime(&self, id: &str) -> anyhow::Result<Option<Media>> {
        let id: u64 = id.parse()?;
        let res: MediaData = self.catalog("AnimeDetail", json!({"id": id})).await?;
        Ok(res.media)
    }

    /// Titles of the latest released episodes, newest first.
    async fn recently_released(&self, page: u16) -> anyhow::Result<Vec<Media>> {
        if page > 1 {
            return Ok(vec![]);
        }

        let res: RecentEpisodes = self
            .client
            .get(format!("{URL}/api/recent-episodes"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Several episodes of the same title may have been released recently.
        let mut seen = HashSet::new();
        Ok(res
            .schedules
            .into_iter()
            .map(|schedule| schedule.media)
            .filter(|media| seen.insert(media.id))
            .collect())
    }

    /// Returns `(title, secondary_title)`, preferring the English title like
    /// the site does.
    fn titles(id: u64, title: Title) -> (String, Option<String>) {
        let Title {
            english,
            romaji,
            native,
        } = title;

        match english {
            Some(english) => {
                let original = romaji.filter(|romaji| *romaji != english).or(native);
                (english, original)
            }
            None => (romaji.unwrap_or_else(|| format!("Catalog #{id}")), native),
        }
    }

    fn to_content_info(media: Media) -> ContentInfo {
        let (title, secondary_title) = Self::titles(media.id, media.title);

        ContentInfo {
            id: media.id.to_string(),
            title,
            secondary_title,
            image: media
                .cover_image
                .and_then(|cover| cover.large.or(cover.extra_large))
                .unwrap_or_default(),
        }
    }

    fn to_content_details(media: Media) -> ContentDetails {
        // `AnimeDetail` already carries everything the episode list is derived
        // from, so it is returned right away instead of via `load_media_items`.
        let media_items = Self::aired_episodes(&media);

        let mut additional_info = vec![];

        if let Some(score) = media.average_score {
            additional_info.push(format!("Score: {score}"));
        }

        if let Some(status) = &media.status {
            additional_info.push(format!("Status: {status}"));
        }

        if let Some(format) = &media.format {
            additional_info.push(format!("Format: {format}"));
        }

        match (&media.season, media.season_year) {
            (Some(season), Some(year)) => additional_info.push(format!("Season: {season} {year}")),
            (None, Some(year)) => additional_info.push(format!("Year: {year}")),
            _ => {}
        }

        if let Some(episodes) = media.episodes.filter(|&n| n > 0) {
            additional_info.push(format!("Episodes: {episodes}"));
        }

        if let Some(duration) = media.duration {
            additional_info.push(format!("Duration: {duration} min"));
        }

        let studios: Vec<_> = media
            .studios
            .iter()
            .flat_map(|studios| &studios.nodes)
            .map(|studio| studio.name.as_str())
            .collect();
        if !studios.is_empty() {
            additional_info.push(format!("Studios: {}", studios.join(", ")));
        }

        if let Some(genres) = media.genres.filter(|genres| !genres.is_empty()) {
            additional_info.push(format!("Genres: {}", genres.join(", ")));
        }

        let similar = media
            .recommendations
            .into_iter()
            .flat_map(|recommendations| recommendations.nodes)
            .filter_map(|node| node.media_recommendation)
            .map(Self::to_content_info)
            .collect();

        let (title, original_title) = Self::titles(media.id, media.title);

        ContentDetails {
            title,
            original_title,
            image: media
                .cover_image
                .and_then(|cover| cover.extra_large.or(cover.large))
                .unwrap_or_default(),
            description: utils::text::strip_html(&media.description.unwrap_or_default()),
            additional_info,
            similar,
            media_items: Some(media_items),
            media_type: MediaType::Video,
            params: vec![],
        }
    }

    fn cached_watch_cookie(&self) -> Option<String> {
        self.watch_cookie
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|cookie| cookie.is_reusable(SystemTime::now()))
            .map(|cookie| cookie.value.clone())
    }

    /// Visiting any watch page (`/anime/{id}-episode-{episode}`) issues a
    /// `shiro_watch` session cookie (valid for 24h, not bound to the title or
    /// User-Agent). It is required by `/api/episode` and by every `/stream/...`
    /// request.
    ///
    /// The cookie is set even on the redirect to the canonical slug URL (and
    /// on 404s), so the response status is irrelevant as long as it is there.
    ///
    /// The new cookie replaces the cached one. Concurrent callers may both
    /// refresh at the same time; that is harmless as every issued cookie works.
    async fn refresh_watch_cookie(&self, anime_id: u64, episode: u32) -> anyhow::Result<String> {
        let res = self
            .client
            .head(format!("{URL}/anime/{anime_id}-episode-{episode}"))
            .send()
            .await?;

        let now = SystemTime::now();
        let cookie = res
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .find_map(|value| WatchCookie::parse(value, now))
            .ok_or_else(|| {
                anyhow!(
                    "[anv] {WATCH_COOKIE} cookie was not issued (status {})",
                    res.status()
                )
            })?;

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
        anime_id: u64,
        mal_id: Option<u32>,
        episode: u32,
    ) -> anyhow::Result<reqwest::Response> {
        let res = self
            .client
            .post(format!("{URL}/api/episode"))
            .header(header::COOKIE, cookie)
            .header(
                header::REFERER,
                format!("{URL}/anime/{anime_id}-episode-{episode}"),
            )
            // Without `first`/`prefer` the API returns every variant and server
            // at once; the site only uses them to show the first stream sooner.
            // The field is still called `anilistId`, but takes the catalog id.
            .json(&json!({
                "anilistId": anime_id,
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
        variants.sort_by_key(|variant| !variant.id.eq_ignore_ascii_case("dub"));

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

    /// Port of anv.to's `airedEpisodes`: the site has no episode list endpoint
    /// and derives it from the catalog entry instead.
    ///
    /// `streamingEpisodes` is filled by anv from its own stream sources, so
    /// when present it is authoritative: the highest episode number in it is
    /// the episode count (even if that leaves gaps). Only titles without it
    /// fall back to airing metadata:
    ///
    /// | signal                  | reliable for                         |
    /// |-------------------------|--------------------------------------|
    /// | `nextAiringEpisode - 1` | currently airing shows               |
    /// | `episodes` + `FINISHED` | completed shows (no airing schedule) |
    ///
    /// Every item's params are `[episode_number, mal_id]`, which is what
    /// `POST /api/episode` expects alongside the catalog id.
    fn aired_episodes(media: &Media) -> Vec<ContentMediaItem> {
        // Extracts the number from streaming titles such as
        // "Episode 12 — Title" or "Ep. 3". Same pattern as the site uses.
        static EPISODE_NUMBER_RE: OnceLock<Regex> = OnceLock::new();
        let episode_number_re = EPISODE_NUMBER_RE
            .get_or_init(|| Regex::new(r"(?i)\b(?:episode|ep\.?)\s*(\d+)").unwrap());

        // Index streaming entries by episode number. Titles are parsed first
        // since list order can't be trusted; position in the list is the
        // fallback.
        let by_number: HashMap<u32, &StreamingEpisode> = media
            .streaming_episodes
            .iter()
            .flatten()
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

        let count = if media.streaming_episodes.is_some() {
            by_number.keys().copied().max().unwrap_or(0)
        } else {
            // If episode N airs next, episodes 1..N-1 are out.
            let next_airing = media
                .next_airing_episode
                .as_ref()
                .map_or(0, |next| next.episode.saturating_sub(1));

            // A finished show has all of its planned episodes out.
            let finished = if media.status.as_deref() == Some("FINISHED") {
                media.episodes.unwrap_or(0)
            } else {
                0
            };

            next_airing.max(finished)
        };

        // Required by `/api/episode` together with the catalog id. Some titles
        // (mostly recent/obscure) have no MAL entry; the site then sends
        // `null`, so an empty param is passed through as-is.
        let mal_id = media.id_mal.map(|id| id.to_string()).unwrap_or_default();

        (1..=count)
            .map(|number| {
                let streaming = by_number.get(&number);

                ContentMediaItem {
                    title: streaming
                        .and_then(|ep| ep.title.clone())
                        .filter(|title| !title.is_empty())
                        .unwrap_or_else(|| format!("Episode {number}")),
                    section: None,
                    image: None,
                    sources: None,
                    params: vec![number.to_string(), mal_id.clone()],
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        models::{EpisodeSource, EpisodeTrack, EpisodeVariant},
        *,
    };

    // Frieren: Beyond Journey's End (AniList id 154587, MAL id 52991).
    const FRIEREN: &str = "1000006351";

    #[test_log::test(tokio::test)]
    async fn anv_should_search() {
        let res = AnvContentSupplier::default()
            .search("frieren", 1)
            .await
            .unwrap();
        println!("{res:#?}");
        assert!(res.iter().any(|item| item.id == FRIEREN));
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_load_channel() {
        let supplier = AnvContentSupplier::default();
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
    async fn anv_should_get_content_details() {
        let res = AnvContentSupplier::default()
            .get_content_details(FRIEREN)
            .await
            .unwrap()
            .unwrap();
        println!("{res:#?}");

        let media_items = res.media_items.unwrap();
        assert_eq!(media_items.len(), 28);
        assert_eq!(media_items[0].params, vec!["1", "52991"]);
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_get_content_details_by_legacy_anilist_id() {
        // One Piece, as stored by versions that used AniList ids.
        let res = AnvContentSupplier::default()
            .get_content_details("21")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(res.title, "One Piece");
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_return_none_for_unknown_id() {
        let res = AnvContentSupplier::default()
            .get_content_details("999999999")
            .await
            .unwrap();
        assert!(res.is_none());
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_report_catalog_errors() {
        let res = AnvContentSupplier::default()
            .catalog::<Value>("UnknownOperation", json!({}))
            .await;
        assert!(res.is_err());
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_load_media_items() {
        let res = AnvContentSupplier::default()
            .load_media_items(FRIEREN, vec![])
            .await
            .unwrap();
        println!("count: {}", res.len());
        println!("{:#?}", res.first());
        println!("{:#?}", res.last());
        assert_eq!(res[0].params, vec!["1", "52991"]);
    }

    fn media(
        status: &str,
        episodes: Option<u32>,
        next_airing: Option<u32>,
        streaming_titles: Option<&[&str]>,
    ) -> Media {
        let streaming = streaming_titles.map(|titles| {
            titles
                .iter()
                .map(|title| json!({"title": title, "thumbnail": format!("thumb:{title}")}))
                .collect::<Vec<_>>()
        });

        serde_json::from_value(json!({
            "id": 1,
            "idMal": 2,
            "title": {"english": null, "romaji": "Title", "native": null},
            "status": status,
            "episodes": episodes,
            "nextAiringEpisode": next_airing.map(|episode| json!({"episode": episode})),
            "coverImage": null,
            "streamingEpisodes": streaming,
        }))
        .unwrap()
    }

    #[test]
    fn aired_episodes_should_use_total_for_finished() {
        let items = AnvContentSupplier::aired_episodes(&media("FINISHED", Some(12), None, None));
        assert_eq!(items.len(), 12);
        assert_eq!(items[0].title, "Episode 1");
        assert_eq!(items[0].image, None);
        assert_eq!(items[11].params, vec!["12", "2"]);
    }

    #[test]
    fn aired_episodes_should_stop_before_next_airing() {
        let items = AnvContentSupplier::aired_episodes(&media("RELEASING", None, Some(5), None));
        assert_eq!(items.len(), 4);
    }

    #[test]
    fn aired_episodes_should_map_streaming_titles_by_number() {
        let items = AnvContentSupplier::aired_episodes(&media(
            "RELEASING",
            Some(12),
            Some(3),
            Some(&["Episode 2 — Second", "Episode 1 — First"]),
        ));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Episode 1 — First");
        assert_eq!(items[1].image.as_deref(), Some("thumb:Episode 2 — Second"));
    }

    #[test]
    fn aired_episodes_should_trust_streaming_over_airing_metadata() {
        let items = AnvContentSupplier::aired_episodes(&media(
            "FINISHED",
            Some(12),
            None,
            Some(&["Episode 1 — A", "Episode 3 — C"]),
        ));
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].title, "Episode 2");

        let items =
            AnvContentSupplier::aired_episodes(&media("FINISHED", Some(12), None, Some(&[])));
        assert!(items.is_empty());
    }

    #[test]
    fn titles_should_prefer_english() {
        let title = |english: Option<&str>, romaji: Option<&str>, native: Option<&str>| Title {
            english: english.map(str::to_owned),
            romaji: romaji.map(str::to_owned),
            native: native.map(str::to_owned),
        };

        assert_eq!(
            AnvContentSupplier::titles(1, title(Some("Frieren"), Some("Sousou no Frieren"), None)),
            ("Frieren".to_owned(), Some("Sousou no Frieren".to_owned()))
        );
        assert_eq!(
            AnvContentSupplier::titles(1, title(Some("One Piece"), Some("One Piece"), None)),
            ("One Piece".to_owned(), None)
        );
        assert_eq!(
            AnvContentSupplier::titles(1, title(None, Some("Romaji"), Some("Native"))),
            ("Romaji".to_owned(), Some("Native".to_owned()))
        );
        assert_eq!(
            AnvContentSupplier::titles(7, title(None, None, None)),
            ("Catalog #7".to_owned(), None)
        );
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_load_media_item_sources() {
        let res = AnvContentSupplier::default()
            .load_media_item_sources(FRIEREN, vec!["3".into(), "52991".into()])
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

    #[test_log::test(tokio::test)]
    async fn anv_should_load_media_item_sources_by_legacy_anilist_id() {
        let res = AnvContentSupplier::default()
            .load_media_item_sources("154587", vec!["3".into(), "52991".into()])
            .await
            .unwrap();
        assert!(!res.is_empty());
    }

    #[test]
    fn sources_should_put_dub_first() {
        let variant = |id: &str, label: &str| EpisodeVariant {
            id: id.to_owned(),
            label: label.to_owned(),
            sources: vec![EpisodeSource {
                label: "Plum".to_owned(),
                url: format!("/stream/{id}/index.m3u8"),
                tracks: vec![EpisodeTrack {
                    label: "English".to_owned(),
                    src: format!("/stream/{id}/en.vtt"),
                }],
            }],
        };
        let episode_res = EpisodeResponse {
            status: "ready".to_owned(),
            reason: None,
            variants: Some(vec![
                variant("sub", "Sub"),
                variant("hsub", "Hard Sub"),
                variant("dub", "Dub"),
            ]),
        };

        let sources = AnvContentSupplier::to_media_item_sources(episode_res, "c=1").unwrap();
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
                "[Dub] Plum",
                "[Sub] Plum",
                "[Hard Sub] Plum",
                "[Dub] Plum - English",
                "[Sub] Plum - English",
                "[Hard Sub] Plum - English",
            ]
        );
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_reject_invalid_params() {
        let res = AnvContentSupplier::default()
            .load_media_item_sources(FRIEREN, vec!["3".into()])
            .await;
        assert!(res.is_err());
    }

    #[test_log::test(tokio::test)]
    async fn anv_should_reuse_watch_cookie() {
        fn cookie_of(sources: &[ContentMediaItemSource]) -> String {
            match sources.first() {
                Some(ContentMediaItemSource::Video {
                    headers: Some(headers),
                    ..
                }) => headers["Cookie"].clone(),
                other => panic!("unexpected source: {other:?}"),
            }
        }

        let supplier = AnvContentSupplier::default();
        let first = supplier
            .load_media_item_sources(FRIEREN, vec!["1".into(), "52991".into()])
            .await
            .unwrap();
        // One Piece
        let second = supplier
            .load_media_item_sources("1000001642", vec!["1".into(), "21".into()])
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
