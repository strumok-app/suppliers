use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResultItem {
    pub slug: String,
    #[serde(rename = "titleUa")]
    pub title_ua: String,
    pub image: Image,
}

#[derive(Debug, Deserialize)]
pub struct Image {
    pub preview: String,
}

#[derive(Debug, Deserialize)]
pub struct DetailsResponse {
    #[serde(rename = "titleUa")]
    pub title_ua: String,
    #[serde(rename = "titleOriginal")]
    pub title_original: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub raiting: Option<String>,
    pub status: Option<String>,
    pub genres: Vec<Genre>,
    pub studio: Option<Studio>,
    #[serde(rename = "malScored")]
    pub mal_scored: Option<String>,
    pub image: Image,
}

#[derive(Debug, Deserialize)]
pub struct Studio {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Genre {
    #[serde(rename = "nameUa")]
    pub name_ua: String,
}

#[derive(Debug, Deserialize)]
pub struct TranslationsResponse {
    pub translations: Vec<TranslationItem>,
}

#[derive(Debug, Deserialize)]
pub struct TranslationItem {
    pub translation: Translation,
    pub player: Vec<Player>,
}

#[derive(Debug, Deserialize)]
pub struct Translation {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Player {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct EpisodesResponse {
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Deserialize)]
pub struct Episode {
    pub id: u32,
    pub episode: i32,
    pub poster: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VideoResponse {
    #[serde(rename = "videoUrl")]
    pub video_url: Option<String>,
    #[serde(rename = "fileUrl")]
    pub file_url: Option<String>,
}
