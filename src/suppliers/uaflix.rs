use anyhow::anyhow;
use indexmap::IndexMap;
use scraper::{ElementRef, Selector};

use crate::{
    models::{
        ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
        MediaType,
    },
    suppliers::ContentSupplier,
    utils::{
        self,
        html::{self, DOMProcessor, ItrDOMProcessor, attr_value_map},
    },
};

const URL: &str = "https://uafix.net";
const SEARCH_URL: &str = "https://uafix.net/search.html";

struct Episode {
    link: String,
    image: Option<String>,
}

struct Episodes {
    episodes: Vec<Episode>,
    pages: usize,
}

pub struct UAFlixSupplier {
    channels_map: IndexMap<&'static str, String>,
    selector_episodes_scope: Selector,
    selector_episode_link: Selector,
    selector_episode_image: Selector,
    selector_pages: Selector,
    processor_content_details: html::ScopeProcessor<ContentDetails>,
    processor_content_info_items: html::ItemsProcessor<ContentInfo>,
    processor_content_info_channel_items: html::ItemsProcessor<ContentInfo>,
}

impl Default for UAFlixSupplier {
    fn default() -> Self {
        Self {
            channels_map: IndexMap::from([
                ("Фільми", format!("{URL}/film/")),
                ("Серіали", format!("{URL}/serials/")),
                ("Мультфільми", format!("{URL}/cartoons/")),
                ("Дорами", format!("{URL}/dorama/")),
                ("Аніме", format!("{URL}/anime/")),
            ]),
            selector_episodes_scope: Selector::parse("#sers-wr .video-item").unwrap(),
            selector_episode_link: Selector::parse("a.vi-img").unwrap(),
            selector_episode_image: Selector::parse("img").unwrap(),
            selector_pages: Selector::parse(".pagination li").unwrap(),
            processor_content_details: html::ScopeProcessor::new(
                "#dle-content",
                html::ContentDetailsProcessor {
                    media_type: MediaType::Video,
                    title: html::text_value("#ftitle > span"),
                    original_title: html::default_value(),
                    image: html::self_hosted_image(URL, ".fposter2 img", "src"),
                    description: html::text_value_map("#serial-kratko", |text| {
                        utils::text::sanitize_text(&text)
                    }),
                    additional_info: html::items_processor(
                        "#finfo li",
                        html::TextValue::new().all_nodes().boxed(),
                    ),
                    similar: html::default_value(),
                    params: html::join_processors(vec![html::attr_value(
                        ".video-box iframe",
                        "src",
                    )])
                    .filter(|link| !link.is_empty())
                    .boxed(),
                }
                .boxed(),
            ),
            processor_content_info_items: html::ItemsProcessor::new(
                ".sres-wrap",
                html::ContentInfoProcessor {
                    id: html::AttrValue::new("href")
                        .map_optional(extract_id_from_url)
                        .unwrap_or_default()
                        .boxed(),
                    title: html::text_value("h2"),
                    secondary_title: html::default_value(),
                    image: html::self_hosted_image(URL, ".sres-img img", "src"),
                }
                .boxed(),
            ),
            processor_content_info_channel_items: html::ItemsProcessor::new(
                "#dle-content .video-item",
                html::ContentInfoProcessor {
                    id: attr_value_map(".vi-img", "href", extract_id_from_url),
                    title: html::text_value(".vi-desc .vi-title"),
                    secondary_title: html::default_value(),
                    image: html::self_hosted_image(URL, ".vi-img img", "src"),
                }
                .boxed(),
            ),
        }
    }
}

impl ContentSupplier for UAFlixSupplier {
    fn get_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_default_channels(&self) -> Vec<String> {
        vec![]
    }

    fn get_supported_types(&self) -> Vec<ContentType> {
        vec![ContentType::Movie, ContentType::Series]
    }

    fn get_supported_languages(&self) -> Vec<String> {
        vec!["uk".to_string()]
    }

    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let client = utils::create_client();

        let request = client
            .get(SEARCH_URL)
            .query(&[("do", "search"), ("subaction", "search"), ("story", query)])
            .query(&[("search_start", (page + 1))]);

        let results = utils::scrap_page(request, &self.processor_content_info_items).await?;

        Ok(results)
    }

    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        let url = utils::datalife::get_channel_url(&self.channels_map, channel, page)?;

        utils::scrap_page(
            utils::create_client().get(&url),
            &self.processor_content_info_channel_items,
        )
        .await
    }

    async fn get_content_details(
        &self,
        id: &str,
        _langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>> {
        let url = format!("{URL}/{id}/");
        let html = utils::create_client()
            .get(&url)
            .send()
            .await?
            .text()
            .await?;

        let (mut maybe_content_details, episodes) = self.try_load_content_details(&html);

        if let Some(&mut ref mut content_details) = maybe_content_details.as_mut() {
            if !episodes.episodes.is_empty() {
                let mut content_media_items: Vec<ContentMediaItem> = vec![];

                let pages = episodes.pages;

                self.fill_content_media_items_from_episodes(&mut content_media_items, episodes);

                if pages > 1 {
                    for page in 2..pages {
                        let page_url = format!("{url}?page={page}");

                        let episodes: Episodes = self.load_next_page_episodes(&page_url).await?;

                        if episodes.episodes.is_empty() {
                            break;
                        }

                        self.fill_content_media_items_from_episodes(
                            &mut content_media_items,
                            episodes,
                        );
                    }
                }

                content_details.media_items = Some(content_media_items)
            }
        }

        Ok(maybe_content_details)
    }

    async fn load_media_items(
        &self,
        _id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        if params.len() != 1 {
            return Err(anyhow!("single param expected"));
        }

        let media_items = utils::playerjs::load_and_parse_playerjs(
            utils::create_client()
                .get(&params[0])
                .header("Referer", URL),
            utils::playerjs::convert_strategy_dub_season_ep,
        )
        .await?;

        Ok(media_items)
    }

    async fn load_media_item_sources(
        &self,
        id: &str,
        _langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        if params.len() != 1 {
            return Err(anyhow!("single param expected"));
        }

        let url = format!("{}/{}/{}/", URL, id, params[0]);
        let iframe_url = self.extract_iframe_url(&url).await?;

        let sources = utils::playerjs::load_and_parse_playerjs_sources(
            utils::create_client()
                .get(&iframe_url)
                .header("Referer", URL),
            "Source",
        )
        .await?;

        Ok(sources)
    }
}

impl UAFlixSupplier {
    async fn load_next_page_episodes(&self, page_url: &str) -> anyhow::Result<Episodes> {
        let html = utils::create_client()
            .get(page_url)
            .send()
            .await?
            .text()
            .await?;

        let document = scraper::Html::parse_document(&html);

        Ok(self.try_extract_episodes_links(document.root_element()))
    }

    async fn extract_iframe_url(&self, url: &str) -> anyhow::Result<String> {
        let html = utils::create_client().get(url).send().await?.text().await?;

        let root = scraper::Html::parse_document(&html);
        let selector = scraper::Selector::parse(".video-box iframe").unwrap();

        let iframe_el = root
            .select(&selector)
            .next()
            .ok_or_else(|| anyhow!("iframe not found"))?;

        let link = iframe_el
            .attr("src")
            .ok_or_else(|| anyhow!("iframe has no link"))?;

        Ok(link.to_string())
    }

    fn fill_content_media_items_from_episodes(
        &self,
        content_media_items: &mut Vec<ContentMediaItem>,
        episodes: Episodes,
    ) {
        episodes
            .episodes
            .into_iter()
            .rev()
            .filter_map(|ep| {
                let image = ep.image;

                let id = ep.link;
                let (_, id) = id[..id.len() - 1].rsplit_once("/")?;
                let parts: Vec<_> = id.split("-").collect();

                let &s_num = parts.get(1)?;
                let &e_num = parts.get(3)?;

                Some(ContentMediaItem {
                    title: format!("Серія {e_num}"),
                    sources: None,
                    section: Some(s_num.to_string()),
                    image,
                    params: vec![id.to_string()],
                })
            })
            .for_each(|item| content_media_items.push(item));
    }

    fn try_load_content_details(&self, html: &str) -> (Option<ContentDetails>, Episodes) {
        let document = scraper::Html::parse_document(html);
        let root = document.root_element();

        let maybe_details = self.processor_content_details.process(&root);
        let episodes_links = self.try_extract_episodes_links(root);

        return (maybe_details, episodes_links);
    }

    fn try_extract_episodes_links(&self, root: ElementRef<'_>) -> Episodes {
        let episodes: Vec<_> = root
            .select(&self.selector_episodes_scope)
            .filter_map(|el| {
                let link = el
                    .select(&self.selector_episode_link)
                    .next()
                    .and_then(|el| el.attr("href"))?;

                let image = el
                    .select(&self.selector_episode_image)
                    .next()
                    .and_then(|el| el.attr("data-src"))
                    .map(|path| format!("{URL}/{path}"));

                Some(Episode {
                    link: link.to_string(),
                    image,
                })
            })
            .collect();

        let pages = root.select(&self.selector_pages).count();

        Episodes { episodes, pages }
    }
}

fn extract_id_from_url(mut url: String) -> String {
    url.drain((URL.len() + 1)..(url.len() - 1)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_load_channel() {
        let res = UAFlixSupplier::default()
            .load_channel("Аніме", 2)
            .await
            .unwrap();
        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_search() {
        let res = UAFlixSupplier::default().search("Наруто", 1).await;

        println!("{res:#?}");
    }

    #[tokio::test]
    async fn should_get_content_details_1() {
        let res = UAFlixSupplier::default()
            .get_content_details("serials/divni-diva", vec![])
            .await;

        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_get_content_details_2() {
        let res = UAFlixSupplier::default()
            .get_content_details("serials/naruto-naruto", vec![])
            .await;

        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_load_media_item() {
        let res = UAFlixSupplier::default()
            .load_media_items(
                "serials/naruto-naruto",
                vec![],
                vec!["https://ashdi.vip/serial/236".to_string()],
            )
            .await;

        println!("{res:#?}")
    }

    #[tokio::test]
    async fn should_load_media_item_sources() {
        let res = UAFlixSupplier::default()
            .load_media_item_sources(
                "serials/schodennik-z-chuzhozemja",
                vec![],
                vec!["season-01-episode-02".to_string()],
            )
            .await;

        println!("{res:#?}")
    }
}
