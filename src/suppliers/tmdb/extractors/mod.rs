mod embed_su;
mod two_embed;

use futures::future::BoxFuture;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::models::ContentMediaItemSource;

type BoxExtractor =
    Box<dyn Fn(&SourceParams) -> BoxFuture<anyhow::Result<Vec<ContentMediaItemSource>>>>;

pub async fn run_extractors(params: &SourceParams) -> Vec<ContentMediaItemSource> {
    let extractors: Vec<(&str, BoxExtractor)> = vec![
        ("two_embed", Box::new(|p| Box::pin(two_embed::extract(p)))),
        ("embed_su", Box::new(|p| Box::pin(embed_su::extract(p)))),
    ];

    let etractors_itr = extractors.into_iter().map(|(name, f)| async move {
        match f(params).await {
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
pub struct SourceParams {
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
}
