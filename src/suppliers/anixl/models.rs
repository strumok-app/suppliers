use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    pub data: SearchResponseData,
}

#[derive(Deserialize, Debug)]
pub struct SearchResponseData {
    #[serde(rename = "get_searchAnime")]
    pub get_search_anime: GetSearchAnime,
}

#[derive(Deserialize, Debug)]
pub struct GetSearchAnime {
    pub items: Vec<AnimeNode>,
}

#[derive(Deserialize, Debug)]
pub struct AnimeNode {
    pub data: AnimeNodeData,
}

#[derive(Deserialize, Debug)]
pub struct AnimeNodeData {
    pub ani_id: String,
    pub info_title: String,
    #[serde(rename = "urlCover300")]
    pub url_cover: String,
}
