use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Title {
    pub native: Option<String>,
    pub romaji: Option<String>,
    pub english: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct CoverImage {
    pub large: String,
    #[serde(alias = "extraLarge")]
    pub extra_large: String,
}

#[derive(Deserialize, Debug)]
pub struct SearchMedia {
    pub id: u32,
    pub title: Title,
    #[serde(alias = "coverImage")]
    pub cover_image: CoverImage,
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

#[derive(Deserialize, Debug)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
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
pub struct Media {
    pub title: Title,
    pub status: String,
    pub description: String,
    #[serde(alias = "startDate")]
    pub start_date: Date,
    #[serde(alias = "countryOfOrigin")]
    pub country_of_origin: Option<String>,
    #[serde(alias = "coverImage")]
    pub cover_image: CoverImage,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(alias = "averageScore")]
    pub average_score: u8,
    pub relations: Relation,
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
