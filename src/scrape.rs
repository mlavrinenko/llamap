//! The scrape module provides functionality to scrape websites using sitemap.xml
//! and store the scraped content in a local database.

extern crate spider;

use anyhow::{Context, Result};
use log::{error, info, warn};
use spider::configuration::Configuration;
use spider::website::Website;
use std::sync::Arc;
use tokio::sync::mpsc;
use url::Url;

use crate::sitemap::extract_sitemap_url_entries;
use crate::storage::Storage;

/// Scrapes a website using its sitemap and saves pages to a local database.
///
/// # Arguments
///
/// * `sitemap_url` - The URL of the sitemap to scrape
/// * `db_path` - Path to the database where pages will be stored
/// * `delay` - Delay between requests in milliseconds (rate limiting)
/// * `concurrency` - Number of concurrent requests
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if any operation fails
///
/// # Errors
///
/// Returns an error if:
/// * The sitemap URL is invalid
/// * Network requests fail
/// * Database operations fail
///
/// # Panics
///
/// This function will panic if the system time is before UNIX_EPOCH when creating new page records,
/// or if the fallback URL "http://example.com" fails to parse
pub async fn process_sitemap(
    sitemap_url: Url,
    db_path: &str,
    delay: u64,
    concurrency: usize,
) -> Result<()> {
    let (mut website, storage) =
        setup_website_and_storage(sitemap_url.as_str(), db_path, delay, concurrency).await?;
    let (scrape_storage, cleanup_storage) = (Arc::clone(&storage), Arc::clone(&storage));
    let (failed_url_tx, failed_url_rx) = mpsc::unbounded_channel();

    let mut receiver = website
        .subscribe(888)
        .context("Unable to create receiver.")?;

    let handle = tokio::spawn(async move {
        while let Ok(page) = receiver.recv().await {
            info!("Scraped {} with {}", page.get_url(), page.status_code);

            if !page.status_code.is_success() {
                warn!("Skipping {} as {}", page.get_url(), page.status_code);
                if let Err(e) = failed_url_tx.send(page.get_url().to_string()) {
                    error!("Failed to send failed URL through channel: {}", e);
                }
                continue;
            }

            let html = page.get_html();
            let url = match Url::parse(page.get_url()) {
                Ok(parsed_url) => parsed_url,
                Err(parse_error) => {
                    error!("Error parsing URL {}: {parse_error}", page.get_url());
                    if let Err(e) = failed_url_tx.send(page.get_url().to_string()) {
                        error!("Failed to send failed URL through channel: {}", e);
                    }
                    continue;
                }
            };

            let metadata = page.get_metadata().as_ref();

            let db_page = crate::storage::Page {
                url,
                added_at: chrono::Utc::now(),
                lastmod: chrono::Utc::now(),
                html,
                title: metadata.and_then(|meta| meta.title.clone().map(|title| title.to_string())),
                text: None,
                summary: None,
            };

            if let Err(storage_error) = scrape_storage.upsert_page(&db_page) {
                error!(
                    "Error storing page {} with minimal data: {storage_error}",
                    db_page.url.as_str()
                );

                return;
            }
        }
    });

    info!("Starting Crawl on {sitemap_url:?}");
    website.persist_links();
    website.crawl().await;
    website.unsubscribe();
    handle.await.context("Task failed to complete")?;

    storage
        .old
        .then(async || cleanup_unvisited_pages(website, &cleanup_storage, failed_url_rx).await);
    Ok(())
}

async fn setup_website_and_storage(
    sitemap_url_str: &str,
    db_path: &str,
    delay: u64,
    concurrency: usize,
) -> Result<(Website, Arc<Storage>)> {
    let sitemap_url = Url::parse(sitemap_url_str)?;
    let base_url = sitemap_url.join("/")?.to_string();

    let config = Configuration::new()
        .with_user_agent(Some("LLaMap Bot"))
        .with_subdomains(false)
        .with_redirect_limit(3)
        .with_retry(1)
        .with_depth(0)
        .with_respect_robots_txt(true)
        .with_delay(delay)
        .with_concurrency_limit(Some(concurrency))
        .build();

    let storage = Arc::new(Storage::new(db_path)?);
    let mut website = Website::new(&base_url)
        .with_config(config.clone())
        .build()?;

    let sitemap_entries = extract_sitemap_url_entries(sitemap_url_str).await?;
    let sitemap_entries_count = sitemap_entries.len();
    let scrape_urls = if storage.new {
        sitemap_entries.into_keys().collect()
    } else {
        storage.resolve_modified(sitemap_entries)?
    };

    info!(
        "Sitemap entries: {}/{} (modified/all)",
        scrape_urls.len(),
        sitemap_entries_count
    );

    website.set_extra_links(
        scrape_urls
            .into_iter()
            .map(|url| spider::CaseInsensitiveString::new(&url))
            .collect::<spider::hashbrown::HashSet<spider::CaseInsensitiveString>>(),
    );

    Ok((website, storage))
}

async fn cleanup_unvisited_pages(
    website: Website,
    cleanup_storage: &Storage,
    mut failed_url_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
) {
    let mut scraped_urls: Vec<String> = website
        .get_all_links_visited()
        .await
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    while let Ok(failed_url) = failed_url_rx.try_recv() {
        scraped_urls.retain(|url| url != &failed_url);
    }

    match cleanup_storage.remove_unvisited_pages(scraped_urls) {
        Ok(count) => info!("Removed {count} unvisited pages from storage"),
        Err(error) => error!("Error removing unvisited pages: {error}"),
    }
}
