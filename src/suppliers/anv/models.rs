use serde::Deserialize;

/// `POST /api/catalog` envelope. GraphQL-style errors come in `errors`, while
/// requests rejected by the proxy itself (unknown operation, bad variables)
/// get a plain `{"error": "..."}` body.
#[derive(Deserialize, Debug)]
pub struct CatalogResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<CatalogError>>,
    pub error: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CatalogError {
    pub message: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PageData {
    #[serde(rename = "Page")]
    pub page: Page,
}

#[derive(Deserialize, Debug)]
pub struct Page {
    pub media: Vec<Media>,
}

#[derive(Deserialize, Debug)]
pub struct MediaData {
    #[serde(rename = "Media")]
    pub media: Option<Media>,
}

#[derive(Deserialize, Debug)]
pub struct Title {
    pub english: Option<String>,
    pub romaji: Option<String>,
    pub native: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CoverImage {
    pub large: Option<String>,
    pub extra_large: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct NextAiringEpisode {
    pub episode: u32,
}

#[derive(Deserialize, Debug)]
pub struct Studios {
    pub nodes: Vec<Studio>,
}

#[derive(Deserialize, Debug)]
pub struct Studio {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct StreamingEpisode {
    pub title: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Recommendations {
    pub nodes: Vec<Recommendation>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub media_recommendation: Option<Media>,
}

/// anv's catalog entry. AniList-shaped, but `id` is anv's own id (e.g.
/// `1000006351`); legacy AniList ids are still accepted by every endpoint.
/// Nearly every field may be `null`.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub id: u64,
    pub title: Title,
    pub description: Option<String>,
    pub cover_image: Option<CoverImage>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub episodes: Option<u32>,
    pub next_airing_episode: Option<NextAiringEpisode>,
    pub season: Option<String>,
    pub season_year: Option<u16>,
    pub genres: Option<Vec<String>>,
    pub average_score: Option<u8>,
    pub duration: Option<u32>,
    pub studios: Option<Studios>,
    pub id_mal: Option<u32>,
    pub is_adult: Option<bool>,
    /// Only requested by `AnimeDetail`.
    pub streaming_episodes: Option<Vec<StreamingEpisode>>,
    /// Only requested by `AnimeDetail`.
    pub recommendations: Option<Recommendations>,
}

/// `GET /api/recent-episodes`: latest released episodes, newest first.
#[derive(Deserialize, Debug)]
pub struct RecentEpisodes {
    pub schedules: Vec<RecentEpisode>,
}

#[derive(Deserialize, Debug)]
pub struct RecentEpisode {
    pub media: Media,
}

#[derive(Deserialize, Debug)]
pub struct EpisodeResponse {
    /// `ready` or `unavailable`.
    pub status: String,
    pub reason: Option<String>,
    pub variants: Option<Vec<EpisodeVariant>>,
}

/// Audio/subtitle flavour of an episode.
#[derive(Deserialize, Debug)]
pub struct EpisodeVariant {
    /// `sub`, `hsub` (hardsub) or `dub`.
    pub id: String,
    /// Display name, e.g. `Sub`.
    pub label: String,
    #[serde(default)]
    pub sources: Vec<EpisodeSource>,
}

/// A single server (named after fruits on the site, e.g. "Orange", "Pear").
#[derive(Deserialize, Debug)]
pub struct EpisodeSource {
    pub label: String,
    /// Site-relative HLS playlist, e.g. `/stream/<token>/index.m3u8?k=...`.
    pub url: String,
    #[serde(default)]
    pub tracks: Vec<EpisodeTrack>,
}

/// External subtitle file (`vtt` or `ass`).
#[derive(Deserialize, Debug)]
pub struct EpisodeTrack {
    pub label: String,
    pub src: String,
}
