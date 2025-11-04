use llm::{
    chat::{ChatMessage, ChatProvider, ChatResponse, Tool},
    error::LLMError,
};

#[macro_export]
macro_rules! assert_responses {
    (
        $(
            $test_name:ident : response => $response:expr, result => $result:expr
        ),+ $(,)?
    ) => {
        $(
            #[tokio::test]
            async fn $test_name() {
                let context = llamap::summarize::SummarizeContext {
                    model: &StubLlmProvider::new($response.to_owned()),
                    prompt_template: None,
                    rate_limiter: None,
                };
                let result = llamap::summarize::summarize_page("", "", &context)
                    .await
                    .expect("Expected successful processing.");

                assert_that(&result).is_equal_to($result.to_owned());
            }
        )+
    }
}

pub(crate) struct StubLlmProvider {
    response_content: String,
}

impl StubLlmProvider {
    pub fn new(response_content: String) -> Self {
        StubLlmProvider { response_content }
    }
}

impl ChatProvider for StubLlmProvider {
    fn chat<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _messages: &'life1 [ChatMessage],
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Box<dyn ChatResponse>, LLMError>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            #[derive(Debug)]
            struct StringResponse(String);

            impl ChatResponse for StringResponse {
                fn text(&self) -> Option<String> {
                    Some(self.0.clone())
                }

                fn tool_calls(&self) -> Option<Vec<llm::ToolCall>> {
                    panic!()
                }

                fn thinking(&self) -> Option<String> {
                    None
                }

                fn usage(&self) -> Option<llm::chat::Usage> {
                    None
                }
            }

            impl std::fmt::Display for StringResponse {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(formatter, "{}", self.0)
                }
            }

            Ok(Box::new(StringResponse(self.response_content.clone())) as Box<dyn ChatResponse>)
        })
    }

    fn chat_with_tools<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        _messages: &'life1 [ChatMessage],
        _tools: Option<&'life2 [Tool]>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = Result<Box<dyn ChatResponse>, LLMError>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        panic!()
    }
}
