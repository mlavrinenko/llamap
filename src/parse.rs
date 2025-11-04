use crate::{ParseTarget, TextBy, storage::Storage};

use anyhow::Result;
use dom_smoothie::{Article, CandidateSelectMode, Config, Readability, TextMode};
use html2md;
use log::{error, info};
use scraper::{Html, Selector as ScraperSelector};

/// Represents an article extracted from a webpage.
///
/// This struct contains the title and text content of the article.
#[derive(Debug)]
pub struct PageArticle {
    /// The title of the article, if available.
    pub title: Option<String>,
    /// The text content of the article.
    pub text: String,
}

/// Extracts an article from the given HTML content.
///
/// This function uses the specified text extraction method to parse the HTML and extract the article.
///
/// # Arguments
///
/// * `html` - A string slice that holds the HTML content of the webpage.
/// * `text_by` - The method to use for text extraction (dom_smoothie or fast_html2md).
/// * `selector` - An optional CSS selector to limit the HTML subset from which content is extracted.
///
/// # Returns
///
/// A `Result` containing a `PageArticle` if the extraction is successful, or an error if it fails.
///
/// # Errors
///
/// This function will return an error if:
///
/// - The HTML content is invalid or cannot be parsed.
/// - The chosen extraction method fails to extract the article from the HTML content.
pub fn extract_article(
    html: &str,
    text_by: TextBy,
    selector: &Option<ScraperSelector>,
) -> Result<PageArticle> {
    let title = parse_title(html);
    let selected_html = if let Some(sel) = selector {
        let document = Html::parse_document(html);
        let elements = document.select(sel);
        let selected_content: Vec<String> = elements.map(|el| el.html()).collect();
        &selected_content.join("\n")
    } else {
        html
    };

    match text_by {
        TextBy::DomSmoothie => {
            let config = Config {
                text_mode: TextMode::Markdown,
                candidate_select_mode: CandidateSelectMode::DomSmoothie,
                ..Default::default()
            };

            let mut readability = Readability::new(selected_html, None, Some(config))?;
            let article: Article = readability.parse()?;

            Ok(PageArticle {
                title,
                text: article.text_content.to_string(),
            })
        }
        TextBy::FastHtml2Md => {
            let text = html2md::parse_html(selected_html, false);
            Ok(PageArticle { title, text })
        }
    }
}

/// Parses the title from HTML content
fn parse_title(html: &str) -> Option<String> {
    let document = Html::parse_document(html);

    if let Ok(title_selector) = ScraperSelector::parse("title")
        && let Some(title_element) = document.select(&title_selector).next()
    {
        let title_text = title_element
            .text()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        if !title_text.is_empty() {
            return Some(title_text);
        }
    }

    for tag in ["h1", "h2"] {
        if let Ok(tag_selector) = ScraperSelector::parse(tag)
            && let Some(tag_element) = document.select(&tag_selector).next()
        {
            let tag_text = tag_element
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            if !tag_text.is_empty() {
                return Some(tag_text);
            }
        }
    }

    None
}

/// Extract content from HTML stored in the database and updates the text and title field
///
/// # Arguments
///
/// * `db_path` - Path to the database file to read pages from
/// * `target` - The parse target (all pages or specific page)
/// * `text_by` - The method to use for text extraction (dom_smoothie or fast_html2md)
/// * `selector` - An optional CSS selector to limit the HTML subset from which content is extracted.
///
/// # Errors
///
/// This function will return an error if:
/// - Database operations fail
pub async fn parse_db_html(
    db_path: &str,
    target: ParseTarget,
    text_by: TextBy,
    selector: &Option<ScraperSelector>,
) -> Result<()> {
    let storage = Storage::new(db_path)?;

    match target {
        ParseTarget::All => {
            let urls = storage.list_urls()?;
            for url in urls {
                info!("Parsing {url}");
                let mut page = match storage.get_page(&url)? {
                    Some(page) => page,
                    None => continue,
                };

                let article = extract_article(&page.html, text_by.clone(), selector)?;
                page.apply_article(article);
                storage.upsert_page(&page)?;
            }
        }
        ParseTarget::Page { url } => {
            let mut page = if let Some(page) = storage.get_page(&url)? {
                page
            } else {
                error!("Page not found: {url}");
                return Ok(());
            };

            let article = extract_article(&page.html, text_by, selector)?;

            page.apply_article(article);
            storage.upsert_page(&page)?;
        }
    }

    Ok(())
}
