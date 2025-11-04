use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    pub data: SearchResponseData,
}

#[derive(Deserialize, Debug)]
pub struct SearchResponseData {
    #[serde(rename = "get_searchComic")]
    pub result: GetSearchComic,
}

#[derive(Deserialize, Debug)]
pub struct GetSearchComic {
    pub items: Vec<SearchComicNode>,
}

#[derive(Deserialize, Debug)]
pub struct SearchComicNode {
    pub data: SearchComicNodeData,
}

#[derive(Deserialize, Debug)]
pub struct SearchComicNodeData {
    pub id: String,
    pub name: String,
    #[serde(rename = "altNames")]
    pub alt_names: Vec<String>,
    #[serde(rename = "urlCoverOri")]
    pub cover_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct DetailsResponse {
    pub data: DetailsResponseData,
}

#[derive(Deserialize, Debug)]
pub struct DetailsResponseData {
    #[serde(rename = "get_comicNode")]
    pub comic_node: ComicNode,
    #[serde(rename = "get_comicChapterList")]
    pub chapter_list: Vec<ChapterNode>,
}

#[derive(Deserialize, Debug)]
pub struct ComicNode {
    pub data: Option<ComicNodeData>,
}

#[derive(Deserialize, Debug)]
pub struct ComicNodeData {
    pub name: String,
    pub score_val: Option<f32>,
    #[serde(rename = "altNames")]
    pub alt_names: Vec<String>,
    pub artists: Vec<String>,
    pub authors: Vec<String>,
    pub genres: Vec<String>,
    #[serde(rename = "originalStatus")]
    pub original_status: String,
    #[serde(rename = "uploadStatus")]
    pub upload_status: Option<String>,
    pub summary: Option<String>,
    #[serde(rename = "urlCoverOri")]
    pub cover_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ChapterNode {
    pub data: ChapterNodeData,
}

#[derive(Deserialize, Debug)]
pub struct ChapterNodeData {
    pub dname: String,
    pub title: Option<String>,
    #[serde(rename = "dupChapters")]
    pub dup_chapters: Vec<ChapterDuplicate>,
}

#[derive(Deserialize, Debug)]
pub struct ChapterDuplicate {
    pub data: ChapterDuplicateData,
}

#[derive(Deserialize, Debug)]
pub struct ChapterDuplicateData {
    pub id: String,
    #[serde(rename = "srcTitle")]
    pub src_title: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PagesResponse {
    pub data: PagesResponseData,
}

#[derive(Deserialize, Debug)]
pub struct PagesResponseData {
    #[serde(rename = "get_chapterNode")]
    pub chapter_node: ChapterNodePages,
}

#[derive(Deserialize, Debug)]
pub struct ChapterNodePages {
    pub data: ChapterNodePagesData,
}

#[derive(Deserialize, Debug)]
pub struct ChapterNodePagesData {
    #[serde(rename = "imageFile")]
    pub image_file: ImageFile,
}

#[derive(Deserialize, Debug)]
pub struct ImageFile {
    #[serde(rename = "urlList")]
    pub url_list: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct LatestResponse {
    pub data: LatestResponseData,
}

#[derive(Deserialize, Debug)]
pub struct LatestResponseData {
    #[serde(rename = "get_latestReleases")]
    pub result: LatestResult,
}

#[derive(Deserialize, Debug)]
pub struct LatestResult {
    pub items: Vec<LatestItem>,
}

#[derive(Deserialize, Debug)]
pub struct LatestItem {
    pub data: LatestItemData,
}

#[derive(Deserialize, Debug)]
pub struct LatestItemData {
    pub id: String,
    pub name: String,
    #[serde(rename = "tranLang")]
    pub tran_lang: String,
    #[serde(rename = "urlCover600")]
    pub cover_url: Option<String>,
    pub score_val: f64,
    #[serde(rename = "last_chapterNodes")]
    pub last_chapters: Vec<LastChapterNode>,
}

#[derive(Deserialize, Debug)]
pub struct LastChapterNode {
    pub data: LastChapterData,
}

#[derive(Deserialize, Debug)]
pub struct LastChapterData {
    pub dname: String,
}
