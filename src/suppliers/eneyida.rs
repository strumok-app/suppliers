use anyhow::anyhow;
use indexmap::IndexMap;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{
        self, datalife,
        html::{self, DOMProcessor, ItrDOMProcessor},
        playerjs,
    },
};

const SITE_URL: &str = "https://eneyida.tv";

pub struct EneyidaContentSupplier {
    channels_map: IndexMap<&'static str, String>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
}

impl Default for EneyidaContentSupplier {
    fn default() -> Self {
        EneyidaContentSupplier {
            channels_map: IndexMap::from([
                ("Фільми", format!("{SITE_URL}/films/page/")),
                ("Серіали", format!("{SITE_URL}/series/page/")),
                ("Мультфільми", format!("{SITE_URL}/cartoon/page/")),
                ("Мультсеріали", format!("{SITE_URL}/cartoon-series/page/")),
                ("Аніме", format!("{SITE_URL}/anime/page/")),
            ]),
            processor_content_info_items: html::ItemsProcessor::new(
                "#dle-content article.short",
                content_info_processor(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "#dle-content",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value_map(".full_header-title h1", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    original_title: html::optional_text_value(".full_header-subtitle"),
                    image: html::self_hosted_image(
                        SITE_URL,
                        ".full_header-title .full_content-poster img",
                        "src",
                    ),
                    description: html::text_value_map("#full_content-desc p", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    additional_info: html::ItemsProcessor::new(
                        "#full_info > li",
                        html::TextValue::new()
                            .all_nodes()
                            .map(|s| utils::text::sanitize_text(&s))
                            .boxed(),
                    )
                    .filter(|s| !s.is_empty())
                    .boxed(),
                    similar: html::items_processor(".related_item", content_info_processor()),
                    params: html::JoinProcessors::default()
                        .add_processor(html::attr_value(".video_box.visible iframe", "src"))
                        .filter(|s| !s.is_empty())
                        .boxed(),
                }
                .boxed(),
            ),
        }
    }
}

fn content_info_processor() -> Box<dyn DOMProcessor<ContentInfo>> {
    html::ContentInfoProcessor {
        id: html::attr_value_map("a.short_title", "href", |s| {
            datalife::extract_id_from_url(SITE_URL, s)
        }),
        title: html::text_value_map("a.short_title", |s| utils::text::sanitize_text(&s)),
        secondary_title: html::TextValue::new()
            .all_nodes()
            .map(|s| utils::text::sanitize_text(&s))
            .in_scope(".short_subtitle")
            .boxed(),
        image: html::self_hosted_image(SITE_URL, "a.short_img img", "data-src"),
    }
    .boxed()
}

impl ContentSupplier for EneyidaContentSupplier {
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
        utils::scrap_page(
            datalife::search_request(SITE_URL, query).query(&[("search_start", page.to_string())]),
            &self.processor_content_info_items,
        )
        .await
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = datalife::get_channel_url(&self.channels_map, channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_info_items,
        )
        .await
    }

    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>> {
        let url = datalife::format_id_from_url(SITE_URL, id);

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_details,
        )
        .await
    }

    async fn load_media_items(
        &self,
        _id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if !params.is_empty() {
            playerjs::load_and_parse_playerjs(
                utils::create_client().get(&params[0]),
                playerjs::convert_strategy_dub_season_ep,
            )
            .await
        } else {
            Err(anyhow!("iframe url expected"))
        }
    }

    async fn load_media_item_sources(
        &self,
        _id: &str,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        Err(anyhow!("unimplemented"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn eneyida_should_load_channels() {
        let supplier = EneyidaContentSupplier::default();
        for channel in supplier.get_channels() {
            let res = supplier.load_channel(&channel, 2).await.unwrap();
            println!("{channel}: {res:#?}");
            assert!(!res.is_empty(), "channel {channel} is empty");
        }
    }

    #[tokio::test]
    async fn eneyida_should_load_content_details() {
        let res = EneyidaContentSupplier::default()
            .get_content_details("9186-ludyna-iakoi-ne-bulo")
            .await
            .unwrap();
        println!("{res:#?}");
        let details = res.unwrap();
        assert!(!details.title.is_empty());
        assert!(!details.params.is_empty());
    }

    #[tokio::test]
    async fn eneyida_should_load_series_content_details() {
        let res = EneyidaContentSupplier::default()
            .get_content_details("3178-terminator-hroniky-sary-konnor")
            .await
            .unwrap();
        println!("{res:#?}");
        let details = res.unwrap();
        assert!(!details.title.is_empty());
        assert!(!details.params.is_empty());
    }

    #[tokio::test]
    async fn eneyida_should_load_movie_media_items() {
        let res = EneyidaContentSupplier::default()
            .load_media_items(
                "9186-ludyna-iakoi-ne-bulo",
                vec!["https://hdvbua.pro/vid/93825".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
        assert_eq!(res.len(), 1);
    }

    #[tokio::test]
    async fn eneyida_should_load_series_media_items() {
        let res = EneyidaContentSupplier::default()
            .load_media_items(
                "3178-terminator-hroniky-sary-konnor",
                vec!["https://hdvbua.pro/embd/462".into()],
            )
            .await
            .unwrap();
        println!("{res:#?}");
        assert!(res.len() > 1);
    }

    #[tokio::test]
    async fn eneyida_should_search() {
        let res = EneyidaContentSupplier::default()
            .search("Термінатор", 1)
            .await
            .unwrap();
        println!("{res:#?}");
        assert!(!res.is_empty());
    }

    #[tokio::test]
    async fn eneyida_should_search_second_page() {
        let first = EneyidaContentSupplier::default()
            .search("людина", 1)
            .await
            .unwrap();
        let second = EneyidaContentSupplier::default()
            .search("людина", 2)
            .await
            .unwrap();
        println!("{second:#?}");
        assert!(!second.is_empty());
        assert_ne!(first[0].id, second[0].id);
    }
}
