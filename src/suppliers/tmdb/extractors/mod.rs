mod autoembed;
mod primewire;
mod two_embed;
// mod vidsrcvip;
mod xprime;

use futures::future::BoxFuture;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::models::ContentMediaItemSource;

type BoxExtractor = for<'a> fn(
    &'a SourceParams,
    &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>>;

const EXTRACTORS: [(&str, BoxExtractor); 4] = [
    ("xprime", xprime::extract_boxed),
    ("autoembed", autoembed::extract_boxed),
    ("primewire", primewire::extract_boxed),
    ("two_embed", two_embed::extract_boxed),
];

pub async fn run_extractors(
    params: &SourceParams,
    langs: &[String],
) -> Vec<ContentMediaItemSource> {
    let etractors_itr = EXTRACTORS.into_iter().map(|(name, f)| async move {
        match f(params, langs).await {
            Ok(r) => r,
            Err(err) => {
                warn!("[tmdb] extractor '{name}' failed: {err}");
                vec![]
            }
        }
    });

    futures::future::join_all(etractors_itr)
        .await
        .into_iter()
        .flatten()
        .collect()
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Episode {
    pub s: u32,
    pub e: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SourceParams {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ep: Option<Episode>,
}
