mod extractors;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{self},
};
use anyhow::anyhow;
use extractors::{run_extractors, SourceParams};
use indexmap::IndexMap;
use serde::Deserialize;
use std::sync::OnceLock;

use super::ContentSupplier;

static SECRET: &str = env!("TMDB_SECRET");
const URL: &str = "https://api.themoviedb.org/3";
const IMAGES_URL: &str = "http://image.tmdb.org/t/p";

#[derive(Deserialize)]
struct TMDBContentSupplier;

impl ContentSupplier for TMDBContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![
            ContentType::Movie,
            ContentType::Series,
            ContentType::Cartoon,
            ContentType::Anime,
        ]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["en".into()]
    }

    async fn search(&self, query: String, _types: Vec<String>) -> anyhow::Result<Vec<ContentInfo>> {
        let res: TMDBSearchResponse = utils::create_client()
            .get(format!("{URL}/search/multi"))
            .header("Authorization", format!("Bearer {SECRET}"))
            .query(&[("query", query.as_str()), ("langauge", "en-US")])
            .send()
            .await?
            .json()
            .await?;

        Ok(res
            .results
            .into_iter()
            .filter_map(|r| match r.media_type.as_deref() {
                Some("tv") | Some("movie") => Some(r.to_content_info("")),
                _ => None,
            })
            .collect())
    }

    async fn load_channel(&self, channel: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let (fallback_media_type, path) = match get_channels_map().get(&channel) {
            Some(params) => params,
            None => return Err(anyhow!("Unknow channel")),
        };
        let res: TMDBSearchResponse = utils::create_client()
            .get(format!("{URL}{path}"))
            .header("Authorization", format!("Bearer {SECRET}"))
            .query(&[("page", page.to_string().as_str()), ("langauge", "en-US")])
            .send()
            .await?
            .json()
            .await?;

        Ok(res
            .results
            .into_iter()
            .map(|r| r.to_content_info(fallback_media_type))
            .collect())
    }

    async fn get_content_details(&self, id: String) -> anyhow::Result<Option<ContentDetails>> {
        let res: TMDBDetailsResponse = utils::create_client()
            .get(format!("{URL}/{id}"))
            .header("Authorization", format!("Bearer {SECRET}"))
            .query(&[("append_to_response", "external_ids,credits,recommendations")])
            .send()
            .await?
            .json()
            .await?;

        // println!("{res:#?}");

        let media_items = build_media_items(&id, &res).await?;
        let details = build_content_details(res, media_items);

        Ok(Some(details))
    }

    async fn load_media_items(
        &self,
        _id: String,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        Err(anyhow!("Unimplemented"))
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.is_empty() {
            return Err(anyhow!("Source params expected"));
        }

        let source_params: SourceParams = serde_json::from_str(&params[0])?;
        let sources = run_extractors(&source_params).await;

        Ok(sources)
    }
}

impl SourceParams {
    fn new_movie(res: &TMDBDetailsResponse) -> Self {
        Self {
            id: res.id,
            imdb_id: res.external_ids.imdb_id.as_deref().map(String::from),
            season: None,
            episode: None,
        }
    }

    fn new_episode(res: &TMDBDetailsResponse, ep: &TMDBEpisode) -> Self {
        Self {
            id: res.id,
            imdb_id: res.external_ids.imdb_id.as_deref().map(String::from),
            season: Some(ep.season_number),
            episode: Some(ep.episode_number),
        }
    }
}

#[derive(Deserialize, Debug)]
struct TMDBSearchResponse {
    results: Vec<TMDBSearchResult>,
}

#[derive(Deserialize, Debug)]
struct TMDBSearchResult {
    id: u32,
    name: Option<String>,
    title: Option<String>,
    original_title: Option<String>,
    media_type: Option<String>,
    poster_path: Option<String>,
}

impl TMDBSearchResult {
    fn to_content_info(self: TMDBSearchResult, fallback_media_type: &str) -> ContentInfo {
        let id = self.id;
        let media_type = self.media_type.as_deref().unwrap_or(fallback_media_type);
        let title = self.name.or(self.title).unwrap_or_default();
        let original_title = self.original_title.filter(|s| s != &title);
        let poster = self.poster_path.map(poster_image).unwrap_or_default();

        ContentInfo {
            id: format!("{media_type}/{id}"),
            title,
            secondary_title: original_title,
            image: poster,
        }
    }
}

#[derive(Deserialize, Debug)]
struct TMDBDetailsResponse {
    id: u32,
    name: Option<String>,
    title: Option<String>,
    original_title: Option<String>,
    poster_path: Option<String>,
    overview: String,
    vote_average: f32,
    created_by: Option<Vec<TMDBCreatedBy>>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    last_air_date: Option<String>,
    next_air_date: Option<String>,
    genres: Option<Vec<TMDBGenre>>,
    production_countries: Option<Vec<TMDBCountry>>,
    credits: Option<TMDBCredit>,
    recommendations: TMDBSearchResponse,
    external_ids: TMDBExternalIds,
    seasons: Option<Vec<TMDBSeason>>,
}

#[derive(Deserialize, Debug)]
struct TMDBCreatedBy {
    name: String,
}

#[derive(Deserialize, Debug)]
struct TMDBGenre {
    name: String,
}

#[derive(Deserialize, Debug)]
struct TMDBCountry {
    name: String,
}

#[derive(Deserialize, Debug)]
struct TMDBCredit {
    cast: Vec<TMDBCast>,
}

#[derive(Deserialize, Debug)]
struct TMDBCast {
    name: String,
}

#[derive(Deserialize, Debug)]
struct TMDBExternalIds {
    imdb_id: Option<String>,
}

#[derive(Deserialize, Debug)]
struct TMDBSeason {
    season_number: u32,
}

#[derive(Deserialize, Debug)]
struct TMDBSeasonResponse {
    episodes: Vec<TMDBEpisode>,
}

#[derive(Deserialize, Debug)]
struct TMDBEpisode {
    season_number: u32,
    episode_number: u32,
    name: String,
    still_path: Option<String>,
}

async fn build_media_items(
    id: &str,
    res: &TMDBDetailsResponse,
) -> anyhow::Result<Vec<ContentMediaItem>> {
    match &res.seasons {
        Some(seasons) => {
            let client = &utils::create_client();

            let seasons_res_itr = seasons
                .iter()
                .filter(|s| s.season_number != 0)
                .map(|s| async {
                    let season_number = s.season_number;
                    client
                        .get(format!("{URL}/{id}/season/{season_number}"))
                        .header("Authorization", format!("Bearer {SECRET}"))
                        .send()
                        .await?
                        .json::<TMDBSeasonResponse>()
                        .await
                });

            let media_items: Vec<_> = futures::future::try_join_all(seasons_res_itr)
                .await?
                .into_iter()
                .flat_map(|season| season.episodes)
                .enumerate()
                .map(|(id, episode)| {
                    let media_item_param =
                        serde_json::to_string(&SourceParams::new_episode(res, &episode)).unwrap();
                    ContentMediaItem {
                        number: id as u32,
                        title: episode.name,
                        image: episode.still_path.map(poster_image),
                        section: Some(episode.season_number.to_string()),
                        sources: None,
                        params: vec![media_item_param],
                    }
                })
                .collect();

            Ok(media_items)
        }
        None => {
            let media_item_param = serde_json::to_string(&SourceParams::new_movie(res)).unwrap();
            Ok(vec![ContentMediaItem {
                number: 0,
                title: "".into(),
                section: None,
                image: None,
                sources: None,
                params: vec![media_item_param],
            }])
        }
    }
}

fn build_content_details(
    res: TMDBDetailsResponse,
    media_items: Vec<ContentMediaItem>,
) -> ContentDetails {
    // MediaItemParams::new_movie(&res);

    let title = res.title.or(res.name).unwrap_or_default();
    let original_title = res.original_title.filter(|v| v != &title);
    let image = res
        .poster_path
        .map(original_poster_image)
        .unwrap_or_default();
    let description = res.overview;
    let additional_info: Vec<_> = [
        Some(res.vote_average.to_string()),
        res.created_by
            .map(|v| v.into_iter().map(|i| i.name).collect::<Vec<_>>().join(", "))
            .map(|v| format!("Created by: {v}")),
        res.release_date.map(|v| format!("Release date: {v}")),
        res.first_air_date.map(|v| format!("First air date: {v}")),
        res.last_air_date.map(|v| format!("Last air date: {v}")),
        res.next_air_date.map(|v| format!("Next air date: {v}")),
        res.genres
            .map(|v| v.into_iter().map(|i| i.name).collect::<Vec<_>>().join(", "))
            .map(|v| format!("Genres: {v}")),
        res.production_countries
            .map(|v| v.into_iter().map(|i| i.name).collect::<Vec<_>>().join(", "))
            .map(|v| format!("Country: {v}")),
        res.credits
            .map(|v| v.cast)
            .map(|v| v.into_iter().map(|i| i.name).collect::<Vec<_>>().join(", "))
            .map(|v| format!("Cast: {v}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let similar: Vec<_> = res
        .recommendations
        .results
        .into_iter()
        .map(|v| v.to_content_info(""))
        .collect();

    ContentDetails {
        title,
        original_title,
        description,
        image,
        media_type: MediaType::Video,
        additional_info,
        similar,
        media_items: Some(media_items),
        params: vec![],
    }
}

fn poster_image(path: String) -> String {
    if path.starts_with("/") {
        format!("{IMAGES_URL}/w342{path}")
    } else {
        path
    }
}

fn original_poster_image(path: String) -> String {
    if path.starts_with("/") {
        format!("{IMAGES_URL}/original{path}")
    } else {
        path
    }
}

fn get_channels_map() -> &'static IndexMap<String, (&'static str, &'static str)> {
    static CHANNELS_MAP: OnceLock<IndexMap<String, (&str, &str)>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("Trending".into(), ("", "/trending/all/day")),
            ("Popular Movies".into(), ("movie", "/movie/popular")),
            ("Popular TV Shows".into(), ("movie", "/tv/popular")),
            ("Top Rated Movies".into(), ("movie", "/movie/top_rated")),
            ("Top Rated TV Shows".into(), ("movie", "/tv/top_rated")),
            ("Latest Movies".into(), ("movie", "/movie/latest")),
            ("Latest TV Shows".into(), ("movie", "/movie/latest")),
            ("On The Air TV Shows".into(), ("movie", "/tv/on_the_air")),
        ])
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = TMDBContentSupplier.search("venom".into(), vec![]).await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_channel() {
        let res = TMDBContentSupplier
            .load_channel("Popular Movies".into(), 1)
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test())]
    async fn should_get_movie_content_details() {
        let res = TMDBContentSupplier
            .get_content_details("movie/90228".into())
            .await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test())]
    async fn should_get_tv_content_details() {
        let res = TMDBContentSupplier
            .get_content_details("tv/253".into())
            .await;
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_item_sources() {
        let res = TMDBContentSupplier
            .load_media_item_sources("movie/310131".into(), vec![r#"{"id": 310131}"#.into()])
            .await;
        println!("{res:#?}")
    }
}
