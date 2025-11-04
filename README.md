# llamap

A CLI tool to build [llms.txt](https://llmstxt.org) from [sitemap.xml](https://www.sitemaps.org/protocol.html).

## Workflow and Examples

1. Scrape sitemap.xml URL and save their HTML to a local SQLite database.
```bash
# Scrape or re-scrape a website using sitemap and save pages to a database
llamap scrape https://www.sitemaps.org/sitemap.xml sitemaps.org.sqlite
```

2. Parse text content and title from web pages using multiple extraction methods.
```bash
# Parse or re-parse HTMLs of all stored database pages using dom_smoothie (default)
llamap parse sitemaps.org.sqlite --text-by dom_smoothie
# Re-parse a specific page using fast_html2md
llamap parse sitemaps.org.sqlite --target https://www.sitemaps.org/faq.html --text-by fast_html2md
```

3. Summarize scraped content using different LLM providers and customizable prompt.
```bash
# Summarize unsummarized pages using an LLM model
llamap summarize sitemaps.org.sqlite ollama://8b@qwen3
# Summarize all pages (including those already summarized)
llamap summarize sitemaps.org.sqlite ollama://8b@qwen3 --target all
# Summarize a specific page with a custom prompt template
llamap summarize sitemaps.org.sqlite ollama://8b@qwen3 --target=https://www.sitemaps.org/faq.html --prompt-file /path/to/prompt.txt
```

4. Compose the final llms.txt file from database summaries.
```bash
# Compose the final llms.txt file
llamap compose sitemaps.org.sqlite sitemaps.org.llms.txt
#
llamap scrape -vvv https://www.sitemaps.org/sitemap.xml sitemaps.org.sqlite
```

Also, at each step you can configure verbosity using multiple `-v` (0=error, 1=warn, 2=info, 3=debug, 4=trace).

## References

* https://emschwartz.me/comparing-13-rust-crates-for-extracting-text-from-html/

## Ideas / TODOs

- [ ] Implement [migrations](https://github.com/samgqroberts/migratio) system when needed
- [ ] Add llm_readability parser support
