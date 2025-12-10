/// flutter_rust_bridge:ignore
// suppliers
mod animeua;
mod anitube;
// mod anixl;
mod animekai;
mod anizone;
mod hianime;
mod mangadex;
mod mangafire;
mod mangainua;
mod mangapark;
mod tmdb;
mod uakinoclub;
mod uaserial;
mod uaserials_pro;
mod ufdub;

use animeua::AnimeUAContentSupplier;
use anitube::AniTubeContentSupplier;
// use anixl::AnixlContentSupplier;
use animekai::AnimeKaiContentSupplier;
use anizone::AnizoneContentSupplier;
use hianime::HianimeContentSupplier;
use mangadex::MangaDexContentSupplier;
use mangafire::MangaFireContentSupplier;
use mangainua::MangaInUaContentSupplier;
use mangapark::MangaParkContentSupplier;
use tmdb::TMDBContentSupplier;
use uakinoclub::UAKinoClubContentSupplier;
use uaserial::UAserialContentSupplier;
use uaserials_pro::UASerialsProContentSupplier;
use ufdub::UFDubContentSupplier;

use enum_dispatch::enum_dispatch;
use std::str::FromStr;
use strum::VariantNames;
use strum_macros::{EnumIter, EnumString, VariantNames};

use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, ContentType,
};

#[enum_dispatch]
pub trait ContentSupplier {
    fn get_channels(&self) -> Vec<String>;
    fn get_default_channels(&self) -> Vec<String>;
    fn get_supported_types(&self) -> Vec<ContentType>;
    fn get_supported_languages(&self) -> Vec<String>;
    async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>>;
    async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>>;
    async fn get_content_details(
        &self,
        id: &str,
        langs: Vec<String>,
    ) -> anyhow::Result<Option<ContentDetails>>;
    async fn load_media_items(
        &self,
        id: &str,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>>;
    async fn load_media_item_sources(
        &self,
        id: &str,
        langs: Vec<String>,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>>;
}

#[enum_dispatch(ContentSupplier)]
#[derive(EnumIter, EnumString, VariantNames)]
#[allow(clippy::enum_variant_names)]
pub enum AllContentSuppliers {
    #[strum(serialize = "TMDB")]
    TMDBContentSupplier,
    #[strum(serialize = "Hianime")]
    HianimeContentSupplier,
    #[strum(serialize = "AnimeKai")]
    AnimeKaiContentSupplier,
    // #[strum(serialize = "Anixl")]
    // AnixlContentSupplier,
    #[strum(serialize = "Anizone")]
    AnizoneContentSupplier,
    #[strum(serialize = "AniTube")]
    AniTubeContentSupplier,
    #[strum(serialize = "AnimeUA")]
    AnimeUAContentSupplier,
    #[strum(serialize = "UASerial")]
    UAserialContentSupplier,
    #[strum(serialize = "UASerialsPro")]
    UASerialsProContentSupplier,
    #[strum(serialize = "UAKinoClub")]
    UAKinoClubContentSupplier,
    #[strum(serialize = "UFDub")]
    UFDubContentSupplier,
    #[strum(serialize = "MangaDex")]
    MangaDexContentSupplier,
    #[strum(serialize = "MangaFire")]
    MangaFireContentSupplier,
    #[strum(serialize = "MangaInUa")]
    MangaInUaContentSupplier,
    #[strum(serialize = "MangaPark")]
    MangaParkContentSupplier,
}

#[enum_dispatch]
pub trait MangaPagesLoader {
    async fn load_pages(&self, id: &str, params: Vec<String>) -> anyhow::Result<Vec<String>>;
}

#[enum_dispatch(MangaPagesLoader)]
#[derive(EnumString)]
pub enum AllMangaPagesLoaders {
    #[strum(serialize = "MangaDex")]
    MangaDexContentSupplier,
    #[strum(serialize = "MangaInUa")]
    MangaInUaContentSupplier,
    #[strum(serialize = "MangaPark")]
    MangaParkContentSupplier,
}

pub fn avalaible_suppliers() -> Vec<String> {
    AllContentSuppliers::VARIANTS
        .iter()
        .map(|&s| s.to_owned())
        .collect()
}

pub fn get_supplier(name: &str) -> Result<AllContentSuppliers, anyhow::Error> {
    AllContentSuppliers::from_str(name).map_err(|err| err.into())
}
