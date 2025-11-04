//! The summarize module handles summarization of scraped pages from the database
//! using an LLM model and updates the summary in the database.

extern crate spider;

use anyhow::Result;
use llm::builder::LLMBuilder;
use llm::chat::{ChatMessage, ChatMessageBuilder, ChatProvider};
use log::{debug, info};
use once_cell::sync::Lazy;
use regex::Regex;
use std::cell::RefCell;

use crate::SummarizeTarget;
use crate::constants::{DEFAULT_PROMPT_TEMPLATE, THINK_STRIPPER};
use crate::storage::Storage;

use rate_guard::{RateLimit, StdTokenBucket, TokenBucketBuilder};
use std::time::Duration;

static THINK_STRIPPER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(THINK_STRIPPER).expect("Failed to compile THINK_STRIPPER regex"));

/// Configuration containing shared data for summarization operations
pub struct SummarizeContext<'a> {
    /// LLM model to use for summarization
    pub model: &'a dyn ChatProvider,
    /// Prompt template to use
    pub prompt_template: Option<&'a str>,
    /// Rate limiter for controlling request frequency
    pub rate_limiter: Option<&'a StdTokenBucket>,
}

/// Summarizes pages from the database that have not been summarized yet
/// Each page is processed and the summary is stored in the database.
/// This function processes pages in batches to avoid overloading memory.
///
/// # Arguments
///
/// * `db_path` - Path to the database containing scraped pages
/// * `llm_builder` - The LLM builder to create the model for processing
/// * `prompt` - Optional prompt template to use for summarization
/// * `rpm` - Rate limit: requests per minute (default: no limit)
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if any operation fails
///
/// # Errors
///
/// Returns an error if:
/// * The LLM model fails to build
/// * Database operations fail
/// * File operations fail
pub async fn summarize(
    db_path: &str,
    llm_builder: LLMBuilder,
    prompt_template: Option<&str>,
    target: SummarizeTarget,
    rpm: Option<u32>,
) -> Result<()> {
    let model = llm_builder
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build LLM model: {}", e))?;

    let rate_limiter: Option<StdTokenBucket> = rpm.and_then(|rpm| {
        let capacity = rpm.max(1) as u64;
        let refill_interval = Duration::from_secs_f64(60.0 / capacity as f64);

        TokenBucketBuilder::builder()
            .capacity(capacity)
            .refill_amount(1_u64)
            .refill_every(refill_interval)
            .with_time(rate_guard::StdTimeSource::new())
            .with_precision::<rate_guard::Nanos>()
            .build()
            .ok()
    });

    let storage = Storage::new(db_path)?;

    let ctx = SummarizeContext {
        model: model.as_ref(),
        prompt_template,
        rate_limiter: rate_limiter.as_ref(),
    };

    let total_processed = match &target {
        SummarizeTarget::Unsummarized => {
            info!("Summarizing pages from database {db_path} that haven't been summarized yet...");
            summarize_unsummarized_pages(&ctx, &storage).await?
        }
        SummarizeTarget::All => {
            info!("Summarizing ALL pages from database {db_path}...");
            summarize_all_pages(&ctx, &storage).await?
        }
        SummarizeTarget::Page { url } => {
            info!("Summarizing page {url} from database {db_path}...");
            summarize_single_page(&ctx, &storage, url).await?
        }
    };

    if total_processed == 0 {
        match &target {
            SummarizeTarget::Unsummarized => {
                info!("No pages to summarize. All pages already have summaries.");
            }
            SummarizeTarget::All => {
                info!("No pages in the database.");
            }
            SummarizeTarget::Page { url } => {
                info!("Page {url} not found in the database.");
            }
        }
    } else {
        info!("Summarized {total_processed} pages");
    }

    Ok(())
}

const FETCH_BATCH_SIZE: u32 = 100;

/// Summarizes pages from the database that have not been summarized yet
async fn summarize_unsummarized_pages(
    ctx: &SummarizeContext<'_>,
    storage: &Storage,
) -> Result<u32> {
    summarize_fetched_pages(ctx, storage, || {
        storage.fetch_unsummarized_pages(FETCH_BATCH_SIZE)
    })
    .await
}

/// Summarizes ALL pages from the database, regardless of whether they're already summarized
async fn summarize_all_pages(ctx: &SummarizeContext<'_>, storage: &Storage) -> Result<u32> {
    let offset = RefCell::new(0);
    let has_more = RefCell::new(true);
    summarize_fetched_pages(ctx, storage, || {
        if !*has_more.borrow() {
            return Ok(Vec::new());
        }

        let batch = storage.fetch_pages(FETCH_BATCH_SIZE, *offset.borrow())?;
        let batch_size = batch.len();
        *offset.borrow_mut() += FETCH_BATCH_SIZE;
        if batch_size < FETCH_BATCH_SIZE as usize {
            *has_more.borrow_mut() = false;
        }

        Ok(batch)
    })
    .await
}

/// Summarizes a single page by URL
async fn summarize_single_page(
    ctx: &SummarizeContext<'_>,
    storage: &Storage,
    url: &str,
) -> Result<u32> {
    let content = match storage.fetch_page_content(url)? {
        None => return Ok(0),
        Some(content) => content,
    };
    let summary = summarize_page(url, &content, ctx).await?;
    storage.update_page_summary(url, &summary)?;
    debug!("Summarized page: {url}");
    Ok(1)
}

/// Generalized function to summarize pages using a fetcher callback
async fn summarize_fetched_pages<F>(
    ctx: &SummarizeContext<'_>,
    storage: &Storage,
    mut fetcher: F,
) -> Result<u32>
where
    F: FnMut() -> Result<Vec<(String, String)>>,
{
    let mut processed = 0;

    loop {
        let batch = fetcher()?;
        if batch.is_empty() {
            break;
        }

        for (url, content) in batch {
            let summary = summarize_page(&url, &content, ctx).await?;
            storage.update_page_summary(&url, &summary)?;
            processed += 1;
            debug!("Summarized page: {url}");
        }
    }

    Ok(processed)
}

/// Summarises a single page by formatting its URL and content using an LLM model.
///
/// # Arguments
///
/// * `url` - The URL of the page
/// * `content` - The content of the page
/// * `ctx` - Context containing model, prompt template, and rate limiter
///
/// # Returns
///
/// Returns the processed content as a string
///
/// # Errors
///
/// Returns an error if:
/// * LLM chat operation fails
/// * Regex operations fail
pub async fn summarize_page(
    url: &str,
    text: &str,
    ctx: &SummarizeContext<'_>,
) -> Result<String, anyhow::Error> {
    let prompt_template = ctx.prompt_template.unwrap_or(DEFAULT_PROMPT_TEMPLATE);
    let prompt = prompt_template
        .replace("{url}", url)
        .replace("{text}", text);

    let mut messages: Vec<ChatMessageBuilder> = vec![ChatMessage::user().content(prompt)];

    if !prompt_template.contains("{text}") {
        messages.push(ChatMessage::user().content(text));
    }

    let messages: Vec<ChatMessage> = messages
        .into_iter()
        .map(|message| message.build())
        .collect();

    if let Some(limiter) = ctx.rate_limiter {
        loop {
            match limiter.try_acquire(1) {
                Ok(()) => break,
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    let response = ctx
        .model
        .chat(&messages)
        .await
        .map_err(|err| anyhow::anyhow!("LLM error: {err}."))?
        .to_string();

    let summary = THINK_STRIPPER_REGEX
        .replace_all(&response, "")
        .to_string()
        .trim()
        .to_owned();

    Ok(summary)
}
