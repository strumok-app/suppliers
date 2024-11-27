/// flutter_rust_bridge:ignore

mod tests;
mod utils;

// suppliers
mod anitube;
mod animeua;
mod uaserial;
mod uaserials_pro;
mod uakinoclub; 
mod uafilms;
mod ufdub;

use anitube::AniTubeContentSupplier;
use animeua::AnimeUAContentSupplier;
use uaserial::UAserialContentSupplier;
use uaserials_pro::UASerialsProContentSupplier;
use uakinoclub::UAKinoClubContentSupplier;
use uafilms::UAFilmsContentSupplier;
use ufdub::UFDubContentSupplier;

use std::str::FromStr;
use enum_dispatch::enum_dispatch;
use strum::VariantNames;
use strum_macros::{EnumIter, EnumString, VariantNames};

use crate::models::{ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType};


#[enum_dispatch]
pub trait ContentSupplier {
    fn get_channels(&self) -> Vec<String>;
    fn get_default_channels(&self) -> Vec<String>;
    fn get_supported_types(&self) -> Vec<ContentType>;
    fn get_supported_languages(&self) -> Vec<String>;
    async fn search(
        &self,
        query: String,
        types: Vec<String>,
    ) -> Result<Vec<ContentInfo>, anyhow::Error>;
    async fn load_channel(
        &self,
        channel: String,
        page: u16,
    ) -> Result<Vec<ContentInfo>, anyhow::Error>;
    async fn get_content_details(
        &self,
        id: String,
    ) -> Result<Option<ContentDetails>, anyhow::Error>;
    async fn load_media_items(
        &self,
        id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItem>, anyhow::Error>;
    async fn load_media_item_sources(
        &self,
        id: String,
        params: Vec<String>,
    ) -> Result<Vec<ContentMediaItemSource>, anyhow::Error>;
}

#[enum_dispatch(ContentSupplier)]
#[derive(EnumIter, EnumString, VariantNames)]
pub enum AllContentSuppliers {
    #[strum(serialize="AniTube")]
    AniTubeContentSupplier,
    #[strum(serialize="AnimeUA")]
    AnimeUAContentSupplier,
    #[strum(serialize="UASerial")]
    UAserialContentSupplier,
    #[strum(serialize="UASerialsPro")]
    UASerialsProContentSupplier,
    #[strum(serialize="UAKinoClub")]
    UAKinoClubContentSupplier,
    #[strum(serialize="UAFilms")]
    UAFilmsContentSupplier,
    #[strum(serialize="UFDub")]
    UFDubContentSupplier,
}

pub fn avalaible_suppliers() -> Vec<String> {
    AllContentSuppliers::VARIANTS.iter().map(|&s| s.to_owned()).collect()
}

pub fn get_supplier(name: &str) -> Result<AllContentSuppliers, anyhow::Error> {
    AllContentSuppliers::from_str(name).map_err(|err| err.into())
}
