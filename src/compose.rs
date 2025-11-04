//! The compose module handles writing summaries from the database to the llms.txt file.

extern crate spider;

use anyhow::Result;
use log::info;
use std::fs::OpenOptions;
use std::io::Write;

use crate::storage::Storage;

/// Composes the output file by reading already summarized pages from the database
/// and writing them to the specified output file.
/// Each page's summary is written to the specified output file.
///
/// # Arguments
///
/// * `output_file` - Path to the output file where the composed content will be written
/// * `db_path` - Path to the database containing scraped pages with summaries
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if any operation fails
///
/// # Errors
///
/// Returns an error if:
/// * Database operations fail
/// * File operations fail
pub async fn compose(db_path: &str, output_path: &str) -> Result<()> {
    let storage = Storage::new(db_path)?;

    info!("Composing pages from database {db_path} to {output_path}...");

    let urls = storage.list_urls()?;

    let mut processed_count = 0;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(output_path)?;

    for url in &urls {
        let page = match storage.get_page(url)? {
            Some(page) => page,
            None => continue,
        };

        let summary = match page.summary {
            Some(summary) => summary,
            None => continue,
        };

        file.write_all(
            format!(
                "## {}\n{}\n\n",
                page.title
                    .map(|title| format!("[{}]({})", title, page.url))
                    .unwrap_or(page.url.to_string()),
                summary,
            )
            .as_bytes(),
        )?;

        processed_count += 1;
    }

    info!("Composed {processed_count} pages to {output_path}");
    Ok(())
}
