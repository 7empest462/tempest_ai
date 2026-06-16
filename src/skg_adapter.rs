use crate::inference::{Backend, SamplingConfig};
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use skg_turn::infer::{InferRequest, InferResponse, ToolCall as SkgToolCall};
use skg_turn::provider::{Provider, ProviderError};
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct SkgBackendProvider {
    pub backend: Backend,
    pub model: String,
}

impl Provider for SkgBackendProvider {
    fn infer(
        &self,
        request: InferRequest,
    ) -> impl Future<Output = Result<InferResponse, ProviderError>> + Send {
        let backend = self.backend.clone();
        let model = self.model.clone();

        async move {
            let mut history = Vec::new();
            for msg in request.messages {
                let role = match msg.role {
                    layer0::context::Role::User => MessageRole::User,
                    layer0::context::Role::Assistant => MessageRole::Assistant,
                    layer0::context::Role::System => MessageRole::System,
                    layer0::context::Role::Tool { .. } => MessageRole::User,
                    _ => MessageRole::User,
                };

                let content = msg.content.as_text().unwrap_or_default().to_string();
                history.push(ChatMessage {
                    role,
                    content,
                    images: None,
                    tool_calls: Vec::new(),
                    thinking: None,
                });
            }

            let sampling = SamplingConfig {
                temperature: request.temperature.unwrap_or(0.7) as f32,
                top_p: 0.9,
                repeat_penalty: 1.1,
                context_size: 8192,
            };

            let stop = Arc::new(AtomicBool::new(false));
            let event_tx = Arc::new(parking_lot::Mutex::new(None));

            let sys_prompt = request.system.unwrap_or_default();

            let output = backend
                .stream_chat(crate::inference::ChatRequest {
                    model: model.clone(),
                    history,
                    sampling,
                    event_tx,
                    stop,
                    system_prompt: sys_prompt,
                    on_tool_call: None,
                    tool_registry: None,
                })
                .await
                .map_err(|e| ProviderError::Other(e.into()))?;

            let mut tool_calls = Vec::new();
            for (i, tc) in output.native_tool_calls.into_iter().enumerate() {
                tool_calls.push(SkgToolCall {
                    id: format!("call_{}", i),
                    name: tc.function.name,
                    input: tc.function.arguments,
                });
            }

            Ok(InferResponse {
                content: layer0::content::Content::text(output.content),
                tool_calls: tool_calls.clone(),
                stop_reason: if tool_calls.is_empty() {
                    skg_turn::types::StopReason::EndTurn
                } else {
                    skg_turn::types::StopReason::ToolUse
                },
                usage: skg_turn::types::TokenUsage::default(),
                model,
                cost: None,
                truncated: None,
            })
        }
    }
}
