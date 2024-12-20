use crate::utils::{extract_digits, html::{self, DOMProcessor}};

#[derive(Debug)]
pub struct AjaxPlaylist {
    pub videos: Vec<AjaxPlaylistVideo>,
    pub lables: Vec<AjaxPlaylistLabel>,
}

#[derive(Debug)]
pub struct AjaxPlaylistLabel {
    pub id: String,
    pub label: String,
}

#[derive(Debug)]
pub struct AjaxPlaylistVideo {
    pub id: String,
    pub name: String,
    pub file: String,
    pub number: u32,
}

pub struct AjaxPlaylistProcessor {
    videos: Box<dyn DOMProcessor<Vec<AjaxPlaylistVideo>>>,
    lables: Box<dyn DOMProcessor<Vec<AjaxPlaylistLabel>>>,
}

impl AjaxPlaylistProcessor {
    pub fn new() -> Self {
        Self {
            videos: html::ItemsProcessor::new(
                ".playlists-videos > .playlists-items li",
                Box::new(AjaxPlaylisVideoProcessor::new()),
            )
            .filter(|v| !v.id.is_empty())
            .into(),
            lables: html::ItemsProcessor::new(
                ".playlists-lists > .playlists-items li",
                Box::new(AjaxPlaylistLabelProcessor::new()),
            )
            .into(),
        }
    }
}

impl DOMProcessor<AjaxPlaylist> for AjaxPlaylistProcessor {
    fn process(&self, el: &scraper::ElementRef) -> AjaxPlaylist {
        AjaxPlaylist {
            videos: self.videos.process(el),
            lables: self.lables.process(el),
        }
    }
}

struct AjaxPlaylistLabelProcessor {
    id: Box<dyn DOMProcessor<String>>,
    label: Box<dyn DOMProcessor<String>>,
}

impl AjaxPlaylistLabelProcessor {
    pub fn new() -> Self {
        Self {
            id: html::AttrValue::new("data-id").into(),
            label: html::TextValue::new().into(),
        }
    }
}

impl DOMProcessor<AjaxPlaylistLabel> for AjaxPlaylistLabelProcessor {
    fn process(&self, el: &scraper::ElementRef) -> AjaxPlaylistLabel {
        AjaxPlaylistLabel {
            id: self.id.process(el),
            label: self.label.process(el),
        }
    }
}

struct AjaxPlaylisVideoProcessor {
    id: Box<dyn DOMProcessor<String>>,
    name: Box<dyn DOMProcessor<String>>,
    file: Box<dyn DOMProcessor<String>>,
}

impl AjaxPlaylisVideoProcessor {
    fn new() -> Self {
        Self {
            id: html::AttrValue::new("data-id").into(),
            name: html::TextValue::new().into(),
            file: html::AttrValue::new("data-file").into(),
        }
    }
}

impl DOMProcessor<AjaxPlaylistVideo> for AjaxPlaylisVideoProcessor {
    fn process(&self, el: &scraper::ElementRef) -> AjaxPlaylistVideo {
        let name = self.name.process(el);
        let number = extract_digits(&name);

        AjaxPlaylistVideo {
            id: self.id.process(el),
            name,
            file: self.file.process(el),
            number,
        }
    }
}
