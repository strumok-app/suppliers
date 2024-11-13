mod utils;
mod html;

pub mod models;
pub mod uaserials_pro;

use uaserials_pro::UaserialsProContentSupplier;
use models::{
    ContentDetails, 
    ContentInfo, 
    ContentMediaItem, 
    ContentMediaItemSource, 
    ContentType, 
    ContentSupplier
};

use std::{error::Error, future::Future, str::FromStr};
use strum::VariantNames;
use enum_dispatch::enum_dispatch;
use strum_macros::{EnumIter, EnumString, VariantNames};


#[enum_dispatch(ContentSupplier)]
#[derive(EnumIter, EnumString, VariantNames)]
pub enum AllContentSuppliers {
    #[strum(serialize="UaserialsPro")]
    UaserialsProContentSupplier
}

pub fn avalaible_suppliers() -> Vec<&'static str> {
    AllContentSuppliers::VARIANTS.to_vec()
}

pub fn get_supplier(name: &str) -> Result<AllContentSuppliers, Box<dyn Error>> {
    AllContentSuppliers::from_str(name).map_err(|err| err.into())
}

