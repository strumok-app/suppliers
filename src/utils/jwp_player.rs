use std::collections::HashMap;

use serde::Deserialize;

use crate::models::ContentMediaItemSource;

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
    pub sources: Vec<Source>,
    pub tracks: Vec<Track>,
}

impl JWPConfig {
    pub fn to_media_item_sources(
        &self,
        prefix: &str,
        headers: Option<HashMap<String, String>>,
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
                    description.push(' ');
                    description.push_str(label);
                }

                result.push(ContentMediaItemSource::Video {
                    link: String::from(&track.file),
                    headers: None,
                    description,
                });
            });
        result
    }
}
