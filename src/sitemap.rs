use std::collections::HashMap;

use anyhow::Result;
use sitemap::{
    reader::{SiteMapEntity, SiteMapReader},
    structs::UrlEntry,
};

/// Extracts URL entries from a sitemap.
///
/// This function takes a sitemap URL and returns a `HashMap` containing the URL entries found in the sitemap.
/// It processes the sitemap and any nested sitemaps recursively.
///
/// # Arguments
///
/// * `sitemap_url` - A string slice that holds the URL of the sitemap to be processed.
///
/// # Returns
///
/// A `Result` containing a `HashMap` with the URL entries if successful, or an error if any operation fails.
///
/// # Errors
///
/// This function will return an error if there is a problem fetching the sitemap or parsing its content.
pub async fn extract_sitemap_url_entries(sitemap_url: &str) -> Result<HashMap<String, UrlEntry>> {
    let mut entries = HashMap::new();
    let mut sitemaps_to_process = vec![sitemap_url.to_string()];
    let client = reqwest::Client::new();

    while let Some(current_sitemap) = sitemaps_to_process.pop() {
        let response = client.get(&current_sitemap).send().await?;
        let content = response.bytes().await?;

        let reader = SiteMapReader::new(&*content);

        for entity in reader {
            match entity {
                SiteMapEntity::Url(url_entry) => {
                    if let sitemap::structs::Location::Url(ref url) = url_entry.loc {
                        entries.insert(url.to_string(), url_entry);
                    }
                }
                SiteMapEntity::SiteMap(sitemap_entry) => {
                    if let sitemap::structs::Location::Url(ref url) = sitemap_entry.loc {
                        sitemaps_to_process.push(url.to_string());
                    }
                }
                SiteMapEntity::Err(_) => continue,
            }
        }
    }

    Ok(entries)
}
