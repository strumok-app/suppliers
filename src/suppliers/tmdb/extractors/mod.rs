mod open_subs;
mod two_embed;
mod vidfast;
mod vidlink;
mod vidrock;
mod vidzee;

use std::time;

use futures::future::BoxFuture;
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::models::ContentMediaItemSource;

type BoxExtractor =
    for<'a> fn(&'a SourceParams) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>>;

const EXTRACTORS: [(&str, BoxExtractor); 6] = [
    ("vidlink", vidlink::extract_boxed),
    ("vidfast", vidfast::extract_boxed),
    ("vidrock", vidrock::extract_boxed),
    ("vidzee", vidzee::extract_boxed),
    ("two_embed", two_embed::extract_boxed),
    ("open_subs", open_subs::extract_boxed),
];

pub async fn run_extractors(params: &SourceParams) -> Vec<ContentMediaItemSource> {
    let etractors_itr = EXTRACTORS.into_iter().map(|(name, f)| async move {
        let start_ts = time::Instant::now();
        let res = match f(params).await {
            Ok(r) => r,
            Err(err) => {
                warn!("[tmdb] extractor '{name}' failed: {err}");
                vec![]
            }
        };
        let end_ts = time::Instant::now();
        let duration = end_ts.duration_since(start_ts);
        info!("[tmdb] extractor '{name}' finished in {duration:?}");
        res
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
