use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Title {
    pub native: Option<String>,
    pub romaji: Option<String>,
    pub english: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CoverImage {
    pub large: Option<String>,
    #[serde(alias = "extraLarge")]
    pub extra_large: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SearchMedia {
    pub id: u32,
    pub title: Title,
    #[serde(alias = "coverImage")]
    pub cover_image: Option<CoverImage>,
}

#[derive(Deserialize, Debug)]
pub struct SearchPage {
    pub media: Vec<SearchMedia>,
}

#[derive(Deserialize, Debug)]
pub struct SearchData {
    #[serde(alias = "Page")]
    pub page: SearchPage,
}

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    pub data: SearchData,
}

/// AniList `FuzzyDate`: any part may be unknown, e.g. upcoming titles often
/// only have a year (or nothing at all).
#[derive(Deserialize, Debug)]
pub struct Date {
    pub year: Option<u16>,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

#[derive(Deserialize, Debug)]
pub struct RelationEdge {
    pub node: SearchMedia,
}

#[derive(Deserialize, Debug)]
pub struct Relation {
    pub edges: Vec<RelationEdge>,
}

#[derive(Deserialize, Debug)]
// Every field except `title` is nullable in the AniList schema and is
// actually null for some titles (mostly upcoming ones).
pub struct Media {
    pub title: Title,
    pub status: Option<String>,
    pub description: Option<String>,
    #[serde(alias = "startDate")]
    pub start_date: Option<Date>,
    #[serde(alias = "countryOfOrigin")]
    pub country_of_origin: Option<String>,
    #[serde(alias = "coverImage")]
    pub cover_image: Option<CoverImage>,
    pub genres: Option<Vec<String>>,
    #[serde(alias = "averageScore")]
    pub average_score: Option<u8>,
    pub relations: Option<Relation>,
}

#[derive(Deserialize, Debug)]
pub struct GetAnimeData {
    #[serde(alias = "Media")]
    pub media: Option<Media>,
}

#[derive(Deserialize, Debug)]
pub struct GetAnimeResponse {
    pub data: Option<GetAnimeData>,
}

#[derive(Deserialize, Debug)]
pub struct NextAiringEpisode {
    pub episode: u32,
}

#[derive(Deserialize, Debug)]
pub struct StreamingEpisode {
    pub title: Option<String>,
    pub thumbnail: Option<String>,
}

/// Fields needed to derive the list of aired episodes for a title.
#[derive(Deserialize, Debug)]
pub struct AnimeEpisodes {
    pub id: u32,
    #[serde(alias = "idMal")]
    pub id_mal: Option<u32>,
    pub status: Option<String>,
    pub episodes: Option<u32>,
    #[serde(alias = "nextAiringEpisode")]
    pub next_airing_episode: Option<NextAiringEpisode>,
    #[serde(alias = "bannerImage")]
    pub banner_image: Option<String>,
    #[serde(alias = "coverImage")]
    pub cover_image: Option<CoverImage>,
    #[serde(alias = "streamingEpisodes")]
    pub streaming_episodes: Option<Vec<StreamingEpisode>>,
}

#[derive(Deserialize, Debug)]
pub struct GetAnimeEpisodesData {
    #[serde(alias = "Media")]
    pub media: Option<AnimeEpisodes>,
}

#[derive(Deserialize, Debug)]
pub struct GetAnimeEpisodesResponse {
    pub data: Option<GetAnimeEpisodesData>,
}
