use ai::chat_completions::{ChatCompletion, ChatCompletionMessage, ChatCompletionChunk, ChatCompletionTool, ChatCompletionToolFunctionDefinition};
use ai::embeddings::{Embeddings, EmbeddingsRequestBuilder};
use ai::clients::ollama::Client as OllamaClient;
use ai::clients::openai::Client as OpenAIClient;
use miette::{Result, IntoDiagnostic, miette};
use std::pin::Pin;
use futures::Stream;
use dyn_clone::DynClone;

pub trait UnifiedAiClient: ChatCompletion + Embeddings + DynClone + Send + Sync {}
impl<T: ChatCompletion + Embeddings + DynClone + Send + Sync> UnifiedAiClient for T {}
dyn_clone::clone_trait_object!(UnifiedAiClient);

pub enum ModelProvider {
    Ollama { base_url: String },
    #[allow(dead_code)]
    OpenAI { api_key: String, base_url: Option<String> },
    #[allow(dead_code)]
    Gemini { api_key: String },
    #[allow(dead_code)]
    MLX,
}

pub struct TempestAiBridge {
    pub client: Box<dyn UnifiedAiClient>,
    pub reqwest_client: reqwest::Client,
    pub model_name: String,
    pub base_url: String,
    pub auth_token: Option<String>,
}

impl Clone for TempestAiBridge {
    fn clone(&self) -> Self {
        Self {
            client: dyn_clone::clone_box(&*self.client),
            reqwest_client: self.reqwest_client.clone(),
            model_name: self.model_name.clone(),
            base_url: self.base_url.clone(),
            auth_token: self.auth_token.clone(),
        }
    }
}

impl TempestAiBridge {
    pub fn new(provider: ModelProvider, model_name: String) -> Result<Self> {
        let mut auth_token = None;
        let client: Box<dyn UnifiedAiClient> = match provider {
            ModelProvider::Ollama { ref base_url } => {
                Box::new(OllamaClient::from_url(base_url).into_diagnostic()?)
            }
            ModelProvider::OpenAI { ref api_key, ref base_url } => {
                auth_token = Some(api_key.clone());
                let client = if let Some(url) = base_url {
                    OpenAIClient::from_url(api_key, url).into_diagnostic()?
                } else {
                    OpenAIClient::new(api_key).into_diagnostic()?
                };
                Box::new(client)
            }
            ModelProvider::Gemini { ref api_key } => {
                auth_token = Some(api_key.clone());
                let client = OpenAIClient::from_url(api_key, "https://generativelanguage.googleapis.com/v1beta/openai/").into_diagnostic()?;
                Box::new(client)
            }
            ModelProvider::MLX => {
                return Err(miette!("MLX provider not yet implemented in AI Bridge. Use native MLX backend for now."));
            }
        };

        let req_client = reqwest::Client::new();
        let base_url_str = match &provider {
            ModelProvider::Ollama { base_url } => base_url.clone(),
            ModelProvider::OpenAI { base_url, .. } => base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            ModelProvider::Gemini { .. } => "https://generativelanguage.googleapis.com/v1beta/openai/".to_string(),
            ModelProvider::MLX => "".to_string(),
        };

        Ok(Self { 
            client, 
            reqwest_client: req_client, 
            model_name,
            base_url: base_url_str,
            auth_token,
        })
    }

    #[allow(dead_code)]
    pub async fn chat(&self, messages: Vec<ChatCompletionMessage>, tools: Option<Vec<ollama_rs::generation::tools::ToolInfo>>) -> Result<String> {
        use ai::chat_completions::ChatCompletionRequestBuilder;
        
        let mut builder = ChatCompletionRequestBuilder::default();
        builder.model(self.model_name.clone());
        builder.messages(messages);
        builder.stream(false);

        if let Some(t_vec) = tools {
            let ai_tools: Vec<ChatCompletionTool> = t_vec.into_iter().map(|t| {
                ChatCompletionTool::Function {
                    function: ChatCompletionToolFunctionDefinition {
                        name: t.function.name,
                        description: Some(t.function.description),
                        parameters: Some(sanitize_schema(serde_json::to_value(&t.function.parameters).unwrap_or_default())),
                        strict: None,
                    }
                }
            }).collect();
            builder.tools(ai_tools);
        }

        let request = builder.build()
            .map_err(|e| miette!("AI Bridge Request Build Error (Model: {}): {}", self.model_name, e))?;

        let response = self.client.chat_completions(&request).await.map_err(|e| {
            miette!("AI Bridge Chat Error (Model: {}): {}. Check if the model is loaded in LM Studio.", self.model_name, e)
        })?;
        
        response.choices.first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| miette!("AI Bridge: No content in response"))
    }

    pub async fn stream_chat(&self, messages: Vec<ChatCompletionMessage>, tools: Option<Vec<ollama_rs::generation::tools::ToolInfo>>) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, miette::Report>> + Send>>> {
        use ai::chat_completions::ChatCompletionRequestBuilder;
        
        let mut builder = ChatCompletionRequestBuilder::default();
        builder.model(self.model_name.clone());
        builder.messages(messages);
        builder.stream(true);

        if let Some(t_vec) = tools {
            let ai_tools: Vec<ChatCompletionTool> = t_vec.into_iter().map(|t| {
                ChatCompletionTool::Function {
                    function: ChatCompletionToolFunctionDefinition {
                        name: t.function.name,
                        description: Some(t.function.description),
                        parameters: Some(sanitize_schema(serde_json::to_value(&t.function.parameters).unwrap_or_default())),
                        strict: None,
                    }
                }
            }).collect();
            builder.tools(ai_tools);
        }
            
        let request = builder.build()
            .map_err(|e| miette!("AI Bridge Stream Build Error (Model: {}): {}", self.model_name, e))?;

        // We implement a manual SSE parser because the 'ai' crate's parser is too rigid 
        // and fails when LM Studio sends tool_call deltas without a 'name' field.
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut req = self.reqwest_client.post(url).json(&request);
        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }
        
        let response = req.send()
            .await
            .map_err(|e| miette!("AI Bridge Stream Error: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            return Err(miette!("AI Bridge Stream HTTP Error ({}): {}", status, err_text));
        }

        let stream = response.bytes_stream();
        let mut line_buffer = String::new();

        Ok(Box::pin(async_stream::try_stream! {
            use futures::StreamExt;
            let mut stream = stream;
            let mut in_thought = false;
            while let Some(chunk_res) = stream.next().await {
                let chunk_bytes = chunk_res.map_err(|e| miette!("SSE Chunk Error: {}", e))?;
                let text = String::from_utf8_lossy(&chunk_bytes);
                line_buffer.push_str(&text);

                while let Some(pos) = line_buffer.find('\n') {
                    let line = line_buffer[..pos].trim().to_string();
                    line_buffer = line_buffer[pos + 1..].to_string();

                    if line.starts_with("data: ") {
                        let data = line.strip_prefix("data: ").unwrap().trim();
                        if data == "[DONE]" {
                            break;
                        }

                        // Use a lenient parser that ignores missing fields
                        if let Ok(chunk_val) = serde_json::from_str::<serde_json::Value>(data) {
                            // Convert Value to ChatCompletionChunk manually to skip rigid validation
                            let mut repaired = chunk_val.clone();
                            if let Some(choices) = repaired.get_mut("choices").and_then(|c| c.as_array_mut()) {
                                for choice in choices {
                                    if let Some(fr) = choice.get_mut("finish_reason") {
                                        if let Some(fr_str) = fr.as_str() {
                                            if !["stop", "length", "tool_calls", "content_filter", "function_call", "null"].contains(&fr_str) {
                                                *fr = serde_json::Value::String("stop".to_string());
                                            }
                                        }
                                    }
                                    if let Some(delta) = choice.get_mut("delta").and_then(|d| d.as_object_mut()) {
                                        if let Some(tool_calls) = delta.get_mut("tool_calls").and_then(|t| t.as_array_mut()) {
                                            for tc in tool_calls {
                                                if let Some(func) = tc.get_mut("function").and_then(|f| f.as_object_mut()) {
                                                    if !func.contains_key("name") {
                                                        func.insert("name".to_string(), serde_json::Value::String("".to_string()));
                                                    }
                                                }
                                            }
                                        }

                                        // Handle reasoning_content by wrapping it in <think> tags and mapping it to content
                                        if let Some(reasoning) = delta.remove("reasoning_content") {
                                            if let Some(reasoning_str) = reasoning.as_str() {
                                                if !reasoning_str.is_empty() {
                                                    let mut content_token = String::new();
                                                    if !in_thought {
                                                        content_token.push_str("<think>");
                                                        in_thought = true;
                                                    }
                                                    content_token.push_str(reasoning_str);
                                                    delta.insert("content".to_string(), serde_json::Value::String(content_token));
                                                }
                                            }
                                        } else if in_thought {
                                            let mut content_token = String::new();
                                            content_token.push_str("</think>");
                                            if let Some(std_content) = delta.get("content").and_then(|c| c.as_str()) {
                                                content_token.push_str(std_content);
                                            }
                                            delta.insert("content".to_string(), serde_json::Value::String(content_token));
                                            in_thought = false;
                                        }
                                    }
                                }
                            }

                            match serde_json::from_value::<ChatCompletionChunk>(repaired) {
                                Ok(chunk) => yield chunk,
                                Err(e) => {
                                    eprintln!("⚠️ AI Bridge: Failed to deserialize ChatCompletionChunk: {}. Value: {:?}", e, chunk_val);
                                }
                            }
                        }
                    }
                }
            }
        }))
    }

    pub async fn generate_embeddings(&self, text: String) -> Result<Vec<f32>> {
        let request = EmbeddingsRequestBuilder::default()
            .model(self.model_name.clone())
            .input(vec![text])
            .build()
            .map_err(|e| miette!("AI Bridge Embeddings Request Build Error: {}", e))?;

        let response = self.client.create_embeddings(&request).await.map_err(|e| miette!("AI Bridge Embeddings Error: {}", e))?;
        
        if let Some(data) = response.data.first() {
            // Convert Vec<f64> to Vec<f32>
            Ok(data.embedding.iter().map(|&f| f as f32).collect())
        } else {
            Err(miette!("AI Bridge: No embeddings returned"))
        }
    }
}

fn sanitize_schema(schema: serde_json::Value) -> serde_json::Value {
    let mut schema = schema;
    if let Some(obj) = schema.as_object_mut() {
        if obj.get("type").and_then(|t| t.as_str()) == Some("object") {
            if !obj.contains_key("properties") {
                obj.insert("properties".into(), serde_json::json!({}));
            }
        }
    }
    schema
}
