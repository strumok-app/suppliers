use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
};

use super::ContentSupplier;

const URL: &str = "https://aniplaynow.live";

#[derive(Default)]
struct AniplayContentSupplier {}

impl ContentSupplier for AniplayContentSupplier {
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
        vec!["en".into()]
    }

    async fn search(&self, query: String, types: Vec<String>) -> anyhow::Result<Vec<ContentInfo>> {
        todo!()
    }

    async fn load_channel(&self, channel: String, page: u16) -> anyhow::Result<Vec<ContentInfo>> {
        todo!()
    }

    async fn get_content_details(&self, id: String) -> anyhow::Result<Option<ContentDetails>> {
        todo!()
    }

    async fn load_media_items(
        &self,
        id: String,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>> {
        todo!()
    }

    async fn load_media_item_sources(
        &self,
        id: String,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>> {
        todo!()
    }
}
