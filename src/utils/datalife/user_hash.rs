use std::{
    fmt,
    sync::{Mutex, PoisonError},
};

use anyhow::anyhow;
use regex::Regex;

use crate::utils;

/// Returned by an operation passed to [`UserHash::with_retry`] when the site
/// rejected the hash, so a fresh one is scraped and the operation retried.
#[derive(Debug)]
pub struct UserHashRejected;

impl fmt::Display for UserHashRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("user_hash was rejected")
    }
}

impl std::error::Error for UserHashRejected {}

/// DLE `user_hash` required by the ajax endpoints.
///
/// For anonymous visitors it is the same site-wide value, exposed in every page
/// as a JS variable, but sites rotate it from time to time, so it is scraped and
/// cached rather than hardcoded. Suppliers live for the whole app session, so
/// the cache is shared between calls. The lock is never held across `.await`.
pub struct UserHash {
    page_url: &'static str,
    re: Regex,
    hash: Mutex<Option<String>>,
}

impl UserHash {
    /// `page_url` is any page exposing the hash, `js_var` is the name of the
    /// variable holding it (e.g. `dle_login_hash`).
    pub fn new(page_url: &'static str, js_var: &str) -> Self {
        Self {
            page_url,
            re: Regex::new(&format!(r#"{js_var}\s*=\s*['"]([a-z0-9]+)['"]"#)).unwrap(),
            hash: Mutex::new(None),
        }
    }

    pub fn cached(&self) -> Option<String> {
        self.hash
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn store(&self, hash: String) {
        *self.hash.lock().unwrap_or_else(PoisonError::into_inner) = Some(hash);
    }

    pub fn extract(&self, html: &str) -> Option<String> {
        self.re
            .captures(html)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Caches the hash found in an already loaded page, saving a request later.
    pub fn store_from_page(&self, html: &str) {
        if let Some(hash) = self.extract(html) {
            self.store(hash);
        }
    }

    pub async fn refresh(&self) -> anyhow::Result<String> {
        let html = utils::create_client()
            .get(self.page_url)
            .send()
            .await?
            .text()
            .await?;

        let hash = self
            .extract(&html)
            .ok_or_else(|| anyhow!("user_hash not found on {}", self.page_url))?;

        self.store(hash.clone());

        Ok(hash)
    }

    /// Runs `operation` with the cached hash. If it fails with
    /// [`UserHashRejected`], the hash is re-scraped and the operation retried once.
    /// Concurrent callers may refresh at the same time, which is harmless.
    pub async fn with_retry<T, F, Fut>(&self, operation: F) -> anyhow::Result<T>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let (hash, cached) = match self.cached() {
            Some(hash) => (hash, true),
            None => (self.refresh().await?, false),
        };

        match operation(hash).await {
            Err(err) if cached && err.is::<UserHashRejected>() => {
                operation(self.refresh().await?).await
            }
            res => res,
        }
    }
}
