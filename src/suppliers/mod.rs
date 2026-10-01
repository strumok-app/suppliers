mod anikoto;
/// flutter_rust_bridge:ignore
// suppliers
mod animeon;
mod animeua;
mod anitube;
mod anizone;
mod eneyida;
mod mangadex;
mod mangainua;
mod shiro;
mod tmdb;
mod uaflix;
mod uakinoclub;
mod uaserial;
mod uaserials_pro;
mod ufdub;
mod weebcentral;

use anikoto::AnikotoContentSupplier;
use animeon::AnimeONContentSupplier;
use animeua::AnimeUAContentSupplier;
use anitube::AniTubeContentSupplier;
use anizone::AnizoneContentSupplier;
use eneyida::EneyidaContentSupplier;
use mangadex::MangaDexContentSupplier;
use mangainua::MangaInUaContentSupplier;
use shiro::ShiroContentSupplier;
use tmdb::TMDBContentSupplier;
use uaflix::UAFlixSupplier;
use uakinoclub::UAKinoClubContentSupplier;
use uaserial::UAserialContentSupplier;
use uaserials_pro::UASerialsProContentSupplier;
use ufdub::UFDubContentSupplier;
use weebcentral::WeebCentralContentSupplier;

use anyhow::anyhow;
use enum_dispatch::enum_dispatch;
use std::{collections::HashMap, str::FromStr, sync::LazyLock};
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
    async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>>;
    async fn load_media_items(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItem>>;
    async fn load_media_item_sources(
        &self,
        id: &str,
        params: Vec<String>,
    ) -> anyhow::Result<Vec<ContentMediaItemSource>>;
}

#[enum_dispatch(ContentSupplier)]
#[derive(EnumIter, EnumString, VariantNames)]
#[allow(clippy::enum_variant_names)]
pub enum AllContentSuppliers {
    #[strum(serialize = "TMDB")]
    TMDBContentSupplier,
    #[strum(serialize = "Anizone")]
    AnizoneContentSupplier,
    #[strum(serialize = "Anikoto")]
    AnikotoContentSupplier,
    #[strum(serialize = "Shiro")]
    ShiroContentSupplier,
    #[strum(serialize = "AniTube")]
    AniTubeContentSupplier,
    #[strum(serialize = "AnimeUA")]
    AnimeUAContentSupplier,
    #[strum(serialize = "AnimeOn")]
    AnimeONContentSupplier,
    #[strum(serialize = "Eneyida")]
    EneyidaContentSupplier,
    #[strum(serialize = "UAFlix")]
    UAFlixSupplier,
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
    #[strum(serialize = "MangaInUa")]
    MangaInUaContentSupplier,
    #[strum(serialize = "WeebCentral")]
    WeebCentralContentSupplier,
}

#[enum_dispatch]
pub trait MangaPagesLoader {
    async fn load_pages(&self, id: &str, params: Vec<String>) -> anyhow::Result<Vec<String>>;
}

#[enum_dispatch(MangaPagesLoader)]
#[derive(EnumString, VariantNames)]
#[allow(clippy::enum_variant_names)]
pub enum AllMangaPagesLoaders {
    #[strum(serialize = "MangaDex")]
    MangaDexContentSupplier,
    #[strum(serialize = "MangaInUa")]
    MangaInUaContentSupplier,
    #[strum(serialize = "WeebCentral")]
    WeebCentralContentSupplier,
}

// Suppliers are created once and live for the whole app session, so their
// fields (HTTP clients, parsed selectors, caches) are shared between calls.
// Any state a supplier keeps must therefore be thread-safe (`Send + Sync`).
static SUPPLIERS: LazyLock<HashMap<&'static str, AllContentSuppliers>> =
    LazyLock::new(|| build_registry(AllContentSuppliers::VARIANTS));

static MANGA_PAGES_LOADERS: LazyLock<HashMap<&'static str, AllMangaPagesLoaders>> =
    LazyLock::new(|| build_registry(AllMangaPagesLoaders::VARIANTS));

fn build_registry<T: FromStr>(names: &[&'static str]) -> HashMap<&'static str, T> {
    names
        .iter()
        .filter_map(|&name| T::from_str(name).ok().map(|value| (name, value)))
        .collect()
}

pub fn avalaible_suppliers() -> Vec<String> {
    AllContentSuppliers::VARIANTS
        .iter()
        .map(|&s| s.to_owned())
        .collect()
}

pub fn get_supplier(name: &str) -> anyhow::Result<&'static AllContentSuppliers> {
    SUPPLIERS
        .get(name)
        .ok_or_else(|| anyhow!("unknown supplier: {name}"))
}

pub fn get_manga_pages_loader(name: &str) -> anyhow::Result<&'static AllMangaPagesLoaders> {
    MANGA_PAGES_LOADERS
        .get(name)
        .ok_or_else(|| anyhow!("unknown manga pages loader: {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_register_every_supplier_once() {
        for &name in AllContentSuppliers::VARIANTS {
            let first = get_supplier(name).unwrap();
            let second = get_supplier(name).unwrap();
            assert!(std::ptr::eq(first, second), "{name} was recreated");
        }

        for &name in AllMangaPagesLoaders::VARIANTS {
            let first = get_manga_pages_loader(name).unwrap();
            let second = get_manga_pages_loader(name).unwrap();
            assert!(std::ptr::eq(first, second), "{name} was recreated");
        }
    }

    #[test]
    fn should_fail_for_unknown_supplier() {
        assert!(get_supplier("Unknown").is_err());
        assert!(get_manga_pages_loader("Unknown").is_err());
    }
}
