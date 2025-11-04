//! llamap is a CLI tool that allows users to scrape websites using sitemap.xml
//! and compose the results into an llms.txt file for AI crawlers.
//!
//! The tool has two main commands:
//! 1. `scrape` - Scrapes a website using its sitemap and saves pages to a local database
//! 2. `compose` - Processes scraped pages and composes results to a file

extern crate spider;

use std::fs;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use env_logger::Builder;
use llm::builder::{LLMBackend, LLMBuilder};
use log::{LevelFilter, info};
use spider::tokio;
use std::str::FromStr;
use url::Url;

use llamap::{
    ParseTarget, SummarizeTarget, TextBy, compose::compose, constants::MODEL_API_KEY_ENV_NAME,
    parse::parse_db_html, scrape::process_sitemap, summarize::summarize,
};
use scraper::Selector as ScraperSelector;

/// A CLI tool to build llms.txt from sitemap.xml
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The command to execute (scrape or compose)
    #[command(subcommand)]
    command: Command,

    #[arg(long, short, action = clap::ArgAction::Count, help = "Output v(v...)erbosity: error (0), warn (1), info (2), debug (3), trace (4)", global = true, default_value_t = 2)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Command {
    /// Scrape a website using sitemap and save pages to a local database
    Scrape {
        /// The sitemap URL to scrape
        url: String,
        /// Path to database file to store pages data
        db: String,
        /// Delay between requests in milliseconds (rate limiting)
        #[arg(long, short, default_value_t = 1000)]
        delay: u64,
        /// Number of concurrent requests (default: 1)
        #[arg(long, short, default_value_t = 1)]
        concurrency: usize,
    },
    /// Parse/re-extract content from HTML in the database
    Parse {
        /// Path to database file to read pages from
        db: String,
        /// Target to parse: "all" (default) or specify an URL
        #[arg(long, short = 't', default_value = "all")]
        target: ParseTarget,
        /// Text extraction method: "dom_smoothie" (default) or "fast_html2md"
        #[arg(long, default_value = "dom_smoothie")]
        text_by: TextBy,
        /// CSS selector to limit the HTML subset from which content is extracted (optional)
        #[arg(long, short)]
        selector: Option<String>,
    },
    /// Summarize scraped pages using an LLM model and store the summary in the database
    Summarize {
        /// Path to database file to read pages from
        db: String,
        /// URL of the LLM model to use for processing
        model: String,
        /// Path to the file with a prompt template
        #[arg(long, short = 'p')]
        prompt_file: Option<String>,
        /// Target to summarize: "unsummarized", "all" or specify an URL
        #[arg(long, short = 't', default_value = "unsummarized")]
        target: SummarizeTarget,
        /// Rate limit: requests per minute (default: no limit)
        #[arg(long, short = 'r')]
        rpm: Option<u32>,
    },
    /// Process scraped pages and composes results to a file
    Compose {
        /// Path to database file to read pages from
        db: String,
        /// Path to output file to compose results to
        output_file: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    Builder::new()
        .filter_level(match cli.verbose {
            0 => LevelFilter::Error,
            1 => LevelFilter::Warn,
            2 => LevelFilter::Info,
            3 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        })
        .init();

    match cli.command {
        Command::Scrape {
            db,
            url,
            delay,
            concurrency,
        } => {
            process_sitemap(
                Url::parse(&url).map_err(|e| anyhow::anyhow!("Invalid sitemap url: {}", e))?,
                &db,
                delay,
                concurrency,
            )
            .await
        }
        Command::Parse {
            db,
            target,
            text_by,
            selector,
        } => handle_parse_command(db, target, text_by, selector).await,
        Command::Summarize {
            db,
            model,
            prompt_file,
            target,
            rpm,
        } => handle_summarize_command(db, model, prompt_file, target, rpm).await,
        Command::Compose { db, output_file } => compose(&db, &output_file).await,
    }
}

async fn handle_parse_command(
    db: String,
    target: ParseTarget,
    text_by: TextBy,
    selector_query: Option<String>,
) -> Result<()> {
    let selector = match selector_query {
        Some(selector_query) => Some(
            ScraperSelector::parse(&selector_query)
                .map_err(|e| anyhow::anyhow!("Invalid CSS selector: {}", e))?,
        ),
        None => None,
    };
    parse_db_html(&db, target, text_by, &selector).await
}

async fn handle_summarize_command(
    db: String,
    model: String,
    prompt_file: Option<String>,
    target: SummarizeTarget,
    rpm: Option<u32>,
) -> Result<()> {
    let model_url = Url::parse(&model).map_err(|e| anyhow::anyhow!("Invalid model URL: {}", e))?;
    let llm_builder = LLMBuilder::new()
        .backend(
            LLMBackend::from_str(model_url.scheme())
                .map_err(|e| anyhow::anyhow!("Invalid LLM backend: {}", e))?,
        )
        .model(
            [
                model_url
                    .host_str()
                    .context("Specify model name as host URL.")?,
                model_url.username(),
            ]
            .iter()
            .filter(|x| !x.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(":"),
        );

    let llm_builder = match std::env::var(MODEL_API_KEY_ENV_NAME) {
        Ok(model_key) => {
            info!("API KEY is provided {model_key}");
            llm_builder.api_key(model_key)
        }
        Err(err) => {
            info!("{err} while providing api key");
            llm_builder
        }
    };

    let prompt_template = match prompt_file {
        Some(file) => {
            let content =
                fs::read_to_string(&file).context(format!("Failed to read prompt file: {file}"))?;
            Some(content)
        }
        None => None,
    };

    summarize(&db, llm_builder, prompt_template.as_deref(), target, rpm).await
}
