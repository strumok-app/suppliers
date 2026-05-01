use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResultItem {
    pub id: u32,
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
pub struct PlayerReponse {
    pub name: String,
    pub json: String,
}
