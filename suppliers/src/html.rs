#![allow(unused)]

use std::str;

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

pub struct ContentDetailsProcessor {
    pub media_type: MediaType,
    pub title: Box<dyn DOMProcessor<String>>,
    pub original_title: Box<dyn DOMProcessor<Option<String>>>,
    pub image: Box<dyn DOMProcessor<Option<String>>>,
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
            image: self.image.process(el).unwrap_or_default(),
            description: self.description.process(el),
            additional_info: self.additional_info.process(el),
            similar: self.similar.process(el),
            params: self.params.process(el),
        }
    }
}

// text nodes

pub struct TextValue {
    pub selector: Selector,
}

impl DOMProcessor<String> for TextValue {
    fn process(&self, el: &ElementRef) -> String {
        el.select(&self.selector)
            .next()
            .map(|e| e.text().collect::<Vec<_>>().join(""))
            .unwrap_or_default()
    }
}

pub fn text_value(selectors: &'static str) -> Box<TextValue> {
    Box::new(TextValue {
        selector: Selector::parse(selectors).unwrap(),
    })
}

pub struct TextOptionalValue {
    pub selector: Selector,
}

impl DOMProcessor<Option<String>> for TextOptionalValue {
    fn process(&self, el: &ElementRef) -> Option<String> {
        el.select(&self.selector)
            .next()
            .map(|e| e.text().collect::<Vec<_>>().join(""))
    }
}

pub fn optional_text_value(selectors: &'static str) -> Box<TextOptionalValue> {
    Box::new(TextOptionalValue {
        selector: Selector::parse(selectors).unwrap(),
    })
}

pub struct IterTextValues {
    pub selector: Selector,
}

impl DOMProcessor<Vec<String>> for IterTextValues {
    fn process(&self, el: &ElementRef) -> Vec<String> {
        el.select(&self.selector)
            .map(|e| e.text().collect::<Vec<_>>().join(""))
            .collect()
    }
}

pub fn iter_text_values(selectors: &'static str) -> Box<IterTextValues> {
    Box::new(IterTextValues {
        selector: Selector::parse(selectors).unwrap(),
    })
}

// attributes

pub struct AttrOptionalValue {
    pub attr: &'static str,
    pub selector: Selector,
}

impl DOMProcessor<Option<String>> for AttrOptionalValue {
    fn process(&self, el: &ElementRef) -> Option<String> {
        el.select(&self.selector)
            .next()
            .and_then(|e| e.attr(&self.attr))
            .map(|s| String::from(s))
    }
}

pub fn optional_attr_value(attr: &'static str, selectors: &'static str) -> Box<AttrOptionalValue> {
    Box::new(AttrOptionalValue {
        attr,
        selector: Selector::parse(selectors).unwrap(),
    })
}

pub struct AttrValue {
    pub attr: &'static str,
    pub selector: Selector,
}

impl DOMProcessor<String> for AttrValue {
    fn process(&self, el: &ElementRef) -> String {
        el.select(&self.selector)
            .next()
            .and_then(|e| e.attr(&self.attr))
            .map(|s| String::from(s))
            .unwrap_or_default()
    }
}

pub fn attr_value(attr: &'static str, selectors: &'static str) -> Box<AttrValue> {
    Box::new(AttrValue {
        attr,
        selector: Selector::parse(selectors).unwrap(),
    })
}

pub struct IterAttrValues {
    pub attr: &'static str,
    pub selector: Selector,
}

impl DOMProcessor<Vec<String>> for IterAttrValues {
    fn process(&self, el: &ElementRef) -> Vec<String> {
        el.select(&self.selector)
            .map(|el| el.attr(&self.attr))
            .flatten()
            .map(|s| String::from(s))
            .collect()
    }
}

pub fn iter_attr_values(attr: &'static str, selectors: &'static str) -> Box<IterAttrValues> {
    Box::new(IterAttrValues {
        attr,
        selector: Selector::parse(selectors).unwrap(),
    })
}


// transformation

pub struct MapValue<In, Out> {
    pub map: fn(In) -> Out,
    pub sub_processor: Box<dyn DOMProcessor<In>>,
}

impl<In, Out> DOMProcessor<Out> for MapValue<In, Out> {
    fn process(&self, el: &ElementRef) -> Out {
        let input = self.sub_processor.process(el);
        (self.map)(input)
    }
}

pub fn map_value<In, Out>(
    map: fn(In) -> Out,
    sub_processor: Box<dyn DOMProcessor<In>>,
) -> Box<MapValue<In, Out>> {
    Box::new(MapValue { map, sub_processor })
}

pub struct MapOptionalValue<In, Out> {
    pub map: fn(In) -> Out,
    pub sub_processor: Box<dyn DOMProcessor<Option<In>>>,
}

impl<In, Out> DOMProcessor<Option<Out>> for MapOptionalValue<In, Out> {
    fn process(&self, el: &ElementRef) -> Option<Out> {
        self.sub_processor
            .process(el)
            .map(|input| (self.map)(input))
    }
}

pub fn optional_map_value<In, Out>(
    map: fn(In) -> Out,
    sub_processor: Box<dyn DOMProcessor<Option<In>>>,
) -> Box<MapOptionalValue<In, Out>> {
    Box::new(MapOptionalValue { map, sub_processor })
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

pub fn items_processor_raw<Item>(
    scope: &'static str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> ItemsProcessor<Item> {
    ItemsProcessor {
        scope: Selector::parse(scope).unwrap(),
        item_processor,
    }
}

pub fn items_processor<Item>(
    scope: &'static str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> Box<ItemsProcessor<Item>> {
    Box::new(items_processor_raw(scope, item_processor))
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

pub fn join_processors<Item>(
    item_processors: Vec<Box<dyn DOMProcessor<Item>>>,
) -> Box<JoinProcessors<Item>> {
    Box::new(JoinProcessors { item_processors })
}

pub struct ConcatProcessor<Item> {
    pub items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
}

impl<Item> DOMProcessor<Vec<Item>> for ConcatProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        let mut res: Vec<Item> = Vec::new();

        for processor in &self.items_processors {
            res.append(&mut processor.process(el));
        }

        res
    }
}

pub fn concat<Item>(
    items_processors: Vec<Box<dyn DOMProcessor<Vec<Item>>>>,
) -> Box<ConcatProcessor<Item>> {
    Box::new(ConcatProcessor { items_processors })
}

pub struct FilterProcessor<Item> {
    pub filter: fn(&Item) -> bool,
    pub items_processor: Box<dyn DOMProcessor<Vec<Item>>>,
}

impl<Item> DOMProcessor<Vec<Item>> for FilterProcessor<Item> {
    fn process(&self, el: &ElementRef) -> Vec<Item> {
        self.items_processor
            .process(el)
            .into_iter()
            .filter(|i| ((self.filter)(&i)))
            .collect()
    }
}

pub fn filter<Item>(
    filter: fn(&Item) -> bool,
    items_processor: Box<dyn DOMProcessor<Vec<Item>>>,
) -> Box<FilterProcessor<Item>> {
    Box::new(FilterProcessor {
        filter,
        items_processor
    })
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

pub fn scoped_processor<Item>(
    scope: &'static str,
    item_processor: Box<dyn DOMProcessor<Item>>,
) -> ScopedProcessor<Item> {
    ScopedProcessor {
        scope: Selector::parse(scope).unwrap(),
        item_processor,
    }
}

// utiliti

pub struct DefaultValue {}

impl<V: Default> DOMProcessor<V> for DefaultValue {
    fn process(&self, _el: &ElementRef) -> V {
        V::default()
    }
}

pub fn default_value<V: Default>() -> Box<DefaultValue> {
    Box::new(DefaultValue {})
}
