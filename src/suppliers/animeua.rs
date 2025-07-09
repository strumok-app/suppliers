use anyhow::anyhow;
use indexmap::IndexMap;
use std::sync::OnceLock;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        html::{self, DOMProcessor},
        playerjs,
    },
};

use crate::utils::{self, datalife};

use super::ContentSupplier;

const URL: &str = "https://animeua.club";

#[derive(Default)]
pub struct AnimeUAContentSupplier;

impl ContentSupplier for AnimeUAContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|&s| s.into()).collect()
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Anime]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".into()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        if page > 1 {
            return Ok(vec![]);
        }

        utils::scrap_page(
            datalife::search_request(URL, query),
            content_info_items_processor(),
        )
        .await
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
        _id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if !params.is_empty() {
            playerjs::load_and_parse_playerjs(&params[0], playerjs::convert_strategy_dub_season_ep)
                .await
        } else {
            Err(anyhow!("iframe url expected"))
        }
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Err(anyhow!("unimplemented"))
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::AttrValue::new("href")
            .map_optional(|s| datalife::extract_id_from_url(URL, s))
            .unwrap_or_default()
            .boxed(),
        title: html::text_value(".poster__desc > .poster__title"),
        secondary_title: html::default_value(),
        image: html::self_hosted_image(URL, ".poster__img img", "data-src"),
    }
    .into()
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::ItemsProcessor::new(".grid-item", content_info_processor()))
}

fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::ScopeProcessor::new(
            "#dle-content",
            html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value(".page__subcol-main > h1"),
                original_title: html::optional_text_value(
                    ".page__subcol-main > .pmovie__original-title",
                ),
                image: html::self_hosted_image(URL, ".pmovie__poster > img", "data-src"),
                description: html::text_value(".page__text"),
                additional_info: html::merge(vec![
                    html::join_processors(vec![
                        html::text_value(".page__subcol-main .pmovie__subrating--site"),
                        html::text_value(".page__subcol-main > .pmovie__year"),
                        html::text_value(".page__subcol-main > .pmovie__genres"),
                    ]),
                    html::items_processor(
                        ".page__subcol-side2 li",
                        html::TextValue::new()
                            .all_nodes()
                            .map(|s| utils::text::sanitize_text(&s))
                            .boxed(),
                    ),
                ]),
                similar: html::items_processor(
                    ".pmovie__related .poster",
                    content_info_processor(),
                ),
                params: html::AttrValue::new("data-src")
                    .in_scope_flatten(".pmovie__player .video-inside iframe")
                    .map_optional(|s| vec![s])
                    .unwrap_or_default()
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
            ("ТОП 100", format!("{URL}/top.html")),
            ("Повнометражки", format!("{URL}/film/page/")),
            ("Аніме серіали", format!("{URL}/anime/page/")),
        ])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_load_channel() {
        let res = AnimeUAContentSupplier
            .load_channel("ТОП 100", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = AnimeUAContentSupplier
            .search("Доктор Стоун", 0)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details() {
        let res = AnimeUAContentSupplier
            .get_content_details("7633-dr-stone-4", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_items() {
        let res = AnimeUAContentSupplier
            .load_media_items(
                "7633-dr-stone-4",
                vec![],
                vec![String::from("https://ashdi.vip/serial/971?season=4")],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
