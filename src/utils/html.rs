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

impl From<ContentInfoProcessor> for Box<dyn DOMProcessor<ContentInfo>> {
    fn from(value: ContentInfoProcessor) -> Self {
        Box::new(value)
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

impl From<ContentDetailsProcessor> for Box<dyn DOMProcessor<ContentDetails>> {
    fn from(value: ContentDetailsProcessor) -> Self {
        Box::new(value)
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

impl From<TextValue> for Box<dyn DOMProcessor<String>> {
    fn from(value: TextValue) -> Self {
        Box::new(value)
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

    pub fn in_scope(self, selectors: &str) -> ScopeProcessor<String> {
        ScopeProcessor::new(selectors, self.into())
    }

    pub fn itr_scope(self, selectors: &str) -> ItemsProcessor<String> {
        ItemsProcessor::new(selectors, self.into())
    }

    pub fn map<Map, Out>(self, map: Map) -> MapValue<String, Out>
    where
        Map: Fn(String) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }
}

pub fn text_value(selectors: &str) -> Box<dyn DOMProcessor<String>> {
    TextValue::new()
        .in_scope(selectors)
        .unwrap_or_default()
        .into()
}

pub fn optional_text_value(selectors: &str) -> Box<dyn DOMProcessor<Option<String>>> {
    TextValue::new().in_scope(selectors).into()
}

pub struct AttrValue {
    pub attr: &'static str,
}

impl DOMProcessor<String> for AttrValue {
    fn process(&self, el: &ElementRef) -> String {
        el.attr(self.attr).map(|s| s.into()).unwrap_or_default()
    }
}

impl From<AttrValue> for Box<dyn DOMProcessor<String>> {
    fn from(value: AttrValue) -> Self {
        Box::new(value)
    }
}

impl AttrValue {
    pub fn new(attr: &'static str) -> AttrValue {
        AttrValue { attr }
    }

    pub fn in_scope(self, selectors: &str) -> ScopeProcessor<String> {
        ScopeProcessor::new(selectors, self.into())
    }

    pub fn itr_scope(self, selectors: &str) -> ItemsProcessor<String> {
        ItemsProcessor::new(selectors, self.into())
    }

    pub fn map<Map, Out>(self, map: Map) -> MapValue<String, Out>
    where
        Map: Fn(String) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }
}

pub fn attr_value(selectors: &str, attr: &'static str) -> Box<dyn DOMProcessor<String>> {
    AttrValue::new(attr)
        .in_scope(selectors)
        .unwrap_or_default()
        .into()
}

pub fn optional_attr_value(
    attr: &'static str,
    selectors: &str,
) -> Box<dyn DOMProcessor<Option<String>>> {
    AttrValue::new(attr).in_scope(selectors).into()
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

impl<Out: 'static> From<ExtractValue<Out>> for Box<dyn DOMProcessor<Out>> {
    fn from(value: ExtractValue<Out>) -> Self {
        Box::new(value)
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

impl<Out: 'static> ExtractValue<Out> {
    pub fn in_scope(self, selectors: &str) -> ScopeProcessor<Out> {
        ScopeProcessor::new(selectors, self.into())
    }

    pub fn itr_scope(self, selectors: &str) -> ItemsProcessor<Out> {
        ItemsProcessor::new(selectors, self.into())
    }
}

impl<Item: Default + 'static> ExtractValue<Item> {
    pub fn map<Map, Out>(self, map: Map) -> MapValue<Item, Out>
    where
        Map: Fn(Item) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }
}

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

impl<In: 'static, Out: 'static> From<MapValue<In, Out>> for Box<dyn DOMProcessor<Out>> {
    fn from(value: MapValue<In, Out>) -> Self {
        Box::new(value)
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

impl<A: 'static, B: 'static> MapValue<A, B> {
    pub fn map<Map, C>(self, map: Map) -> MapValue<B, C>
    where
        Map: Fn(B) -> C + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }

    pub fn in_scope(self, selectors: &str) -> ScopeProcessor<B> {
        ScopeProcessor::new(selectors, self.into())
    }
}

impl<A: 'static, B: 'static> MapValue<Option<A>, Option<B>> {
    pub fn map_optional<Map, C>(self, map: Map) -> MapValue<Option<B>, Option<C>>
    where
        Map: Fn(B) -> C + 'static + Copy + Sync + Send,
    {
        MapValue::new(move |opt| opt.map(map), self.into())
    }
}

impl<A: 'static, B: Default + 'static> MapValue<A, Option<B>> {
    pub fn flatten(self) -> MapValue<Option<B>, B> {
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

impl<Item: 'static> From<ItemsProcessor<Item>> for Box<dyn DOMProcessor<Vec<Item>>> {
    fn from(value: ItemsProcessor<Item>) -> Self {
        Box::new(value)
    }
}

impl<Item> ItemsProcessor<Item> {
    pub fn new(scope: &str, item_processor: Box<dyn DOMProcessor<Item>>) -> ItemsProcessor<Item> {
        ItemsProcessor {
            scope: Selector::parse(scope).unwrap(),
            item_processor,
        }
    }
}

impl<Item: 'static> ItemsProcessor<Item> {
    pub fn map<Map, Out>(self, map: Map) -> MapValue<Vec<Item>, Out>
    where
        Map: Fn(Vec<Item>) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }

    pub fn filter<Predicate>(self, predicate: Predicate) -> FilterProcessor<Item>
    where
        Predicate: Fn(&Item) -> bool + 'static + Sync + Send,
    {
        FilterProcessor::new(predicate, self.into())
    }
}

pub fn items_processor<Item>(
    scope: &str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> Box<ItemsProcessor<Item>> {
    ItemsProcessor::new(scope, item_processor).into()
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

impl<Item: 'static> From<JoinProcessors<Item>> for Box<dyn DOMProcessor<Vec<Item>>> {
    fn from(value: JoinProcessors<Item>) -> Self {
        Box::new(value)
    }
}

impl<Item> Default for JoinProcessors<Item> {
    fn default() -> Self {
        Self {
            item_processors: vec![],
        }
    }
}

impl<Item> JoinProcessors<Item> {
    pub fn new(item_processors: Vec<Box<dyn DOMProcessor<Item>>>) -> JoinProcessors<Item> {
        JoinProcessors { item_processors }
    }

    pub fn add_processor(mut self, processor: Box<dyn DOMProcessor<Item>>) -> Self {
        self.item_processors.push(processor);
        self
    }
}

impl<Item: 'static> JoinProcessors<Item> {
    pub fn map<Map, Out>(self, map: Map) -> MapValue<Vec<Item>, Out>
    where
        Map: Fn(Vec<Item>) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }

    pub fn filter<Predicate>(self, predicate: Predicate) -> FilterProcessor<Item>
    where
        Predicate: Fn(&Item) -> bool + 'static + Sync + Send,
    {
        FilterProcessor::new(predicate, self.into())
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

impl<Item> Default for FlattenProcessor<Item> {
    fn default() -> Self {
        Self {
            items_processors: vec![],
        }
    }
}

impl<Item> FlattenProcessor<Item> {
    pub fn new(items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>) -> FlattenProcessor<Item> {
        FlattenProcessor { items_processors }
    }

    pub fn add_processor(mut self, processor: Box<dyn DOMProcessor<Vec<Item>>>) -> Self {
        self.items_processors.push(processor);
        self
    }
}

impl<Item: 'static> From<FlattenProcessor<Item>> for Box<dyn DOMProcessor<Vec<Item>>> {
    fn from(value: FlattenProcessor<Item>) -> Self {
        Box::new(value)
    }
}

impl<Item: 'static> FlattenProcessor<Item> {
    pub fn map<Map, Out>(self, map: Map) -> MapValue<Vec<Item>, Out>
    where
        Map: Fn(Vec<Item>) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }

    pub fn filter<Predicate>(self, predicate: Predicate) -> FilterProcessor<Item>
    where
        Predicate: Fn(&Item) -> bool + 'static + Sync + Send,
    {
        FilterProcessor::new(predicate, self.into())
    }
}

pub fn flatten<Item>(
    items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
) -> Box<FlattenProcessor<Item>> {
    FlattenProcessor::new(items_processors).into()
}

pub struct FilterProcessor<Item> {
    pub predicate: Box<dyn Fn(&Item) -> bool + Sync + Send>,
    pub items_processor: Box<dyn DOMProcessor<Vec<Item>>>,
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

impl<Item: 'static> From<FilterProcessor<Item>> for Box<dyn DOMProcessor<Vec<Item>>> {
    fn from(value: FilterProcessor<Item>) -> Self {
        Box::new(value)
    }
}

impl<Item> FilterProcessor<Item> {
    pub fn new<Predicate>(
        predicate: Predicate,
        items_processor: Box<dyn DOMProcessor<Vec<Item>>>,
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

impl<Item: 'static> FilterProcessor<Item> {
    pub fn map<Map, Out>(self, map: Map) -> MapValue<Vec<Item>, Out>
    where
        Map: Fn(Vec<Item>) -> Out + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }

    pub fn filter<Predicate>(self, predicate: Predicate) -> FilterProcessor<Item>
    where
        Predicate: Fn(&Item) -> bool + 'static + Sync + Send,
    {
        FilterProcessor::new(predicate, self.into())
    }
}

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

impl<Item: 'static> From<ScopeProcessor<Item>> for Box<dyn DOMProcessor<Option<Item>>> {
    fn from(value: ScopeProcessor<Item>) -> Self {
        Box::new(value)
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

impl<Item: Default + 'static> ScopeProcessor<Item> {
    pub fn unwrap_or_default(self) -> MapValue<Option<Item>, Item> {
        MapValue::new(|opt| opt.unwrap_or_default(), self.into())
    }
}

impl<Item: 'static> ScopeProcessor<Item> {
    pub fn map<Map, Out>(self, map: Map) -> MapValue<Option<Item>, Option<Out>>
    where
        Map: Fn(Option<Item>) -> Option<Out> + 'static + Sync + Send,
    {
        MapValue::new(map, self.into())
    }

    pub fn map_optional<Map, Out>(self, map: Map) -> MapValue<Option<Item>, Option<Out>>
    where
        Map: Fn(Item) -> Out + 'static + Copy + Sync + Send,
    {
        MapValue::new(move |opt| opt.map(map), self.into())
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

//pub fn default_value<V: Default>() -> Box<dyn DOMProcessor<V>> {
//Box::new(DefaultValue::new())
//}

pub fn default_value() -> Box<DefaultValue> {
    Box::new(DefaultValue::new())
}

pub fn sanitize_text(text: &str) -> String {
    static SANITIZE_TEXT_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let re = SANITIZE_TEXT_REGEXP.get_or_init(|| Regex::new(r#"[\n\t\s]+"#).unwrap());

    re.replace_all(text, " ").into_owned().trim().into()
}

pub fn strip_html(text: &str) -> String {
    static STRIP_HTML_REGEXP: OnceLock<regex::Regex> = OnceLock::new();
    let re = STRIP_HTML_REGEXP.get_or_init(|| Regex::new(r#"<[^>]*>"#).unwrap());

    re.replace_all(text, "").into_owned().trim().into()
}

pub fn self_hosted_image(
    url: &'static str,
    selectors: &str,
    attr: &'static str,
) -> Box<dyn DOMProcessor<String>> {
    AttrValue::new(attr)
        .in_scope(selectors)
        .map_optional(move |src| format!("{url}{src}"))
        .flatten()
        .into()
}
