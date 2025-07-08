use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    pub data: SearchResponseData,
}

#[derive(Deserialize, Debug)]
pub struct SearchResponseData {
    #[serde(rename = "result")]
    pub result: GetSearchAnime,
}

#[derive(Deserialize, Debug)]
pub struct GetSearchAnime {
    pub items: Vec<SearchAnimeNode>,
}

#[derive(Deserialize, Debug)]
pub struct SearchAnimeNode {
    pub data: SearchAnimeNodeData,
}

#[derive(Deserialize, Debug)]
pub struct SearchAnimeNodeData {
    pub ani_id: String,
    pub info_title: String,
    #[serde(rename = "urlCover300")]
    pub url_cover: String,
}

#[derive(Deserialize, Debug)]
pub struct DetailsResponse {
    pub data: Option<DetailsResponseData>,
}

#[derive(Deserialize, Debug)]
pub struct DetailsResponseData {
    #[serde(rename = "get_animesNode")]
    pub get_animes_node: DetailsAnimeNode,
}

#[derive(Deserialize, Debug)]
pub struct DetailsAnimeNode {
    pub data: DetailsAnimeNodeData,
}

#[derive(Deserialize, Debug)]
pub struct DetailsAnimeNodeData {
    pub info_title: String,
    #[serde(rename = "urlCoverOri")]
    pub url_cover: String,
    pub info_filmdesc: String,
    pub score_avg: Option<f32>,
    #[serde(rename = "info_meta_dateAiredBegin")]
    pub info_meta_date_aired_begin: Option<String>,
    #[serde(rename = "info_meta_dateAiredEnd")]
    pub info_meta_date_aired_end: Option<String>,
    pub info_meta_genre: Option<Vec<String>>,
    pub info_meta_status: Option<String>,
    pub info_meta_year: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct EpisodesResponse {
    pub data: EpisodesResponseData,
}

#[derive(Deserialize, Debug)]
pub struct EpisodesResponseData {
    #[serde(rename = "get_animesEpisodesList")]
    pub result: AnimeEpisodeList,
}

#[derive(Deserialize, Debug)]
pub struct AnimeEpisodeList {
    pub items: Vec<AnimeEpisodeNode>,
    pub paging: Paging,
}

#[derive(Deserialize, Debug)]
pub struct AnimeEpisodeNode {
    pub data: AnimeEpsiodeNodeData,
}

#[derive(Deserialize, Debug)]
pub struct AnimeEpsiodeNodeData {
    pub ep_title: String,
    #[serde(rename = "sourcesNode_list")]
    pub sources: Vec<AnimeEpisodeSourceNode>,
}

#[derive(Deserialize, Debug)]
pub struct AnimeEpisodeSourceNode {
    pub data: AnimeEpisodeSourceNodeData,
}

#[derive(Deserialize, Debug)]
pub struct AnimeEpisodeSourceNodeData {
    pub src_server: usize,
    pub src_name: String,
    pub src_type: String,
    #[serde(rename = "souPath")]
    pub path: String,
    pub track: Vec<AnimeEpisodeTrack>,
}

#[derive(Deserialize, Debug)]
pub struct AnimeEpisodeTrack {
    #[serde(rename = "trackPath")]
    pub path: String,
    pub label: String,
}

#[derive(Deserialize, Debug)]
pub struct Paging {
    pub pages: usize,
}
