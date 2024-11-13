use std::{collections::HashMap, error::Error, future::Future};

use enum_dispatch::enum_dispatch;
use strum_macros::FromRepr;

#[repr(u8)]
#[derive(Copy, Clone, FromRepr, Debug)]
pub enum ContentType {
    Movie = 0,
    Anime,
    Cartoon,
    Series,
    Manga,
}

#[repr(u8)]
#[derive(Copy, Clone, FromRepr, Debug)]
pub enum MediaType {
    Video,
    Manga
}

#[derive(Debug)]
pub struct ContentInfo {
    pub id: String,
    pub title: String,
    pub secondary_title: Option<String>,
    pub image: String,
}

#[derive(Debug)]
pub struct ContentDetails {
    pub title: String,
    pub original_title: Option<String>,
    pub image: String,
    pub description: String,
    pub media_type: MediaType,
    pub additional_info: Vec<String>,
    pub similar: Vec<ContentInfo>,
    pub params: Vec<String>,
}

#[derive(Debug)]
pub struct ContentMediaItem {
    pub number: u32,
    pub title: String,
    pub section: Option<String>,
    pub image: Option<String>,
    pub sources: Option<Vec<ContentMediaItemSource>>,
    pub params: Vec<String>,
}

#[derive(Debug)]
pub enum ContentMediaItemSource {
    Video {
        link: String,
        description: String,
        headers: Option<HashMap<String, String>>,
    },
    Subtitle {
        link: String,
        description: String,
        headers: Option<HashMap<String, String>>,
    },
    Manga {
        description: String,
        pages: Vec<String>
    },
}

#[enum_dispatch]
pub trait ContentSupplier {
    fn get_channels(&self) -> Vec<&str>;
    fn get_default_channels(&self) -> Vec<&str>;
    fn get_supported_types(&self) -> Vec<ContentType>;
    fn get_supported_languages(&self) -> Vec<&str>;
    fn load_channel(&self, channel: &str, page: u32) -> impl Future<Output = Result<Vec<ContentInfo>, Box<dyn Error>>> + Send;
    fn search(&self, query: &str, types: Vec<ContentType>) -> impl Future<Output = Result<Vec<ContentInfo>, Box<dyn Error>>> + Send;
    fn get_content_details(&self, id: &str) -> impl Future<Output = Result<Option<ContentDetails>, Box<dyn Error>>> + Send;
    fn load_media_items(&self, id: &str, params: Vec<String>) -> impl Future<Output = Result<Vec<ContentMediaItem>, Box<dyn Error>>> + Send;
    fn load_media_item_sources(&self, id: &str, params: Vec<String>) -> impl Future<Output = Result<Vec<ContentMediaItemSource>, Box<dyn Error>>> + Send;
}