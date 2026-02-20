use anyhow::anyhow;
use indexmap::IndexMap;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        self,
        html::{self, DOMProcessor},
        playerjs,
    },
};

use super::ContentSupplier;

const URL: &str = "https://uaserial.biz";
const SEARCH_URL: &str = "https://uaserial.biz/search";

pub struct UAserialContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_channel_items: html::ItemsProcessor<ContentInfo>,
    processor_search_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
}

impl Default for UAserialContentSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("Фільми", format!("{URL}/movie/")),
                ("Серіали", format!("{URL}/serial/")),
                ("Аніме", format!("{URL}/animeukr/")),
                ("Мультфільми", format!("{URL}/cartoon-movie/")),
                ("Мультсеріали", format!("{URL}/cartoon-series/")),
            ]),
            processor_content_channel_items: html::ItemsProcessor::new(
                "#filters-grid-content .row .col",
                content_info_processor(),
            ),
            processor_search_items: html::ItemsProcessor::new(
                "#block-search-page .row .col",
                content_info_processor(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "#container",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value(".header--title  .title"),
                    original_title: html::optional_text_value(".header--title .original"),
                    image: html::self_hosted_image(URL, ".poster img", "src"),
                    description: html::text_value(".player__info .player__description .text"),
                    additional_info: html::MergeItemsProcessor::default()
                        .add_processor(html::items_processor(
                            ".movie-data .movie-data-item",
                            html::JoinProcessors::new(vec![
                                html::text_value(".type"),
                                html::text_value(".value"),
                            ])
                            .map(|v| v.join(" "))
                            .boxed(),
                        ))
                        .add_processor(html::items_processor(
                            ".movie__genres__container .selection__badge",
                            html::TextValue::new().boxed(),
                        ))
                        .map(|v| {
                            v.into_iter()
                                .map(|s| utils::text::sanitize_text(&s))
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                        })
                        .boxed(),
                    similar: html::default_value(),
                    params: html::join_processors(vec![html::attr_value(
                        "iframe.absolute__fill",
                        "src",
                    )]),
                }
                .boxed(),
            ),
        }
    }
}

fn content_info_processor() -> Box<html::ContentInfoProcessor> {
    html::ContentInfoProcessor {
        id: html::attr_value_map(".item > a", "href", |s| extract_id_from_url(&s)),
        title: html::text_value(".item__data > a .name"),
        secondary_title: html::ItemsProcessor::new(
            ".item__data .info__item",
            html::TextValue::new().boxed(),
        )
        .map(|infos| Some(infos.join(",")))
        .boxed(),
        image: html::self_hosted_image(URL, ".item > a > .img-wrap > img", "src"),
    }
    .into()
}

impl ContentSupplier for UAserialContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        self.channels_map.keys().map(|&s| s.into()).collect()
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
        if page > 1 {
            return Ok(vec![]);
        }

        let request_builder = utils::create_client()
            .get(SEARCH_URL)
            .query(&[("query", &query)]);

        utils::scrap_page(request_builder, &self.processor_search_items).await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = match self.channels_map.get(channel) {
            Some(url) => format!("{url}{page}"),
            None => return Err(anyhow!("unknown channel")),
        };

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_channel_items,
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{URL}/{id}");

        let html = utils::create_client().get(url).send().await?.text().await?;
        let document = scraper::Html::parse_document(&html);
        let root = document.root_element();
        let maybe_details = self.processor_content_details.process(&root);

        Ok(maybe_details)
    }

    async fn load_media_items(
        &self,
        _id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if params.len() != 1 {
            return Err(anyhow!("Wrong params size"));
        }

        let url = &params[0];
        let sources = playerjs::load_and_parse_playerjs(
            utils::create_client().get(url),
            playerjs::convert_strategy_dub_season_ep,
        )
        .await?;

        Ok(sources)
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

fn extract_id_from_url(id: &str) -> String {
    if let Some(end) = id.strip_prefix("/") {
        return end.to_string();
    }

    if let Some(end) = id.strip_prefix(URL) {
        return end.to_string();
    }

    id.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn should_load_channel() {
        let res = UAserialContentSupplier::default()
            .load_channel("Серіали", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = UAserialContentSupplier::default()
            .search("термінатор", 0)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details_for_movie() {
        let res = UAserialContentSupplier::default()
            .get_content_details("movie-the-terminator", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_content_details_for_tv_show() {
        let res = UAserialContentSupplier::default()
            .get_content_details("terminator-zero/season-1", vec![])
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_load_media_item() {
        let res = UAserialContentSupplier::default()
            .load_media_items(
                "terminator-zero/season-1",
                vec![],
                vec!["https://hdvbua.pro/embed/9146".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
    }
}
