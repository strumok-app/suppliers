mod schema_generated;

use crate::schema_generated::proto;
use flatbuffers::{FlatBufferBuilder, ForwardsUOffset, Vector, WIPOffset};
use lazy_static::lazy_static;
use std::ffi::{c_char, CString};
use std::ptr::null;
use std::slice;
use std::{collections::HashMap, error::Error};
use strum_macros::FromRepr;
use suppliers::models::ContentSupplier;
use suppliers::{avalaible_suppliers, get_supplier, models, AllContentSuppliers};
use tokio::runtime::Runtime as TokioRuntime;

lazy_static! {
    static ref RT: TokioRuntime = TokioRuntime::new().unwrap();
}

// wire
#[derive(FromRepr, Debug)]
#[repr(u8)]
enum Command {
    SuppliersList = 0,
    Channels,
    DefaultChannels,
    SupportedTypes,
    SupportedLanguges,
    LoadChannel,
    Search,
    GetContentDetails,
    LoadMediaItems,
    LoadMediaItemSources,
}

#[repr(C)]
pub struct WireResult {
    pub size: usize,
    pub buf: *const u8,
    pub error: *const c_char,
}

impl From<Result<Vec<u8>, Box<dyn Error>>> for WireResult {
    fn from(value: Result<Vec<u8>, Box<dyn Error>>) -> Self {
        match value {
            Ok(result) => {
                let size = result.len();
                let res = result.into_boxed_slice();
                return WireResult {
                    size: size,
                    buf: Box::into_raw(res) as *const _,
                    error: null(),
                };
            }
            Err(err) => {
                println!("Commnad Processing failed: {}", err);
                return WireResult {
                    size: 0,
                    buf: null(),
                    error: CString::new(err.to_string()).unwrap().into_raw(),
                };
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn wire(
    cmd_index: u8,
    size: usize,
    req: *const u8,
    callback: unsafe extern "C" fn(WireResult),
) {
    let req_vec = read_request(size, req);
    RT.spawn(async move {
        let result = process_command(cmd_index, req_vec).await;

        unsafe {
            callback(result.into());
        }
    });
}

#[no_mangle]
pub extern "C" fn wire_sync(cmd_index: u8, size: usize, req: *const u8) -> WireResult {
    let req_vec = read_request(size, req);
    let result = process_sync_command(cmd_index, req_vec);

    result.into()
}

#[no_mangle]
pub extern "C" fn free(res: WireResult) {
    unsafe {
        if !res.buf.is_null() {
            _ = Box::from_raw(res.buf as *mut u8);
        }

        if !res.error.is_null() {
            _ = CString::from_raw(res.buf as *mut c_char);
        }
    }
}

// wire methods

fn process_sync_command(cmd_index: u8, req: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    match Command::from_repr(cmd_index) {
        Some(Command::SuppliersList) => suppliers_list(),
        Some(Command::Channels) => get_channels(req),
        Some(Command::DefaultChannels) => get_default_channels(req),
        Some(Command::SupportedTypes) => get_supportes_types(req),
        Some(Command::SupportedLanguges) => get_supportes_lanuages(req),
        Some(_) => Result::Err(format!("Unknown Sync command index {}", cmd_index).into()),
        None => Result::Err(format!("Unknown command index {}", cmd_index).into()),
    }
}

async fn process_command(cmd_index: u8, req: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    match Command::from_repr(cmd_index) {
        Some(Command::LoadChannel) => load_channels(req).await,
        Some(Command::Search) => search(req).await,
        Some(Command::GetContentDetails) => get_content_detail(req).await,
        Some(Command::LoadMediaItems) => load_media_items(req).await,
        Some(Command::LoadMediaItemSources) => load_media_item_sources(req).await,
        Some(_) => Result::Err(format!("Unknown Async command index {}", cmd_index).into()),
        None => Result::Err(format!("Unknown command index {}", cmd_index).into()),
    }
}

fn read_request(size: usize, req: *const u8) -> Vec<u8> {
    if req.is_null() {
        return vec![];
    }

    let bytes = unsafe { slice::from_raw_parts(req, size) };

    Vec::from(bytes)
}

// protocol commands handlers
fn suppliers_list() -> Result<Vec<u8>, Box<dyn Error>> {
    let suppliers = avalaible_suppliers();

    // convert to fb
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();

    let fb_channels = from_str_vec_to_fb(fbb, &suppliers);

    let fb_args = proto::SuppliersResArgs {
        suppliers: Some(fbb.create_vector_from_iter(fb_channels.iter())),
        ..Default::default()
    };

    let fb_suppliers = proto::SuppliersRes::create(fbb, &fb_args);
    fbb.finish(fb_suppliers, None);
    Ok(fbb.finished_data().to_vec())
}

fn get_channels(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::SupplierNameReq>(&request)?;

    let supplier = get_supplier(req.supplier())?;

    let channels = AllContentSuppliers::get_channels(&supplier);

    Ok(create_channels_res(channels))
}

fn get_default_channels(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::SupplierNameReq>(&request)?;

    let supplier = get_supplier(req.supplier())?;

    let channels = AllContentSuppliers::get_default_channels(&supplier);

    Ok(create_channels_res(channels))
}

fn get_supportes_types(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::SupplierNameReq>(&request)?;

    let supplier = get_supplier(&req.supplier())?;

    let types = AllContentSuppliers::get_supported_types(&supplier);

    // convert to fb
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();
    let fb_types: Vec<_> = types.iter().map(|&t| proto::ContentType(t as u8)).collect();

    let fb_args = proto::SupportedTypesResArgs {
        types: Some(fbb.create_vector_from_iter(fb_types.iter())),
    };

    let fb_suppliers = proto::SupportedTypesRes::create(fbb, &fb_args);

    fbb.finish(fb_suppliers, None);
    Ok(fbb.finished_data().to_vec())
}

fn get_supportes_lanuages(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::SupplierNameReq>(&request)?;

    let supplier = get_supplier(&req.supplier())?;

    let languages = AllContentSuppliers::get_supported_languages(&supplier);

    // convert to fb
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();
    let fb_languages = from_str_vec_to_fb(fbb, &languages);

    let fb_args = proto::SupportedLanuagesResArgs {
        langs: Some(fbb.create_vector_from_iter(fb_languages.iter())),
    };

    let fb_suppliers = proto::SupportedLanuagesRes::create(fbb, &fb_args);

    fbb.finish(fb_suppliers, None);
    Ok(fbb.finished_data().to_vec())
}

async fn load_channels(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::LoadChannelsReq>(&request)?;

    let supplier = get_supplier(&req.supplier())?;

    let items = AllContentSuppliers::load_channel(&supplier, &req.channel(), req.page()).await?;

    // convert to fb
    create_content_info_res(items)
}

async fn search(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::SearchReq>(&request)?;

    let supplier = get_supplier(&req.supplier())?;

    let types: Vec<_> = req
        .types()
        .unwrap()
        .iter()
        .map(|t| suppliers::models::ContentType::from_repr(t.0).unwrap())
        .collect();

    let items = AllContentSuppliers::search(&supplier, req.query(), types).await?;

    // convert to fb
    create_content_info_res(items)
}

async fn get_content_detail(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::ContentDetailsReq>(&request)?;

    let supplier = get_supplier(&req.supplier())?;
    let maybe_details = AllContentSuppliers::get_content_details(&supplier, req.id()).await?;

    // convert to fb
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();

    let fb_args = match maybe_details {
        Some(details) => {
            let args = create_fb_content_details(fbb, &details);
            let fb_details = proto::ContentDetails::create(fbb, &args);
            proto::ContentDetailsResArgs {
                details: Some(fb_details),
            }
        }
        None => proto::ContentDetailsResArgs {
            ..Default::default()
        },
    };

    let fb_content_details = proto::ContentDetailsRes::create(fbb, &fb_args);

    fbb.finish(fb_content_details, None);
    Ok(fbb.finished_data().to_vec())
}

async fn load_media_items(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::LoadMediaItemsReq>(&request)?;

    let supplier = get_supplier(&req.supplier())?;
    let params = req.params().iter().map(|s| s.to_string()).collect();

    let media_items = AllContentSuppliers::load_media_items(&supplier, req.id(), params).await?;

    // convert to fb
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();
    let fb_media_items: Vec<_> = media_items
        .iter()
        .map(|item| {
            let fb_args = create_fb_media_item(fbb, item);
            proto::ContentMediaItem::create(fbb, &fb_args)
        })
        .collect();

    let fb_args = proto::LoadMediaItemsResArgs {
        media_items: Some(fbb.create_vector_from_iter(fb_media_items.iter())),
    };

    let fb_media_items_res = proto::LoadMediaItemsRes::create(fbb, &fb_args);

    fbb.finish(fb_media_items_res, None);
    Ok(fbb.finished_data().to_vec())
}

async fn load_media_item_sources(request: Vec<u8>) -> Result<Vec<u8>, Box<dyn Error>> {
    let req = flatbuffers::root::<proto::LoadMediaItemSourcesReq>(&request)?;

    let supplier = get_supplier(&req.supplier())?;
    let params = req.params().iter().map(|s| s.to_string()).collect();

    let sources = AllContentSuppliers::load_media_item_sources(&supplier, req.id(), params).await?;

    // convert to fb
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();

    let fb_sources: Vec<_> = sources
        .iter()
        .map(|s| {
            let fb_args = create_fb_media_items_source(fbb, s);
            proto::ContentMediaItemSource::create(fbb, &fb_args)
        })
        .collect();

    let fb_args = proto::LoadMediaItemSourcesResArgs {
        sources: Some(fbb.create_vector_from_iter(fb_sources.iter())),
    };

    let fb_media_items_res = proto::LoadMediaItemSourcesRes::create(fbb, &fb_args);

    fbb.finish(fb_media_items_res, None);
    Ok(fbb.finished_data().to_vec())
}

// convert methods

fn create_content_info_res(
    items: Vec<suppliers::models::ContentInfo>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();
    let fb_items: Vec<_> = items
        .iter()
        .map(|item| {
            let args = &create_fb_content_info(fbb, item);
            proto::ContentInfo::create(fbb, args)
        })
        .collect();

    let fb_args = proto::ContentInfoResArgs {
        items: Some(fbb.create_vector_from_iter(fb_items.iter())),
    };

    let fb_content_info_res = proto::ContentInfoRes::create(fbb, &fb_args);

    fbb.finish(fb_content_info_res, None);
    Ok(fbb.finished_data().to_vec())
}

fn create_channels_res(channels: Vec<&str>) -> Vec<u8> {
    let fbb = &mut flatbuffers::FlatBufferBuilder::new();
    let fb_channels = from_str_vec_to_fb(fbb, &channels);

    let fb_args = proto::ChannelsResArgs {
        channels: Some(fbb.create_vector_from_iter(fb_channels.iter())),
    };

    let fb_suppliers = proto::ChannelsRes::create(fbb, &fb_args);

    fbb.finish(fb_suppliers, None);
    fbb.finished_data().to_vec()
}

fn create_fb_content_details<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    details: &suppliers::models::ContentDetails,
) -> proto::ContentDetailsArgs<'a> {
    let fb_similar: Vec<_> = details
        .similar
        .iter()
        .map(|item| {
            let args = &create_fb_content_info(fbb, item);
            proto::ContentInfo::create(fbb, args)
        })
        .collect();

    let fb_addtional_info: Vec<_> = details
        .additional_info
        .iter()
        .map(|info| fbb.create_string(info))
        .collect();

    let fb_params: Vec<_> = details
        .params
        .iter()
        .map(|param| fbb.create_string(param))
        .collect();

    proto::ContentDetailsArgs {
        title: Some(fbb.create_string(&details.title)),
        original_title: details
            .original_title
            .as_ref()
            .map(|s| fbb.create_string(s)),
        description: Some(fbb.create_string(&details.description)),
        image: Some(fbb.create_string(&details.image)),
        media_type: proto::MediaType(details.media_type as u8),
        similar: Some(fbb.create_vector_from_iter(fb_similar.iter())),
        additional_info: Some(fbb.create_vector_from_iter(fb_addtional_info.iter())),
        params: Some(fbb.create_vector_from_iter(fb_params.iter())),
    }
}

fn create_fb_content_info<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    item: &suppliers::models::ContentInfo,
) -> proto::ContentInfoArgs<'a> {
    proto::ContentInfoArgs {
        id: Some(fbb.create_string(&item.id)),
        title: Some(fbb.create_string(&item.title)),
        secondary_title: item.secondary_title.as_ref().map(|s| fbb.create_string(s)),
        image: Some(fbb.create_string(&item.image)),
    }
}

fn create_fb_media_item<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    item: &suppliers::models::ContentMediaItem,
) -> proto::ContentMediaItemArgs<'a> {
    let fb_params: Vec<_> = item
        .params
        .iter()
        .map(|param| fbb.create_string(param))
        .collect();

    let optional_sources = &item.sources;
    let fb_optional_sources = optional_sources.as_ref().map(|sources| {
        let fb_sources: Vec<_> = sources
            .iter()
            .map(|item| {
                let args = &create_fb_media_items_source(fbb, item);
                proto::ContentMediaItemSource::create(fbb, args)
            })
            .collect();
        fbb.create_vector_from_iter(fb_sources.iter())
    });

    proto::ContentMediaItemArgs {
        number: item.number,
        title: Some(fbb.create_string(&item.title)),
        section: item.section.as_ref().map(|s| fbb.create_string(s)),
        image: item.image.as_ref().map(|s| fbb.create_string(s)),
        sources: fb_optional_sources,
        params: Some(fbb.create_vector_from_iter(fb_params.iter())),
        ..Default::default()
    }
}

fn create_fb_media_items_source<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    item: &models::ContentMediaItemSource,
) -> proto::ContentMediaItemSourceArgs<'a> {
    match item {
        models::ContentMediaItemSource::Video {
            description,
            headers,
            link,
        } => {
            let fb_links = vec![fbb.create_string(link)];
            let fb_header = create_fb_header(fbb, headers);

            proto::ContentMediaItemSourceArgs {
                type_: proto::ContentMediaItemSourceType::Video,
                description: Some(fbb.create_string(&description)),
                links: Some(fbb.create_vector_from_iter(fb_links.iter())),
                headers: fb_header,
                ..Default::default()
            }
        }
        models::ContentMediaItemSource::Subtitle {
            description,
            headers,
            link,
        } => {
            let fb_links = vec![fbb.create_string(link)];
            let fb_header = create_fb_header(fbb, headers);

            proto::ContentMediaItemSourceArgs {
                type_: proto::ContentMediaItemSourceType::Subtitle,
                description: Some(fbb.create_string(&description)),
                links: Some(fbb.create_vector_from_iter(fb_links.iter())),
                headers: fb_header,
                ..Default::default()
            }
        }
        models::ContentMediaItemSource::Manga { description, pages } => {
            let fb_pages = from_strings_vec_to_fb(fbb, &pages);
            proto::ContentMediaItemSourceArgs {
                type_: proto::ContentMediaItemSourceType::Manga,
                description: Some(fbb.create_string(&description)),
                links: Some(fbb.create_vector_from_iter(fb_pages.iter())),
                ..Default::default()
            }
        }
    }
}

fn create_fb_header<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    headers: &Option<HashMap<String, String>>,
) -> Option<WIPOffset<Vector<'a, ForwardsUOffset<proto::Header<'a>>>>> {
    headers
        .as_ref()
        .map(|h| {
            let fb_h: Vec<_> = h.iter()
                .map(|(key, val)| {
                    let fb_args = proto::HeaderArgs {
                        name: Some(fbb.create_string(key)),
                        value: Some(fbb.create_string(val)),
                    };
                    proto::Header::create(fbb, &fb_args)
                })
                .collect();

            fbb.create_vector_from_iter(fb_h.iter())
        })
}

fn from_strings_vec_to_fb<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    v: &Vec<String>,
) -> Vec<WIPOffset<&'a str>> {
    v.iter().map(|s| fbb.create_string(s)).collect()
}

fn from_str_vec_to_fb<'a>(
    fbb: &mut FlatBufferBuilder<'a>,
    v: &Vec<&str>,
) -> Vec<WIPOffset<&'a str>> {
    v.iter().map(|s| fbb.create_string(s)).collect()
}
