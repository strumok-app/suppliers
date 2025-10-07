use anyhow::Ok;
use futures::future::BoxFuture;

use crate::{models::ContentMediaItemSource, suppliers::tmdb::extractors::SourceParams};

const URL: &str = "https://vidzee.wtf";

pub fn extract_boxed<'a>(
    params: &'a SourceParams,
    _langs: &'a [String],
) -> BoxFuture<'a, anyhow::Result<Vec<ContentMediaItemSource>>> {
    Box::pin(extract(params))
}

pub async fn extract(params: &SourceParams) -> anyhow::Result<Vec<ContentMediaItemSource>> {
    Ok(vec![])
}
