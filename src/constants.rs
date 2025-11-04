pub const MODEL_API_KEY_ENV_NAME: &str = "LLAMAP_MODEL_API_KEY";

pub(crate) const THINK_STRIPPER: &str = r"<think>[\s\S]*</think>\s*";

pub(crate) const DEFAULT_PROMPT_TEMPLATE: &str = r#"
You will see a webpage content from {url}.
Create its concise summary for a digest.
Your answer should contain only summary, it will be pasted directly into digest.
Nobody should know it was generated using an LLM.
Try your best to keep original style and language.
Webpage content to summarize:"#;
