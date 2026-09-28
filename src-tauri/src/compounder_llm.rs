// src-tauri/src/compounder_llm.rs
//
// LLM seam for the Knowledge Compounder (SPEC-001 §5 DG-3).
//
// The compounder's pass-2 completion call is abstracted behind `LlmProvider`
// so tests (and future providers) can run without network access or API keys.

use crate::intelligence::{invoke_with_backoff, OpenRouterRequest, Priority};

/// Provider for compounder completion calls.
pub enum LlmProvider {
    /// Calls OpenRouter using `OPENROUTER_API_KEY`.
    OpenRouter,
    /// Test double: returns the canned content without any network call.
    Static(String),
}

/// Complete `prompt` with the given provider, returning the raw model content.
pub async fn complete(provider: &LlmProvider, prompt: &str) -> Result<String, String> {
    match provider {
        LlmProvider::Static(content) => Ok(content.clone()),
        LlmProvider::OpenRouter => {
            let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();
            if api_key.is_empty() {
                return Err("No OpenRouter API key configured".to_string());
            }

            let request = OpenRouterRequest {
                prompt: prompt.to_string(),
                model: None,
                priority: Priority::Normal,
                max_tokens: Some(2048),
                temperature: Some(0.3),
            };

            let resp = invoke_with_backoff(request, &api_key, 3)
                .await
                .map_err(|e| e.to_string())?;
            Ok(resp.content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_static_provider_returns_canned_content() {
        let provider = LlmProvider::Static("[]".to_string());
        let content = complete(&provider, "ignored prompt").await.unwrap();
        assert_eq!(content, "[]");
    }

    // NOTE: the OpenRouter path is intentionally not unit-tested — it would
    // require mutating the process-global OPENROUTER_API_KEY (racy under
    // parallel tests). It is covered by the real-agent acceptance run.
}
