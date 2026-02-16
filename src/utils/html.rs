#![allow(unused)]

use std::{borrow::Cow, str, sync::OnceLock};

use chrono::format::Item;
use regex::Regex;
use scraper::{ElementRef, Selector};

use crate::models::{
    ContentDetails, ContentInfo, ContentMediaItem, ContentMediaItemSource, MediaType,
};

// default pattern
//
// fn content_info_items_processor() -> &'static html::ItemsProcessor<ContentInfo> {
//     static CONTENT_INFO_ITEMS_PROCESSOR: OnceLock<html::ItemsProcessor<ContentInfo>> =
//         OnceLock::new();
//     CONTENT_INFO_ITEMS_PROCESSOR
//         .get_or_init(|| html::ItemsProcessor::new("", content_info_processor()))
// }
//
// fn content_info_processor() -> Box<html::ContentInfoProcessor> {
//     html::ContentInfoProcessor {
//         id: html::default_value(),
//         title: html::default_value(),
//         secondary_title: html::default_value(),
//         image: html::default_value(),
//     }
//     .into()
// }
//
// fn content_details_processor() -> &'static html::ScopeProcessor<ContentDetails> {
//     static CONTENT_DETAILS_PROCESSOR: OnceLock<html::ScopeProcessor<ContentDetails>> =
//         OnceLock::new();
//     CONTENT_DETAILS_PROCESSOR.get_or_init(|| {
//         html::ScopeProcessor::new(
//             "#dle-content",
//             html::ContentDetailsProcessor {
//                 media_type: MediaType::Video,
//                 title: html::default_value(),
//                 original_title: html::default_value(),
//                 image: html::default_value(),
//                 description: html::default_value(),
//                 additional_info: html::default_value(),
//                 similar: html::default_value(),
//                 params: html::default_value(),
//             }
//             .boxed(),
//         )
//     })
// }

// base
pub trait DOMProcessor<T>: Sync + Send {
    fn process(&self, el: &ElementRef) -> T;

    fn map<Map, Out>(self, map: Map) -> MapValue<T, Out>
    where
        Self: Sized + 'static,
        Map: Fn(T) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, Box::new(self))
    }

    fn in_scope(self, selectors: &str) -> ScopeProcessor<T>
    where
        Self: Sized + 'static,
    {
        ScopeProcessor::new(selectors, Box::new(self))
    }

    fn in_scope_flatten<Item>(self, selectors: &str) -> MapValue<Option<Option<Item>>, Option<Item>>
    where
        Self: Sized + 'static + DOMProcessor<Option<Item>>,
        Item: 'static,
    {
        ScopeProcessor::new(selectors, Box::new(self)).flatten()
    }

    fn itr_scope(self, selectors: &str) -> ItemsProcessor<T>
    where
        Self: Sized + 'static,
    {
        ItemsProcessor::new(selectors, Box::new(self))
    }

    fn map_optional<Map, In, Out>(self, map: Map) -> MapValue<Option<In>, Option<Out>>
    where
        Self: Sized + 'static + DOMProcessor<Option<In>>,
        Map: Fn(In) -> Out + 'static + Copy + Sync + Send,
    {
        MapValue::new(move |opt| opt.map(map), Box::new(self))
    }

    fn flatten<Item>(self) -> MapValue<Option<Option<Item>>, Option<Item>>
    where
        Self: Sized + 'static + DOMProcessor<Option<Option<Item>>>,
    {
        MapValue::new(move |opt| opt.flatten(), Box::new(self))
    }

    fn unwrap_or_default<In>(self) -> MapValue<Option<In>, In>
    where
        Self: Sized + 'static + DOMProcessor<Option<In>>,
        In: Default,
    {
        MapValue::new(|opt| opt.unwrap_or_default(), Box::new(self))
    }

    fn boxed(self) -> Box<dyn DOMProcessor<T>>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

pub trait ItrDOMProcessor<T>: DOMProcessor<Vec<T>> {
    fn filter<Item, Predicate>(self, predicate: Predicate) -> FilterProcessor<Item>
    where
        Self: Sized + 'static + ItrDOMProcessor<Item>,
        Predicate: Fn(&Item) -> bool + 'static + Sync + Send,
    {
        FilterProcessor::new(predicate, Box::new(self))
    }

    fn map_item<Map, Out>(self, map: Map) -> MapItem<T, Out>
    where
        Self: Sized + 'static + ItrDOMProcessor<T>,
        Map: Fn(T) -> Out + 'static + Sync + Send,
    {
        MapItem::new(map, Box::new(self))
    }
}

// models processor
pub struct ContentInfoProcessor {
    pub id: Box<dyn DOMProcessor<String>>,
    pub title: Box<dyn DOMProcessor<String>>,
    pub secondary_title: Box<dyn DOMProcessor<Option<String>>>,
    pub image: Box<dyn DOMProcessor<String>>,
}

impl DOMProcessor<ContentInfo> for ContentInfoProcessor {
    fn process(&self, el: &ElementRef) -> ContentInfo {
        ContentInfo {
            id: self.id.process(el),
            title: self.title.process(el),
            secondary_title: self.secondary_title.process(el),
            image: self.image.process(el),
        }
    }
}

pub struct ContentDetailsProcessor {
    pub media_type: MediaType,
    pub title: Box<dyn DOMProcessor<String>>,
    pub original_title: Box<dyn DOMProcessor<Option<String>>>,
    pub image: Box<dyn DOMProcessor<String>>,
    pub description: Box<dyn DOMProcessor<String>>,
    pub additional_info: Box<dyn DOMProcessor<Vec<String>>>,
    pub similar: Box<dyn DOMProcessor<Vec<ContentInfo>>>,
    pub params: Box<dyn DOMProcessor<Vec<String>>>,
}

impl DOMProcessor<ContentDetails> for ContentDetailsProcessor {
    fn process(&self, el: &ElementRef) -> ContentDetails {
        ContentDetails {
            media_type: self.media_type,
            title: self.title.process(el),
            original_title: self.original_title.process(el),
            image: self.image.process(el),
            description: self.description.process(el),
            additional_info: self.additional_info.process(el),
            media_items: None,
            similar: self.similar.process(el),
            params: self.params.process(el),
        }
    }
}

pub struct ContentMediaItemProcessor {
    pub title: Box<dyn DOMProcessor<String>>,
    pub section: Box<dyn DOMProcessor<Option<String>>>,
    pub image: Box<dyn DOMProcessor<Option<String>>>,
    pub sources: Box<dyn DOMProcessor<Option<Vec<ContentMediaItemSource>>>>,
    pub params: Box<dyn DOMProcessor<Vec<String>>>,
}

impl DOMProcessor<ContentMediaItem> for ContentMediaItemProcessor {
    fn process(&self, el: &ElementRef) -> ContentMediaItem {
        ContentMediaItem {
            title: self.title.process(el),
            section: self.section.process(el),
            image: self.image.process(el),
            sources: self.sources.process(el),
            params: self.params.process(el),
        }
    }
}

// text nodes
#[derive(Default)]
pub struct TextValue {
    pub all_nodes: bool,
}

impl DOMProcessor<String> for TextValue {
    fn process(&self, el: &ElementRef) -> String {
        if self.all_nodes {
            el.text().collect::<Vec<_>>().join("")
        } else {
            el.text().next().unwrap_or_default().into()
        }
    }
}

impl TextValue {
    pub fn new() -> TextValue {
        TextValue { all_nodes: false }
    }

    pub fn all_nodes(mut self) -> Self {
        self.all_nodes = true;
        self
    }
}

pub fn text_value(selectors: &str) -> Box<dyn DOMProcessor<String>> {
    TextValue::new()
        .all_nodes()
        .in_scope(selectors)
        .unwrap_or_default()
        .boxed()
}

pub fn text_value_map<Out>(selectors: &str, mapper: fn(String) -> Out) -> Box<dyn DOMProcessor<Out>>
where
    Out: Default + 'static,
{
    TextValue::new()
        .all_nodes()
        .map(mapper)
        .in_scope(selectors)
        .unwrap_or_default()
        .boxed()
}

pub fn optional_text_value(selectors: &str) -> Box<dyn DOMProcessor<Option<String>>> {
    TextValue::new().all_nodes().in_scope(selectors).boxed()
}

pub struct AttrValue {
    pub attr: &'static str,
}

impl DOMProcessor<Option<String>> for AttrValue {
    fn process(&self, el: &ElementRef) -> Option<String> {
        el.attr(self.attr).map(|s| s.into())
    }
}

impl AttrValue {
    pub fn new(attr: &'static str) -> AttrValue {
        AttrValue { attr }
    }
}

pub fn attr_value(selectors: &str, attr: &'static str) -> Box<dyn DOMProcessor<String>> {
    AttrValue::new(attr)
        .in_scope_flatten(selectors)
        .unwrap_or_default()
        .boxed()
}

pub fn attr_value_map<Out>(
    selectors: &str,
    attr: &'static str,
    mapper: fn(String) -> Out,
) -> Box<dyn DOMProcessor<Out>>
where
    Out: Default + 'static,
{
    AttrValue::new(attr)
        .map_optional(mapper)
        .in_scope_flatten(selectors)
        .unwrap_or_default()
        .boxed()
}

// transformation

pub struct ExtractValue<Out> {
    pub extract: Box<dyn Fn(&ElementRef) -> Out + Sync + Send>,
}

impl<Out> DOMProcessor<Out> for ExtractValue<Out> {
    fn process(&self, el: &ElementRef) -> Out {
        (self.extract)(el)
    }
}

impl<Out> ExtractValue<Out> {
    pub fn new<Extract>(extract: Extract) -> ExtractValue<Out>
    where
        Extract: Fn(&ElementRef) -> Out + Sync + Send + 'static,
    {
        ExtractValue {
            extract: Box::new(extract),
        }
    }
}

// map singe value
pub struct MapValue<In, Out> {
    pub map: Box<dyn Fn(In) -> Out + Sync + Send>,
    pub sub_processor: Box<dyn DOMProcessor<In>>,
}

impl<In, Out> DOMProcessor<Out> for MapValue<In, Out> {
    fn process(&self, el: &ElementRef) -> Out {
        let input = self.sub_processor.process(el);
        (self.map)(input)
    }
}

impl<In, Out> MapValue<In, Out> {
    pub fn new<Map>(map: Map, sub_processor: Box<dyn DOMProcessor<In>>) -> MapValue<In, Out>
    where
        Map: Fn(In) -> Out + 'static + Sync + Send,
    {
        MapValue {
            map: Box::new(map),
            sub_processor,
        }
    }
}

// map vector items
pub struct MapItem<In, Out> {
    pub map: Box<dyn Fn(In) -> Out + Sync + Send>,
    pub sub_processor: Box<dyn ItrDOMProcessor<In>>,
}

impl<In, Out> DOMProcessor<Vec<Out>> for MapItem<In, Out> {
    fn process(&self, el: &ElementRef) -> Vec<Out> {
        let items = self.sub_processor.process(el);
        let mut res = vec![];
        for item in items {
            res.push((self.map)(item));
        }
        res
    }
}

impl<In, Out> ItrDOMProcessor<Out> for MapItem<In, Out> {}

impl<In, Out> MapItem<In, Out> {
    pub fn new<Map>(map: Map, sub_processor: Box<dyn ItrDOMProcessor<In>>) -> MapItem<In, Out>
    where
        Map: Fn(In) -> Out + 'static + Sync + Send,
    {
        MapItem {
            map: Box::new(map),
            sub_processor,
        }
    }
}

// lists
pub struct ItemsProcessor<Item> {
    pub scope: Option<Selector>,
    pub item_processor: Box<dyn DOMProcessor<Item>>,
}

impl<Item> DOMProcessor<Vec<Item>> for ItemsProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        if let Some(selector) = &self.scope {
            el.select(selector)
                .map(|e| self.item_processor.process(&e))
                .collect()
        } else {
            el.child_elements()
                .map(|e| self.item_processor.process(&e))
                .collect()
        }
    }
}

impl<Item> ItrDOMProcessor<Item> for ItemsProcessor<Item> {}

impl<Item> ItemsProcessor<Item> {
    pub fn new(scope: &str, item_processor: Box<dyn DOMProcessor<Item>>) -> ItemsProcessor<Item> {
        ItemsProcessor {
            scope: Some(Selector::parse(scope).unwrap()),
            item_processor,
        }
    }
}

pub fn items_processor<Item>(
    scope: &str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> Box<ItemsProcessor<Item>> {
    ItemsProcessor::new(scope, item_processor).into()
}

// join list processors
pub struct JoinProcessors<Item> {
    pub item_processors: Vec<Box<dyn DOMProcessor<Item>>>,
}

impl<Item> DOMProcessor<Vec<Item>> for JoinProcessors<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        let mut res: Vec<Item> = Vec::new();

        for processor in &self.item_processors {
            res.push(processor.process(el))
        }

        res
    }
}

impl<Item> ItrDOMProcessor<Item> for JoinProcessors<Item> {}

impl<Item> Default for JoinProcessors<Item> {
    fn default() -> Self {
        Self {
            item_processors: vec![],
        }
    }
}

// join processors that return same item and return a list of items
impl<Item> JoinProcessors<Item> {
    pub fn new(item_processors: Vec<Box<dyn DOMProcessor<Item>>>) -> JoinProcessors<Item> {
        JoinProcessors { item_processors }
    }

    pub fn add_processor(mut self, processor: Box<dyn DOMProcessor<Item>>) -> Self {
        self.item_processors.push(processor);
        self
    }
}

pub fn join_processors<Item>(
    item_processors: Vec<Box<dyn DOMProcessor<Item>>>,
) -> Box<JoinProcessors<Item>> {
    JoinProcessors::new(item_processors).into()
}

// merge a list of processors that procude a list of items (flatten them)
pub struct MergeItemsProcessor<Item> {
    pub items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
}

impl<Item> DOMProcessor<Vec<Item>> for MergeItemsProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        let mut res: Vec<Item> = Vec::new();

        for processor in &self.items_processors {
            res.append(&mut processor.process(el));
        }

        res
    }
}

impl<Item> Default for MergeItemsProcessor<Item> {
    fn default() -> Self {
        Self {
            items_processors: vec![],
        }
    }
}

impl<Item> MergeItemsProcessor<Item> {
    pub fn new(
        items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
    ) -> MergeItemsProcessor<Item> {
        MergeItemsProcessor { items_processors }
    }

    pub fn add_processor(mut self, processor: Box<dyn DOMProcessor<Vec<Item>>>) -> Self {
        self.items_processors.push(processor);
        self
    }
}

impl<Item> ItrDOMProcessor<Item> for MergeItemsProcessor<Item> {}

pub fn merge<Item>(
    items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
) -> Box<MergeItemsProcessor<Item>> {
    MergeItemsProcessor::new(items_processors).into()
}

// lister processors list
pub struct FilterProcessor<Item> {
    pub predicate: Box<dyn Fn(&Item) -> bool + Sync + Send>,
    pub items_processor: Box<dyn ItrDOMProcessor<Item>>,
}

impl<Item> DOMProcessor<Vec<Item>> for FilterProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        self.items_processor
            .process(el)
            .into_iter()
            .filter(|i| ((self.predicate)(i)))
            .collect()
    }
}

impl<Item> FilterProcessor<Item> {
    pub fn new<Predicate>(
        predicate: Predicate,
        items_processor: Box<dyn ItrDOMProcessor<Item>>,
    ) -> FilterProcessor<Item>
    where
        Predicate: Fn(&Item) -> bool + Sync + Send + 'static,
    {
        FilterProcessor {
            predicate: Box::new(predicate),
            items_processor,
        }
    }
}

impl<Item> ItrDOMProcessor<Item> for FilterProcessor<Item> {}

// scope

pub struct ScopeProcessor<Item> {
    pub scope: Selector,
    pub item_processor: Box<dyn DOMProcessor<Item>>,
}

impl<Item> DOMProcessor<Option<Item>> for ScopeProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Option<Item> {
        el.select(&self.scope)
            .map(|e| self.item_processor.process(&e))
            .next()
    }
}

impl<Item> ScopeProcessor<Item> {
    pub fn new(scope: &str, item_processor: Box<dyn DOMProcessor<Item>>) -> ScopeProcessor<Item> {
        ScopeProcessor {
            scope: Selector::parse(scope).unwrap(),
            item_processor,
        }
    }
}

pub fn scope_processor<Item>(
    scope: &str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> Box<ScopeProcessor<Item>> {
    ScopeProcessor::new(scope, item_processor).into()
}

// utilities

pub struct DefaultValue {}

impl<V: Default> DOMProcessor<V> for DefaultValue {
    fn process(&self, _el: &ElementRef) -> V {
        V::default()
    }
}

impl DefaultValue {
    pub fn new() -> DefaultValue {
        DefaultValue {}
    }
}
pub fn default_value() -> Box<DefaultValue> {
    Box::new(DefaultValue::new())
}

pub fn self_hosted_image(
    url: &'static str,
    selectors: &str,
    attr: &'static str,
) -> Box<dyn DOMProcessor<String>> {
    AttrValue::new(attr)
        .in_scope_flatten(selectors)
        .map_optional(move |src| {
            if src.starts_with("http") {
                return src.to_string();
            }
            format!("{url}{src}")
        })
        .unwrap_or_default()
        .boxed()
}
