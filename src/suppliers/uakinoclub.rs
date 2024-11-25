use anyhow::anyhow;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::Ok;

use super::utils::{self, html, playerjs};
use super::ContentSupplier;
use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType, MediaType,
};
use crate::suppliers::utils::datalife;

const URL: &str = "https://uakino.me";

#[derive(Default)]
pub struct UAKinoClubContentSupplier;

impl ContentSupplier for UAKinoClubContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![
            ContentType::Movie,
            ContentType::Cartoon,
            ContentType::Series,
            ContentType::Anime,
        ]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".into()]
    }

    async fn search(
        &self,
        query: String,
        _types: Vec<String>,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        let result = utils::scrap_page(
            datalife::search_request(URL, &query),
            content_info_items_processor(),
        )
        .await?;

        let filtered_results = result
            .into_iter()
            .filter(|ci| !ci.title.starts_with("news") && !ci.title.starts_with("franchise"))
            .collect();

        Ok(filtered_results)
    }

    async fn load_channel(
        &self,
        channel: String,
        page: u16,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        let url = datalife::get_channel_url(get_channels_map(), &channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: String,
    ) -> Result<Option<ContentDetails>, anyhow::Error> {
        let url = datalife::format_id_from_url(URL, &id);

        utils::scrap_page(
            utils::create_client().get(&url),
            content_details_processor(),
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
        if !params.is_empty() {
            playerjs::load_and_parse_playerjs(&params[0], playerjs::convert_strategy_dub).await
        } else {
            let maybe_news_id = id
                .rsplit_once("/")
                .and_then(|(_, s)| s.split_once("-"))
                .map(|(s, _)| s);

            let news_id = match maybe_news_id {
                Some(news_id) => news_id,
                None => return Err(anyhow!("No news id found")),
            };

            let referer = datalife::format_id_from_url(URL, &id);
            let playlist_req = utils::create_client()
                .get(format!("{URL}/engine/ajax/playlists.php"))
                .query(&[
                    ("xfield", "playlist"),
                    ("news_id", news_id),
                    (
                        "time",
                        Instant::now().elapsed().as_millis().to_string().as_str(),
                    ),
                ])
                .header("Referer", referer);

            datalife::load_ajax_playlist(playlist_req).await
        }
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        mut params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        if params.len() % 2 != 0 {
            return Err(anyhow!("Wrong params size"));
        }

        let mut results = vec![];
        while !params.is_empty() {
            let description = params.remove(0);
            let url = params.remove(0);

            let mut sources = playerjs::load_and_parse_playerjs_sources(description, url).await?;
            results.append(&mut sources);
        }

        Ok(results)
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .in_scope(".movie-title")
            .map_optional(|s| datalife::extract_id_from_url(URL, s))
            .unwrap()
            .into(),
        title: html::TextValue::new()
            .map(html::sanitize_text)
            .in_scope(".movie-title")
            .unwrap()
            .into(),
        secondary_title: html::optional_text_value(".full-quality"),
        image: html::self_hosted_image(URL, ".movie-img > img", "src"),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR.get_or_init(|| {
        html::ItemsProcessor::new("#dle-content .movie-item", content_info_processor())
    })
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#dle-content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value(".solototle"),
                original_title: html::optional_text_value(".origintitle"),
                image: html::self_hosted_image(URL, ".film-poster img", "src"),
                description: html::TextValue::new()
                    .map(html::sanitize_text)
                    .in_scope("div[itemprop=description]")
                    .unwrap()
                    .into(),
                additional_info: html::ItemsProcessor::new(
                    ".film-info > *",
                    html::JoinProcessors::default()
                        .add_processor(html::text_value(".fi-label"))
                        .add_processor(html::text_value(".fi-desc"))
                        .map(|v| v.join(" ").trim().to_owned())
                        .into(),
                )
                .filter(|s| !s.starts_with("Доступно"))
                .into(),
                similar: html::items_processor(
                    ".related-items > .related-item > a",
                    html::ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map(|s| datalife::extract_id_from_url(URL, s))
                            .into(),
                        title: html::text_value(".full-movie-title"),
                        secondary_title: html::default_value::<Option<String>>(),
                        image: html::self_hosted_image(URL, "img", "src"),
                    }
                    .into(),
                ),
                params: html::JoinProcessors::default()
                    .add_processor(html::attr_value(".visible iframe", "src"))
                    .filter(|s| !s.is_empty())
                    .into(),
            }
            .into(),
        )
    })
}

fn get_channels_map() -> &'static HashMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<HashMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        HashMap::from([
            ("Новинки".into(), format!("{URL}/page/")),
            ("Фільми".into(), format!("{URL}/filmy/page/")),
            ("Серіали".into(), format!("{URL}/series/page/")),
            ("Аніме".into(), format!("{URL}/animeukr/page/")),
            ("Мультфільми".into(), format!("{URL}/cartoon/page/")),
            (
                "Мультсеріали".into(),
                format!("{URL}/cartoon/cartoonseries/page/"),
            ),
        ])
    })
}
