use anyhow::anyhow;

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    utils::{
        self, create_client,
        html::{self, DOMProcessor},
    },
};

use super::ContentSupplier;

const SITE_URL: &str = "https://anizone.to";

pub struct AnizoneContentSupplier {
    selector_player: scraper::Selector,
    selector_tracks: scraper::Selector,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
}

impl Default for AnizoneContentSupplier {
    fn default() -> Self {
        Self {
            selector_player: scraper::Selector::parse("main media-player").unwrap(),
            selector_tracks: scraper::Selector::parse("track[kind='subtitles']").unwrap(),
            processor_content_info_items: html::ItemsProcessor::new(
                "main > div > div > div.grid > div",
                html::ContentInfoProcessor {
                    id: html::attr_value_map("div.h-6.inline > a", "href", extract_id_from_url),
                    title: html::text_value_map("div.h-6.inline > a", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    secondary_title: html::items_processor(
                        "div.h-4 > span",
                        html::TextValue::new().boxed(),
                    )
                    .map(|s| Some(s.join(", ")))
                    .boxed(),
                    image: html::attr_value("img", "src"),
                }
                .boxed(),
            ),
            processor_content_details: html::ScopeProcessor::new(
                "main",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value("h1"),
                    original_title: html::default_value(),
                    image: html::attr_value("div.mx-auto img", "src"),
                    description: html::text_value_map("div.text-slate-100 div", |s| {
                        utils::text::sanitize_text(&s)
                    }),
                    additional_info: html::items_processor(
                        "div.text-slate-100 span span",
                        html::TextValue::new().boxed(),
                    ),
                    similar: html::default_value(),
                    params: html::default_value(),
                }
                .boxed(),
            ),
        }
    }
}

impl ContentSupplier for AnizoneContentSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Anime]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["en".to_string(), "ja".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        if page > 1 {
            return Ok(vec![]);
        }

        utils::scrap_page(
            utils::create_client()
                .get(format!("{SITE_URL}/anime"))
                .query(&[("search", query)]),
            &self.processor_content_info_items,
        )
        .await
    }

    async fn load_channel(&self, _channel: &str, _page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        Err(anyhow!("unimplemented"))
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        utils::scrap_page(
            utils::create_client().get(format!("{SITE_URL}/anime/{id}")),
            &self.processor_content_details,
        )
        .await
    }

    async fn load_media_items(
        &self,
        id: &str,
        _langs: Vec<String>,
        _params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        let url = format!("{SITE_URL}/anime/{id}/1");

        let page_content = create_client().get(url).send().await?.text().await?;

        let document = scraper::Html::parse_document(&page_content);
        let selector = scraper::Selector::parse("main div.order-2 div a").unwrap();

        let results: Vec<_> = document
            .select(&selector)
            .enumerate()
            .map(|(i, el)| {
                let ep_num = i + 1;
                let text: String = el.text().collect();

                ContentMediaItem {
                    title: utils::text::sanitize_text(&text),
                    section: None,
                    sources: None,
                    image: None,
                    params: vec![ep_num.to_string()],
                }
            })
            .collect();

        Ok(results)
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() != 1 {
            return Err(anyhow!("expected ep_num in params"));
        }

        let ep_num = &params[0];

        let anime_page_res = self.load_anime_page(id, ep_num, langs).await?;
        let mut results: Vec<ContentMediaItemSource> = vec![];

        results.push(ContentMediaItemSource::Video {
            link: anime_page_res.hls_src,
            description: "Default".to_string(),
            headers: None,
        });

        for sub in anime_page_res.subtitles {
            results.push(ContentMediaItemSource::Subtitle {
                link: sub.src,
                description: sub.label,
                headers: None,
            });
        }

        Ok(results)
    }
}

#[derive(Debug)]
struct Subtitle {
    src: String,
    label: String,
}

#[derive(Debug)]
struct AnimePageResult {
    hls_src: String,
    subtitles: Vec<Subtitle>,
}

impl AnizoneContentSupplier {
    async fn load_anime_page(
        &self,
        id: &str,
        ep_num: &str,
        langs: Vec<String>,
    ) -> anyhow::Result<AnimePageResult> {
        let url = format!("{SITE_URL}/anime/{id}/{ep_num}");

        let page_content = create_client().get(url).send().await?.text().await?;

        let document = scraper::Html::parse_document(&page_content);

        let player_el = document
            .select(&self.selector_player)
            .next()
            .ok_or_else(|| anyhow!("player not found for anime {} ep_num {}", id, ep_num))?;

        let hls_src = player_el
            .attr("src")
            .ok_or_else(|| anyhow!("player src not found for anime {} ep_num {}", id, ep_num))?;

        let subtitles: Vec<_> = player_el
            .select(&self.selector_tracks)
            .filter_map(|track_el| {
                let src = track_el.attr("src")?;
                let label = track_el.attr("label")?;

                if !utils::lang::is_allowed(&langs, label) {
                    return None;
                }

                Some(Subtitle {
                    src: src.to_string(),
                    label: label.to_string(),
                })
            })
            .collect();

        Ok(AnimePageResult {
            hls_src: hls_src.to_string(),
            subtitles,
        })
    }
}

fn extract_id_from_url(id: String) -> String {
    if !id.is_empty() {
        let offset = SITE_URL.len() + 7;
        return id[offset..].to_string();
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test(tokio::test)]
    async fn should_search() {
        let res = AnizoneContentSupplier::default().search("Naruto", 1).await;
        println!("{res:#?}");
    }

    #[test_log::test(tokio::test)]
    async fn should_get_content_details() {
        let res = AnizoneContentSupplier::default()
            .get_content_details("uyyyn4kf", vec![])
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_items() {
        let res = AnizoneContentSupplier::default()
            .load_media_items("47tr68c3", vec![], vec![])
            .await;
        println!("{res:#?}")
    }

    #[test_log::test(tokio::test)]
    async fn should_load_media_item_sources() {
        let res = AnizoneContentSupplier::default()
            .load_media_item_sources(
                "47tr68c3",
                vec!["en".to_string(), "jp".to_string()],
                vec!["2".to_string()],
            )
            .await;
        println!("{res:#?}")
    }
}
