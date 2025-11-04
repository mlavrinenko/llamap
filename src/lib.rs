//! The llamap library provides functionality for scraping websites using sitemap.xml
//! and composing the results into an llms.txt file for AI crawlers.

pub mod compose;
pub mod constants;
pub mod parse;
pub mod scrape;
pub mod sitemap;
pub mod storage;
pub mod summarize;

/// Enum representing the text extraction method.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum TextBy {
    /// Use dom_smoothie for text extraction
    #[default]
    DomSmoothie,
    /// Use fast_html2md for text extraction
    FastHtml2Md,
}

impl std::str::FromStr for TextBy {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "dom_smoothie" => Ok(TextBy::DomSmoothie),
            "fast_html2md" => Ok(TextBy::FastHtml2Md),
            _ => Err(format!("Invalid text extraction method: {}", input)),
        }
    }
}

/// Enum representing the target for summarization.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum SummarizeTarget {
    /// All database pages with no summary.
    #[default]
    Unsummarized,
    /// All database pages.
    All,
    /// A page with specified URL.
    Page { url: String },
}

impl From<&str> for SummarizeTarget {
    fn from(value: &str) -> Self {
        match value {
            "unsummarized" => Self::Unsummarized,
            "all" => Self::All,
            url => Self::Page {
                url: url.to_string(),
            },
        }
    }
}

/// Enum representing the target for parsing/re-extraction.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum ParseTarget {
    /// All database pages.
    #[default]
    All,
    /// A page with specified URL.
    Page { url: String },
}

impl From<&str> for ParseTarget {
    fn from(value: &str) -> Self {
        match value {
            "all" => Self::All,
            url => Self::Page {
                url: url.to_string(),
            },
        }
    }
}

pub use compose::compose;
pub use parse::{extract_article, parse_db_html};
pub use scrape::process_sitemap;
pub use summarize::summarize;
