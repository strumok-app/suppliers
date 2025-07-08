use anyhow::anyhow;
use indexmap::IndexMap;
use std::sync::OnceLock;
use std::time::Instant;

use anyhow::Ok;

use super::ContentSupplier;
use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType, MediaType,
};
use crate::utils::html::{DOMProcessor, ItrDOMProcessor};
use crate::utils::{self, datalife, html, playerjs};

const URL: &str = "https://uakino.best";

#[derive(Default)]
pub struct UAKinoClubContentSupplier;

impl ContentSupplier for UAKinoClubContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.into()).collect()
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

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let result = utils::scrap_page(
            datalife::search_request(URL, query).query(&[("from_page", page.to_string())]),
            content_info_items_processor(),
        )
        .await?;

        let filtered_results = result
            .into_iter()
            .filter(|ci| !ci.id.starts_with("news") && !ci.id.starts_with("franchise"))
            .collect();

        Ok(filtered_results)
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(get_channels_map(), channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            content_info_items_processor(),
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = datalife::format_id_from_url(URL, id);

        utils::scrap_page(
            utils::create_client().get(&url),
            content_details_processor(),
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
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

            let referer = datalife::format_id_from_url(URL, id);
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
        _id: &str,
        _langs: Vec<String>,
        mut params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() % 2 != 0 {
            return Err(anyhow!("Wrong params size"));
        }

        let mut results = vec![];
        while !params.is_empty() {
            let description = params.remove(0);
            let url = params.remove(0);

            let mut sources = playerjs::load_and_parse_playerjs_sources(&description, &url).await?;
            results.append(&mut sources);
        }

        Ok(results)
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .in_scope_flatten(".movie-title")
            .map_optional(|s| datalife::extract_id_from_url(URL, s))
            .unwrap_or_default()
            .boxed(),
        title: html::TextValue::new()
            .map(|s| utils::text::sanitize_text(&s))
            .in_scope(".movie-title")
            .unwrap_or_default()
            .boxed(),
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
                    .map(|s| utils::text::sanitize_text(&s))
                    .in_scope("div[itemprop=description]")
                    .unwrap_or_default()
                    .boxed(),
                additional_info: html::ItemsProcessor::new(
                    ".film-info > *",
                    html::JoinProcessors::default()
                        .add_processor(html::text_value(".fi-label"))
                        .add_processor(html::text_value(".fi-desc"))
                        .map(|v| v.join(" ").trim().to_owned())
                        .boxed(),
                )
                .filter(|s| !s.starts_with("Доступно"))
                .boxed(),
                similar: html::items_processor(
                    ".related-items > .related-item > a",
                    html::ContentInfoProcessor {
                        id: html::AttrValue::new("href")
                            .map_optional(|s| datalife::extract_id_from_url(URL, s))
                            .unwrap_or_default()
                            .boxed(),
                        title: html::text_value(".full-movie-title"),
                        secondary_title: html::default_value(),
                        image: html::self_hosted_image(URL, "img", "src"),
                    }
                    .boxed(),
                ),
                params: html::JoinProcessors::default()
                    .add_processor(html::attr_value(".visible iframe", "src"))
                    .filter(|s| !s.is_empty())
                    .boxed(),
            }
            .boxed(),
        )
    })
}

fn get_channels_map() -> &'static IndexMap<&'static str, String> {
    static CHANNELS_MAP: OnceLock<IndexMap<&'static str, String>> = OnceLock::new();
    CHANNELS_MAP.get_or_init(|| {
        IndexMap::from([
            ("Новинки", format!("{URL}/page/")),
            ("Фільми", format!("{URL}/filmy/page/")),
            ("Серіали", format!("{URL}/seriesss/page/")),
            ("Аніме", format!("{URL}/animeukr/page/")),
            ("Мультфільми", format!("{URL}/cartoon/page/")),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = UAKinoClubContentSupplier
            .load_channel("Новинки", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = UAKinoClubContentSupplier
            .search("Термінатор", 1)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = UAKinoClubContentSupplier
            .get_content_details("filmy/genre_comedy/24898-zhyv-sobi-policeiskyi", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = UAKinoClubContentSupplier
            .load_media_items(
                "filmy/genre_comedy/24898-zhyv-sobi-policeiskyi",
                vec![],
                vec!["https://ashdi.vip/vod/151972".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_for_dle_playlist() {
        let res = UAKinoClubContentSupplier
            .load_media_items(
                "seriesss/drama_series/7312-zoryaniy-kreyser-galaktika-1-sezon",
                vec![],
                vec![],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items_source() {
        let res = UAKinoClubContentSupplier
            .load_media_item_sources(
                "seriesss/drama_series/7312-zoryaniy-kreyser-galaktika-1-sezon",
                vec![],
                vec![
                    "ТакТребаПродакшн (1-2)".into(),
                    "https://ashdi.vip/vod/150511".into(),
                ],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
