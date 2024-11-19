#![allow(unused)]

use std::{borrow::Cow, str, sync::OnceLock};

use regex::Regex;
use scraper::{ElementRef, Selector};

use crate::models::{ContentDetails, ContentInfo, MediaType};

// base
pub trait DOMProcessor<T>: Sync + Send {
    fn process(&self, el: &ElementRef) -> T;
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

impl Into<Box<dyn DOMProcessor<ContentInfo>>> for ContentInfoProcessor {
    fn into(self) -> Box<dyn DOMProcessor<ContentInfo>> {
        Box::new(self)
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
            similar: self.similar.process(el),
            params: self.params.process(el),
        }
    }
}

impl Into<Box<dyn DOMProcessor<ContentDetails>>> for ContentDetailsProcessor {
    fn into(self) -> Box<dyn DOMProcessor<ContentDetails>> {
        Box::new(self)
    }
}

// text nodes
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

impl Into<Box<dyn DOMProcessor<String>>> for TextValue {
    fn into(self) -> Box<dyn DOMProcessor<String>> {
        Box::new(self)
    }
}

impl TextValue {
    pub fn new() -> TextValue {
        TextValue { all_nodes: false }
    }

    pub fn all(mut self) -> Self {
        self.all_nodes = true;
        self
    }

    pub fn scoped(self, selectors: &'static str) -> ScopedProcessor<String> {
        ScopedProcessor::new(selectors, self.into())
    }

    pub fn map<Map: 'static + Sync + Send, Out>(self, map: Map) -> MapValue<String, Out>
    where
        Map: Fn(String) -> Out,
    {
        MapValue::new(map, self.into())
    }
}

pub fn text_value(selectors: &'static str) -> Box<dyn DOMProcessor<String>> {
    TextValue::new().scoped(selectors).unwrap().into()
}

pub fn optional_text_value(selectors: &'static str) -> Box<dyn DOMProcessor<Option<String>>> {
    TextValue::new().scoped(selectors).into()
}

pub struct AttrValue {
    pub attr: &'static str,
}

impl DOMProcessor<String> for AttrValue {
    fn process(&self, el: &ElementRef) -> String {
        el.attr(&self.attr).map(|s| s.into()).unwrap_or_default()
    }
}

impl Into<Box<dyn DOMProcessor<String>>> for AttrValue {
    fn into(self) -> Box<dyn DOMProcessor<String>> {
        Box::new(self)
    }
}

impl AttrValue {
    pub fn new(attr: &'static str) -> AttrValue {
        AttrValue { attr }
    }

    pub fn scoped(self, selectors: &'static str) -> ScopedProcessor<String> {
        ScopedProcessor::new(selectors, self.into())
    }

    pub fn map<Map: 'static + Sync + Send, Out>(self, map: Map) -> MapValue<String, Out>
    where
        Map: Fn(String) -> Out,
    {
        MapValue::new(map, self.into())
    }
}

pub fn attr_value(attr: &'static str, selectors: &'static str) -> Box<dyn DOMProcessor<String>> {
    AttrValue::new(attr).scoped(selectors).unwrap().into()
}

pub fn optional_attr_value(
    attr: &'static str,
    selectors: &'static str,
) -> Box<dyn DOMProcessor<Option<String>>> {
    AttrValue::new(attr).scoped(selectors).into()
}

// transformation

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

impl<In: 'static, Out: 'static> Into<Box<dyn DOMProcessor<Out>>> for MapValue<In, Out> {
    fn into(self) -> Box<dyn DOMProcessor<Out>> {
        Box::new(self)
    }
}

impl<In, Out> MapValue<In, Out> {
    pub fn new<Map: 'static + Sync + Send>(
        map: Map,
        sub_processor: Box<dyn DOMProcessor<In>>,
    ) -> MapValue<In, Out>
    where
        Map: Fn(In) -> Out,
    {
        MapValue {
            map: Box::new(map),
            sub_processor,
        }
    }
}

impl<A: 'static, B: 'static> MapValue<A, B> {
    pub fn map<Map: 'static + Sync + Send, C>(self, map: Map) -> MapValue<B, C>
    where
        Map: Fn(B) -> C,
    {
        MapValue::new(map, self.into())
    }
}

impl<A: 'static, B: 'static> MapValue<Option<A>, Option<B>> {
    pub fn map_optional<Map: 'static + Copy + Sync + Send, C>(
        self,
        map: Map,
    ) -> MapValue<Option<B>, Option<C>>
    where
        Map: Fn(B) -> C,
    {
        MapValue::new(move |opt| opt.map(map), self.into())
    }
}

impl<A: 'static, B: Default + 'static> MapValue<A, Option<B>> {
    pub fn unwrap(self) -> MapValue<Option<B>, B> {
        MapValue::new(|opt| opt.unwrap_or_default(), self.into())
    }
}

// lists
pub struct ItemsProcessor<Item> {
    pub scope: Selector,
    pub item_processor: Box<dyn DOMProcessor<Item>>,
}

impl<Item> DOMProcessor<Vec<Item>> for ItemsProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        el.select(&self.scope)
            .map(|e| self.item_processor.process(&e))
            .collect()
    }
}

impl<Item: 'static> Into<Box<dyn DOMProcessor<Vec<Item>>>> for ItemsProcessor<Item> {
    fn into(self) -> Box<dyn DOMProcessor<Vec<Item>>> {
        Box::new(self)
    }
}

impl<Item> ItemsProcessor<Item> {
    pub fn new(
        scope: &'static str,
        item_processor: Box<dyn DOMProcessor<Item>>,
    ) -> ItemsProcessor<Item> {
        ItemsProcessor {
            scope: Selector::parse(scope).unwrap(),
            item_processor,
        }
    }
}

impl<Item: 'static> ItemsProcessor<Item> {
    pub fn map<Map: 'static + Sync + Send, Out>(self, map: Map) -> MapValue<Vec<Item>, Out>
    where
        Map: Fn(Vec<Item>) -> Out,
    {
        MapValue::new(map, self.into())
    }
}

pub fn item_processor<Item>(
    scope: &'static str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> Box<ItemsProcessor<Item>> {
    ItemsProcessor {
        scope: Selector::parse(scope).unwrap(),
        item_processor,
    }
    .into()
}
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

impl<Item: 'static> Into<Box<dyn DOMProcessor<Vec<Item>>>> for JoinProcessors<Item> {
    fn into(self) -> Box<dyn DOMProcessor<Vec<Item>>> {
        Box::new(self)
    }
}


impl<Item> JoinProcessors<Item> {
    pub fn new(item_processors: Vec<Box<dyn DOMProcessor<Item>>>) -> JoinProcessors<Item> {
        JoinProcessors { item_processors }
    }
}

impl<Item: 'static> JoinProcessors<Item> {
    pub fn map<Map: 'static + Sync + Send, Out>(self, map: Map) -> MapValue<Vec<Item>, Out>
    where
        Map: Fn(Vec<Item>) -> Out,
    {
        MapValue::new(map, self.into())
    }
}

pub fn join_processors<Item>(
    item_processors: Vec<Box<dyn DOMProcessor<Item>>>,
) -> Box<JoinProcessors<Item>> {
    JoinProcessors::new(item_processors).into()
}

pub struct FlattenProcessor<Item> {
    pub items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
}

impl<Item> DOMProcessor<Vec<Item>> for FlattenProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        let mut res: Vec<Item> = Vec::new();

        for processor in &self.items_processors {
            res.append(&mut processor.process(el));
        }

        res
    }
}

impl<Item> FlattenProcessor<Item> {
    pub fn new(items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,) -> FlattenProcessor<Item> {
        FlattenProcessor { items_processors }
    }
}

pub fn flatten<Item>(
    items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
) -> Box<FlattenProcessor<Item>> {
    FlattenProcessor::new(items_processors).into()
}

// scoping

pub struct ScopedProcessor<Item> {
    pub scope: Selector,
    pub item_processor: Box<dyn DOMProcessor<Item>>,
}

impl<Item> DOMProcessor<Option<Item>> for ScopedProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Option<Item> {
        el.select(&self.scope)
            .map(|e| self.item_processor.process(&e))
            .next()
    }
}

impl<Item: 'static> Into<Box<dyn DOMProcessor<Option<Item>>>> for ScopedProcessor<Item> {
    fn into(self) -> Box<dyn DOMProcessor<Option<Item>>> {
        Box::new(self)
    }
}

impl<Item> ScopedProcessor<Item> {
    pub fn new(
        scope: &'static str,
        item_processor: Box<dyn DOMProcessor<Item>>,
    ) -> ScopedProcessor<Item> {
        ScopedProcessor {
            scope: Selector::parse(scope).unwrap(),
            item_processor,
        }
    }
}

impl<Item: Default + 'static> ScopedProcessor<Item> {
    pub fn unwrap(self) -> MapValue<Option<Item>, Item> {
        MapValue::new(|opt| opt.unwrap_or_default(), self.into())
    }

    pub fn map<Map: 'static + Sync + Send, Out>(
        self,
        map: Map,
    ) -> MapValue<Option<Item>, Option<Out>>
    where
        Map: Fn(Option<Item>) -> Option<Out>,
    {
        MapValue::new(map, self.into())
    }

    pub fn map_optional<Map: 'static + Copy + Sync + Send, Out>(
        self,
        map: Map,
    ) -> MapValue<Option<Item>, Option<Out>>
    where
        Map: Fn(Item) -> Out,
    {
        MapValue::new(move |opt| opt.map(map), self.into())
    }
}

pub fn scoped_processor<Item>(
    scope: &'static str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> Box<ScopedProcessor<Item>> {
    ScopedProcessor::new(scope, item_processor).into()
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

pub fn default_value<V: Default>() -> Box<DefaultValue> {
    Box::new(DefaultValue::new())
}

pub fn sanitize_text<'h>(text: String) -> String {
    static SANITIZE_TEXT_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let re = SANITIZE_TEXT_REGEXP.get_or_init(|| Regex::new(r#"[\n\t\s]+"#).unwrap());

    re.replace_all(&text, " ").into_owned().trim().into()
}

pub fn self_hosted_image(
    url: &'static str,
    selectors: &'static str,
    attr: &'static str,
) -> Box<dyn DOMProcessor<String>> {
    AttrValue::new(attr)
        .scoped(selectors)
        .map_optional(move |src| format!("{url}{src}"))
        .unwrap()
        .into()
}
