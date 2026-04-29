use std::collections::HashMap;

use serde::Deserialize;

use crate::{models::ContentMediaItemSource, utils::lang};

#[derive(Deserialize, Debug)]
pub struct Track {
    pub file: String,
    pub kind: String,
    pub label: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Source {
    #[serde(alias = "url")]
    pub file: String,
    pub label: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct JWPConfig {
    #[serde(default, deserialize_with = "deserialize_sources")]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub tracks: Vec<Track>,
}

fn deserialize_sources<'de, D>(deserializer: D) -> Result<Vec<Source>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ListOrSingle<T> {
        List(Vec<T>),
        Single(T),
    }

    // Try to deserialize into the helper enum
    match ListOrSingle::<Source>::deserialize(deserializer)? {
        ListOrSingle::List(list) => Ok(list),
        ListOrSingle::Single(item) => Ok(vec![item]),
    }
}

impl JWPConfig {
    pub fn to_media_item_sources(
        &self,
        prefix: &str,
        headers: Option<HashMap<String, String>>,
        hls_proxy: bool,
    ) -> Vec<ContentMediaItemSource> {
        let mut result = vec![];

        self.sources.iter().enumerate().for_each(|(idx, source)| {
            let num = idx + 1;
            let mut description = format!("{prefix} {num}.");

            if let Some(label) = &source.label {
                description.push(' ');
                description.push_str(label);
            }

            result.push(ContentMediaItemSource::Video {
                link: String::from(&source.file),
                headers: headers.clone(),
                description,
                hls_proxy: hls_proxy,
            });
        });

        self.tracks
            .iter()
            .filter(|&track| {
                track.kind == "caption" || track.kind == "captions" || track.kind == "subtitle"
            })
            .enumerate()
            .for_each(|(idx, track)| {
                let num = idx + 1;
                let mut description = format!("{prefix} {num}.");

                if let Some(label) = &track.label {
                    if !lang::is_allowed(&label) {
                        return;
                    }

                    description.push(' ');
                    description.push_str(label);
                }

                result.push(ContentMediaItemSource::Subtitle {
                    link: track.file.clone(),
                    headers: None,
                    description,
                });
            });
        result
    }
}
