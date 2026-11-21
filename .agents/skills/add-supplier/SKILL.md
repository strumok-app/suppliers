---
name: add-supplier
description: Add a new content supplier to the strumok_suppliers Rust crate. Use this when the user wants to integrate a new streaming/manga/movie site as a supplier. Guides an interactive, step-by-step workflow that reverse-engineers the target site, confirms design choices with the user, implements the ContentSupplier trait, registers the supplier, and writes tests.
---

# Add a new supplier

Use this skill when the user wants to add a new content source (anime / movie / series /
cartoon / manga site) to the `strumok_suppliers` crate.

Adding a supplier is a **step-by-step, interactive process**. You must reverse-engineer a
live website, so you cannot do it blindly. Work through the phases below in order. Do not
skip ahead to writing code before you understand the site and have confirmed the design
choices with the user.

**Two rules that apply the whole time:**

1. **Ask the user for every detail you need for a complete understanding.** Never guess a
   site URL, supplier name, content type, or endpoint. If something is ambiguous, stop and
   ask.
2. **Confirm any solution that involves a choice.** When more than one reasonable approach
   exists (HTML scraping vs JSON API, which extractor to reuse, supplier display name,
   which channels to expose, how to handle encrypted sources, etc.), present the options
   and get the user's decision before implementing.

---

## Phase 0 — Discovery (ask the user)

Before touching any code, collect the following from the user. Ask for anything that is
still unknown:

- **Site URL** (the base domain, e.g. `https://example.to`).
- **Supplier display name** — this becomes the `#[strum(serialize = "...")]` value and the
  public identifier. Also agree on the Rust names derived from it: file `src/suppliers/<name>.rs`
  and struct `<Name>ContentSupplier` (follow the existing casing conventions, e.g.
  `Anizone` → `anizone.rs` / `AnizoneContentSupplier`).
- **Content type(s)** — one or more of `Movie`, `Anime`, `Cartoon`, `Series`, `Manga`
  (see `src/models.rs`).
- **Media type** — `Video` or `Manga` (drives `ContentDetails.media_type` and whether you
  implement `MangaPagesLoader`).
- **Supported languages** — ISO codes used elsewhere in the crate (e.g. `"en"`,
  `"uk"`).
- **Channels** — the named browse/listing sections to expose (e.g. "Latest", "Popular").
  Ask which ones the user wants and which, if any, should be default channels.
- **Known constraints** — does the site require login, have Cloudflare/DDoS-Guard
  protection, use geo-blocking, or encrypt its video sources? If the site is behind a CF
  JS challenge (`403 cf-mitigated: challenge`), warn the user it may not be feasible and
  confirm whether to continue.

Summarize what you learned and confirm it with the user before moving on.

---

## Phase 1 — Reverse-engineer the site

Figure out, by inspecting the live site, how to drive each of the five data operations.
Use the `fetch` tool, or `curl` from the `terminal` tool. **Network is sandboxed**, so pass
`allow_hosts: ["the-site-host"]` (and any CDN/API hosts) on terminal calls, or use `fetch`
which shares the same per-host grants.

For each operation, determine the request (URL, method, query params, required headers like
`Referer` / `X-Requested-With`) and the response shape (**HTML** to scrape, or **JSON** to
deserialize):

1. **search** — how a text query is performed and paginated.
2. **load_channel** — how each named channel/listing page is fetched and paginated.
3. **get_content_details** — the content page: title, original title, image, description,
   additional info, similar items. Identify the stable **`id`** you can later rebuild into a
   URL. The `id` from search/channel results is exactly what gets passed back here.
4. **load_media_items** — the list of episodes/chapters. Note any intermediate IDs that must
   be carried forward via `params`.
5. **load_media_item_sources** — the actual video/subtitle links (or manga chapter handle).
   Identify the player/host and whether an existing extractor applies (see
   `src/extractors/`: `megaplay`, `moonanime`, `tortuga`, `packer_hls`). If sources are
   encrypted, check `src/utils/` (`crypto`, `crypto_js`, `enc_dec_app`, `playerjs`,
   `jwp_player`, `nextjs`, `unpack`, `datalife`) for a reusable helper.
6. **(Manga only) load_pages** — how a chapter's page images are retrieved.

Write down the concrete selectors / JSON field names you'll use. **Confirm the chosen
approach (HTML vs JSON, which extractor/decryptor, how `id` and `params` are encoded) with
the user** before implementing.

Study one or two existing suppliers that resemble the target as references:
- HTML scraping with processors: `src/suppliers/weebcentral.rs`, `src/suppliers/anikoto.rs`.
- Inline JSON extracted from HTML: `src/suppliers/anizone.rs`.
- Manga (also implements `MangaPagesLoader`): `src/suppliers/weebcentral.rs`.

---

## Phase 2 — Implement the supplier

Create `src/suppliers/<name>.rs`. Follow the conventions used across existing suppliers.

- Put a `const URL` (or `SITE_URL`) at the top.
- Define `pub struct <Name>ContentSupplier { ... }`. Cache reusable, thread-safe state here
  (parsed `scraper::Selector`s, `html` processors, channel maps). Suppliers are created once
  and live for the whole session, so all fields must be `Send + Sync`.
- Implement `Default` to build those fields.
- Implement the `ContentSupplier` trait from `src/suppliers/mod.rs`. All methods:
  - `fn get_channels(&self) -> Vec<String>`
  - `fn get_default_channels(&self) -> Vec<String>` (often `vec![]`)
  - `fn get_supported_types(&self) -> Vec<ContentType>`
  - `fn get_supported_languages(&self) -> Vec<String>`
  - `async fn search(&self, query: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>>`
  - `async fn load_channel(&self, channel: &str, page: u16) -> anyhow::Result<Vec<ContentInfo>>`
  - `async fn get_content_details(&self, id: &str) -> anyhow::Result<Option<ContentDetails>>`
  - `async fn load_media_items(&self, id: &str, params: Vec<String>) -> anyhow::Result<Vec<ContentMediaItem>>`
  - `async fn load_media_item_sources(&self, id: &str, params: Vec<String>) -> anyhow::Result<Vec<ContentMediaItemSource>>`
- **Manga only:** also implement `MangaPagesLoader` (`async fn load_pages(&self, id, params) -> anyhow::Result<Vec<String>>`).

Reuse the shared helpers instead of hand-rolling HTTP/parsing:

- HTTP clients: `utils::create_client()` (HTML `Accept`) and `utils::create_json_client()`
  (JSON). Both are shared singletons with DoH, cookie store, and a browser user-agent.
- Scraping drivers: `utils::scrap_page(request_builder, &processor)` (full document) and
  `utils::scrap_fragment(...)` (HTML fragment).
- Declarative DOM processors in `utils::html`: `ItemsProcessor`, `ScopeProcessor`,
  `ContentInfoProcessor`, `ContentDetailsProcessor`, `ContentMediaItemProcessor`,
  `text_value`, `attr_value`, `attr_value_map`, `items_processor`, `default_value`,
  `self_hosted_image`, etc. Build these in `Default` and store them on the struct.
- Text helpers in `utils::text`: `sanitize_text`, `extract_digits`, `to_full_url`,
  `strip_html`, `unescape_js_string`, …
- Extractors in `extractors::*` for known players; decryption helpers in `utils::*`.

Conventions to honor:
- **Eager population.** If you already have the data earlier in the flow, populate it then
  instead of in a later call. When `get_content_details` already knows the episodes/chapters,
  fill `ContentDetails.media_items` and make `load_media_items` return
  `Err(anyhow!("unimplemented"))`. Likewise, when `load_media_items` already knows the
  links, fill each `ContentMediaItem.sources` and make `load_media_item_sources` return
  `Err(anyhow!("unimplemented"))`. Both fields are `Option` in `src/models.rs` exactly for
  this. Confirm with the user which flow the site supports.
- The `id` you emit in `ContentInfo` must be exactly what `get_content_details` /
  `load_media_items` expect back.
- Use `params: Vec<String>` to carry intermediate identifiers between steps (e.g. an
  episode `data-id`, a server id, a manga chapter id). Validate their length and return a
  clear `anyhow!` error when missing.
- If a site/channel is not paginated, return `Ok(vec![])` for `page > 1`.
- See the data model definitions in `src/models.rs`: `ContentInfo`, `ContentDetails`,
  `ContentMediaItem`, and the `ContentMediaItemSource` enum (`Video`, `Subtitle`, `Manga`).

---

## Phase 3 — Register the supplier

Edit `src/suppliers/mod.rs`:

1. Add `mod <name>;` with the other module declarations (keep alphabetical grouping).
2. Add `use <name>::<Name>ContentSupplier;` with the other `use` lines.
3. Add a variant to the `AllContentSuppliers` enum:
   ```rust
   #[strum(serialize = "DisplayName")]
   <Name>ContentSupplier,
   ```
4. **Manga only:** also add the same `#[strum(serialize = "DisplayName")] <Name>ContentSupplier,`
   variant to the `AllMangaPagesLoaders` enum.

No change is needed in `src/api/mod.rs` — the API dispatches through the registry
automatically. The `should_register_every_supplier_once` test will cover registration.

---

## Phase 4 — Tests

Add a `#[cfg(test)] mod tests` block at the bottom of the new file, mirroring existing
suppliers. Use `#[test_log::test(tokio::test)]` and exercise each method with real IDs you
discovered during reverse-engineering (`search`, `load_channel`, `get_content_details`,
`load_media_items`, `load_media_item_sources`, and `load_pages` for manga). Printing the
result with `println!("{res:#?}")` is the established pattern.

Run them (tests hit the live site, so grant network access):

```
cargo test <name> -- --nocapture
```

Pass `allow_hosts` for the site (and any CDN/API/player hosts) on the terminal call, or
`make test` to run the whole suite. Confirm the new supplier's tests return sensible data,
then run `cargo build` / check `diagnostics` to ensure the crate still compiles cleanly.

---

## Finishing up

Summarize for the user:
- the files added/changed (`src/suppliers/<name>.rs`, `src/suppliers/mod.rs`),
- the display name and how it's registered,
- which tests you ran and their outcome,
- any limitations discovered (unpaginated channels, partial source support, CF risk, etc.).
