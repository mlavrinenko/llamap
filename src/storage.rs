//! The storage module provides database operations for storing and retrieving
//! scraped web page content using SQLite.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use sitemap::structs::LastMod;
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};
use url::Url;

use crate::parse::PageArticle;

/// Storage provides database operations for storing and retrieving scraped web page content.
pub struct Storage {
    /// The underlying SQLite connection wrapped in Arc<Mutex<>> to make it thread-safe
    conn: Arc<Mutex<Connection>>,
    /// Indicates whether the database was newly created or already existed
    pub new: bool,
    /// Indicates whether the database was newly created or already existed
    pub old: bool,
}

impl Storage {
    /// Creates a new Storage instance with a database at the specified path.
    ///
    /// # Arguments
    ///
    /// * `database_path` - Path where the database file should be created or opened
    ///
    /// # Returns
    ///
    /// Returns a new Storage instance on success, or an error if database creation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database creation fails
    pub fn new(database_path: &str) -> Result<Self> {
        let new = std::path::Path::new(database_path).try_exists().is_err();
        let conn = Connection::open(database_path)?;

        Self::init_schema(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            new,
            old: !new,
        })
    }

    /// Initializes the database schema with the pages table if it doesn't exist.
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS pages (
                url TEXT PRIMARY KEY,
                added_at INTEGER NOT NULL,
                lastmod INTEGER NOT NULL,
                html TEXT NOT NULL,
                title TEXT NULL,
                text TEXT NULL,
                summary TEXT NULL
            )",
            params![],
        )?;

        Ok(())
    }

    /// Returns a list of all URLs stored in the database.
    ///
    /// # Returns
    ///
    /// Returns a vector of URL strings on success, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn list_urls(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        let mut stmt = conn.prepare("SELECT url FROM pages")?;
        let urls: Result<Vec<String>, rusqlite::Error> =
            stmt.query_map([], |row| row.get(0))?.collect();

        urls.map_err(|e| e.into())
    }

    /// Gets the content for a specific URL from the database.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to look up in the database
    ///
    /// # Returns
    ///
    /// Returns the content as a string if found, None if not found, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn get_page_text(&self, url: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        let mut stmt = conn.prepare("SELECT text FROM pages WHERE url = ?1")?;
        let content: Result<Option<String>, rusqlite::Error> =
            stmt.query_row([url], |row| row.get(0)).optional();

        content.map_err(|e| e.into())
    }

    /// Gets all page data for a specific URL from the database.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to look up in the database
    ///
    /// # Returns
    ///
    /// Returns a Page struct if found, None if not found, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn get_page(&self, url: &str) -> Result<Option<Page>> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT url, added_at, lastmod, html, title, text, summary FROM pages WHERE url = ?1",
        )?;
        let page_row: Result<Option<PageRow>, rusqlite::Error> = stmt
            .query_row([url], |row| {
                Ok(PageRow {
                    url: row.get(0)?,
                    added_at: row.get(1)?,
                    lastmod: row.get(2)?,
                    html: row.get(3)?,
                    title: row.get(4)?,
                    text: row.get(5)?,
                    summary: row.get(6)?,
                })
            })
            .optional();

        let page_row: Option<PageRow> =
            page_row.map_err(|e| anyhow::anyhow!("Unable to fetch page row: {e}"))?;

        let page_row = match page_row {
            Some(page_row) => page_row,
            None => return Ok(None),
        };

        Ok(Some(page_row.try_into()?))
    }

    /// Adds or updates a page in the database.
    ///
    /// # Arguments
    ///
    /// * `page` - The Page struct containing all the page data
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn upsert_page(&self, page: &Page) -> Result<()> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        conn.execute(
            "INSERT OR REPLACE INTO pages (url, added_at, lastmod, html, title, text, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                page.url.as_str(),
                page.added_at.timestamp(),
                page.lastmod.timestamp(),
                page.html,
                page.title,
                page.text.as_deref().unwrap_or_default(),
                page.summary.as_deref()
            ],
        )?;

        Ok(())
    }

    /// Updates the text content for a page in the database.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the page to update
    /// * `text` - The processed text content to store
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn update_page_text(&self, url: &str, text: &str) -> Result<()> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        conn.execute(
            "UPDATE pages SET text = ?1 WHERE url = ?2",
            params![text, url],
        )?;

        Ok(())
    }

    /// Updates the summary for a page in the database.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the page to update
    /// * `summary` - The summary content to store
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn update_page_summary(&self, url: &str, summary: &str) -> Result<()> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        conn.execute(
            "UPDATE pages SET summary = ?1 WHERE url = ?2",
            params![summary, url],
        )?;

        Ok(())
    }

    /// Removes a page from the database.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the page to remove
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn remove_page(&self, url: &str) -> Result<()> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        conn.execute("DELETE FROM pages WHERE url = ?1", params![url])?;
        Ok(())
    }

    /// Gets a limited number of pages that have not been summarized yet.
    /// This helps manage memory usage when dealing with large databases.
    ///
    /// # Arguments
    ///
    /// * `limit` - The maximum number of pages to retrieve
    ///
    /// # Returns
    ///
    /// Returns a vector of (url, text) tuples for pages that need summarization on success,
    /// or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn fetch_unsummarized_pages(&self, limit: u32) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT url, text FROM pages WHERE summary IS NULL OR summary = '' ORDER BY added_at ASC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let pages: Vec<(String, String)> = rows.flatten().collect();

        Ok(pages)
    }

    /// Gets a limited number of all pages from the database with an offset.
    /// This helps manage memory usage when dealing with large databases
    /// and allows processing all records in batches.
    ///
    /// # Arguments
    ///
    /// * `limit` - The maximum number of pages to retrieve
    /// * `offset` - The offset from which to start retrieving pages
    ///
    /// # Returns
    ///
    /// Returns a vector of (url, text) tuples for all pages on success,
    /// or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn fetch_pages(&self, limit: u32, offset: u32) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        let mut stmt =
            conn.prepare("SELECT url, text FROM pages ORDER BY added_at ASC LIMIT ?1 OFFSET ?2")?;
        let rows = stmt.query_map([limit, offset], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let pages: Vec<(String, String)> = rows.flatten().collect();

        Ok(pages)
    }

    /// Gets the content for a specific URL from the database.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to look up in the database
    ///
    /// # Returns
    ///
    /// Returns the text content as a string if found, None if not found, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn fetch_page_content(&self, url: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        let mut stmt = conn.prepare("SELECT text FROM pages WHERE url = ?1")?;
        let content: Result<Option<String>, rusqlite::Error> =
            stmt.query_row([url], |row| row.get(0)).optional();

        content.map_err(|e| e.into())
    }

    /// Filters and returns URLs that need to be scraped. A URL needs to be scraped if:
    /// 1. It's not in the database (new URL)
    /// 2. Its lastmod timestamp in the sitemap is different from the lastmod in the database
    ///
    /// # Arguments
    ///
    /// * `sitemap_entries` - A map of URLs to their sitemap entries containing lastmod information
    ///
    /// # Returns
    ///
    /// Returns a vector of URLs that need to be scraped on success, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn resolve_modified(
        &self,
        sitemap_entries: std::collections::HashMap<String, sitemap::structs::UrlEntry>,
    ) -> Result<Vec<String>> {
        let mut scrapable_urls = Vec::new();
        for (url, sitemap_entry) in sitemap_entries {
            if self
                .should_scrape(&url, sitemap_entry.lastmod)
                .unwrap_or(true)
            {
                scrapable_urls.push(url);
            }
        }

        Ok(scrapable_urls)
    }

    /// Determines if a URL should be scraped based on its lastmod timestamp.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to check
    /// * `sitemap_lastmod` - The lastmod timestamp from the sitemap
    ///
    /// # Returns
    ///
    /// Returns `true` if the URL should be scraped, `false` otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    fn should_scrape(&self, url: &str, lastmod: LastMod) -> Result<bool> {
        Ok(match lastmod {
            LastMod::DateTime(lastmod) => {
                let db_lastmod = match self.get_lastmod(url)? {
                    Some(db_lastmod) => db_lastmod,
                    // No record in DB, should scrape
                    None => return Ok(true),
                };

                let db_lastmod_datetime = DateTime::from_timestamp(db_lastmod, 0)
                    .ok_or("Can't convert timestamp to DateTime")
                    .map_err(|e| anyhow::anyhow!("{e}"))?;

                lastmod != db_lastmod_datetime
            }
            // No lastmod in sitemap, should scrape.
            LastMod::None => true,
            // Error parsing timestamp, should scrape.
            LastMod::ParseErr(_) => true,
        })
    }

    /// Gets the lastmod timestamp for a specific URL from the database.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to look up in the database
    ///
    /// # Returns
    ///
    /// Returns the lastmod timestamp as an i64 if found, None if not found, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn get_lastmod(&self, url: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().expect("Storage mutex poisoned");
        let mut stmt = conn.prepare("SELECT lastmod FROM pages WHERE url = ?1")?;
        let lastmod: Result<Option<i64>, rusqlite::Error> =
            stmt.query_row([url], |row| row.get(0)).optional();

        lastmod.map_err(|e| e.into())
    }

    /// Removes all pages from the database that are not present in the provided list of visited URLs.
    /// This is more efficient than individual deletions as it uses a single SQL DELETE operation.
    ///
    /// # Arguments
    ///
    /// * `visited_urls` - A collection of URLs that were visited during scraping
    ///
    /// # Returns
    ///
    /// Returns the number of pages removed on success, or an error if database operation fails
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned
    pub fn remove_unvisited_pages<I>(&self, visited_urls: I) -> Result<usize>
    where
        I: IntoIterator<Item = String>,
    {
        let conn = self.conn.lock().expect("Storage mutex poisoned");

        conn.execute_batch(
            r#"
                DROP TABLE IF EXISTS temp_visited_urls;
                CREATE TEMPORARY TABLE temp_visited_urls (url TEXT PRIMARY KEY);
            "#,
        )?;

        let urls: Vec<String> = visited_urls.into_iter().collect();
        const BATCH_SIZE: usize = 100;
        for chunk in urls.chunks(BATCH_SIZE) {
            let placeholders: Vec<String> = vec!["?".to_string(); chunk.len()];
            let sql = format!(
                "INSERT INTO temp_visited_urls (url) VALUES ({})",
                placeholders.join("), (")
            );

            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            conn.execute(&sql, rusqlite::params_from_iter(params))?;
        }

        let deleted_count = conn.execute(
            "DELETE FROM pages WHERE url NOT IN (SELECT url FROM temp_visited_urls)",
            [],
        )?;

        Ok(deleted_count)
    }
}

/// Represents a page stored in the database
#[derive(Debug)]
pub struct PageRow {
    pub url: String,
    pub added_at: i64,
    pub lastmod: i64,
    pub html: String,
    pub title: Option<String>,
    pub text: Option<String>,
    pub summary: Option<String>,
}

/// Represents domain Page
#[derive(Debug)]
pub struct Page {
    pub url: Url,
    pub added_at: DateTime<Utc>,
    pub lastmod: DateTime<Utc>,
    pub html: String,
    pub title: Option<String>,
    pub text: Option<String>,
    pub summary: Option<String>,
}

impl Page {
    /// Applies content from a PageArticle to the page.
    ///
    /// Updates the text field with the article text, and the title field with
    /// the article title if one exists, otherwise keeping the existing title.
    pub fn apply_article(&mut self, article: PageArticle) {
        self.text = Some(article.text);
        if let Some(title) = article.title {
            self.title = Some(title);
        }
    }
}

impl TryFrom<PageRow> for Page {
    type Error = anyhow::Error;

    fn try_from(page_row: PageRow) -> Result<Self> {
        Ok(Page {
            url: Url::parse(&page_row.url)?,
            added_at: DateTime::from_timestamp_secs(page_row.added_at)
                .context("Unable to initialize added_at from database")?,
            lastmod: DateTime::from_timestamp_secs(page_row.lastmod)
                .context("Unable to initialize lastmod from database")?,
            html: page_row.html,
            title: page_row.title,
            text: page_row.text,
            summary: page_row.summary,
        })
    }
}
