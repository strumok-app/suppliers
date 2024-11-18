use std::{collections::HashMap, sync::OnceLock};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::utils::html,
};

use super::{
    utils::{self, datalife},
    ContentSupplier,
};

const URL: &str = "https://animeua.club";

pub struct AnimeUAContentSupplier;

impl Default for AnimeUAContentSupplier {
    fn default() -> Self {
        Self {}
    }
}

impl ContentSupplier for AnimeUAContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        get_channels_map().keys().map(|s| s.into()).collect()
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

    async fn search(
        &self,
        query: String,
        _types: Vec<String>,
    ) -> Result<Vec<ContentInfo>, anyhow::Error> {
        utils::scrap_page(
            datalife::search_request(URL, &query),
            content_info_items_processor(),
        )
        .await
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
        _id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItem>, anyhow::Error> {
        utils::playerjs::load_and_parse_playerjs(&params[0]).await
    }

    async fn load_media_item_sources(
        &self,
        _id: String,
        _params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error> {
        unimplemented!()
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    Box::new(html::ContentInfoProcessor {
        id: html::extract_value(|el| {
            el.attr("href")
                .map(|s| datalife::extract_id_from_url(URL, s.into()))
                .unwrap_or_default()
        }),
        title: html::text_value(".poster__desc > .poster__title"),
        secondary_title: html::default_value::<Option<String>>(),
        image: html::map_value(
            |a| format!("{URL}{a}"),
            html::attr_value("data-src", ".poster__img img"),
        ),
    })
}

fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
    static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
        OnceLock::new();
    CONTENT_INFO_ITEMS_PROCESSOR
        .get_or_init(|| html::items_processor_raw(".grid-item", content_info_processor()))
}

// SelectorsToMap(
//     {
//       "id": Const(id),
//       "supplier": Const(name),
//       "title": TextSelector.forScope(".page__subcol-main > h1"),
//       "originalTitle": TextSelector.forScope(
//           ".page__subcol-main > .pmovie__original-title"),
//       "image": Image.forScope(
//           ".pmovie__poster > img", attribute: "data-src", host),
//       "description": TextSelector.forScope(".page__text"),
//       "additionalInfo": Flatten([
//         Join([
//           TextSelector.forScope(
//               ".page__subcol-main .pmovie__subrating--site"),
//           TextSelector.forScope(".page__subcol-main > .pmovie__year"),
//           TextSelector.forScope(".page__subcol-main > .pmovie__genres"),
//         ]),
//         Iterate(
//           itemScope: ".page__subcol-side2 li",
//           item: TextSelector(inline: true),
//         )
//       ]),
//       "similar":
//           Scope(scope: ".pmovie__related", item: contentInfoSelector),
//       "iframe": Attribute.forScope(
//         ".pmovie__player .video-inside iframe",
//         "data-src",
//       ),
//     },
//   )

fn content_details_processor() -> &'static html::ScopedProcessor<ContentDetails> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopedProcessor<ContentDetails>> =
        OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        html::scoped_processor(
            "#dle-content",
            Box::new(html::ContentDetailsProcessor {
                media_type: MediaType::Video,
                title: html::text_value(".page__subcol-main > h1"),
                original_title: html::optional_text_value(
                    ".page__subcol-main > .pmovie__original-title",
                ),
                image: html::optional_map_value(
                    |a| format!("{URL}{a}"),
                    html::optional_attr_value("data-src", ".pmovie__poster > img"),
                ),
                description: html::text_value(".page__text"),
                additional_info: html::flatten(vec![
                    html::join_processors(vec![
                        html::text_value(".page__subcol-main > .pmovie__year"),
                        html::text_value(".page__subcol-main > .pmovie__genres"),
                    ]),
                    html::items_processor(
                        ".page__subcol-side2 li",
                        html::extract_value(|e| {
                            html::sanitize_text(e.text().map(|s| s.into()).collect::<Vec<String>>().join(""))
                        }),
                    ),
                ]),
                similar: html::items_processor(".pmovie__related", content_info_processor()),
                params: html::join_processors(vec![html::attr_value(
                    "data-src",
                    ".pmovie__player .video-inside iframe",
                )]),
            }),
        )
    })
}

fn get_channels_map() -> &'static HashMap<String, String> {
    static CONTENT_DETAILS_PROCESSOR: OnceLock<HashMap<String, String>> = OnceLock::new();
    CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
        HashMap::from([
            ("Новинки".into(), format!("{URL}/page/")),
            ("ТОП 100".into(), format!("{URL}/top.html")),
            ("Повнометражки".into(), format!("{URL}/film/page/")),
            ("Аніме серіали".into(), format!("{URL}/anime/page/")),
        ])
    })
}
