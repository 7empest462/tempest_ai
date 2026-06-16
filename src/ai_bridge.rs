use miette::{Result, miette};
use std::pin::Pin;
use std::sync::Arc;
use parking_lot::Mutex;
use futures::Stream;

const FALLBACK_THOUGHT_SIGNATURE: &str = "EpIHCo8HAQw51sdknWnAK89Pxs4oLikSCAktcG09aUUkvYIDgh0WL+39AO9+YNckevcRnabXMKrue6P2NWkUojkEvpoJ97MpCwICfzZJQ79YaTQWN+syB/ycovPPp/xARYA0d7hmOtLVggf1HadAjJ9fy3RpEEmt2QxZotpGUtn46i+UAfuq2Oc/AUHmq9zDH0paELcSSbuPzTLkuUNXy6r03SRLs+QGh3MlzxCyvfDmvzIqNRszhjNFNumncV7ZgL/gmfxzHQ2AoaOamtWiqIEI1b3ULeVlG3OcGrCx2LFhqS0ve6Txh3jU1XgM9RkQlr4E+jSw8yZpoox0s8YJ1Iw8JZ2uLEANJxRbOyI7AF7MG+oyfQgGw3JiP7riplpgMCmR6xCU5YKfozXHr+STzQueCPZk1DyV6A2wZztYndOPOgjdwHhfbpM4cYppv6WNwnS3uG3359m1334cRph173p7+TeNb/r5R1xyEeT9bQVVJnXvSV2WvMF25DwWUYJsvSr87KZ2fzzA36mcnL5RcSCvoGciesdY83Yt0mZcjUtHrJhvWnaa2qy5TO+ty06HDbdPwJxPO3Oe/vjMbQ+kwYzLByBqqRZ/1bnjctP+mepsn/XR/vOedOdd8lNRff9Qyf+5Toxtwo8mx1fqxEK1qiv35g6F+Trr/2tk2VZw+3H5cazIIGediL+oJVENmzyuaKxgaB3g0+ZtctDnsLZMi/70oCH034YuMV74mrSyFQRJnu0DUw4ahEYxfwGTIRbMqmGP/AboM8Ih0SHSJ6aRV+AznQuPEfux5AVICCkjaSjm4iWVmyPsWESCYeO5lhRsYgE8rP27Mn6+AtjA1uqk6SC9C0uXpjppQiiw7lsuYzZBhgCbkS61EtN6KITRP6pSDUAUXtJseWhCQT/Kc7PvQSgvVQqhUtL/y+xyCs2ljFwSPfYz8LveB2TMnSDNQfpZtRtvXMyoKLg5pDxBbTDN7Csf8pFFVyP8iAB678QaKJgGgYEFFcql6n9IIfSfjB+DApNGIdi+VurJb93rVixjKrOOBattJfV1WkbryEDf6osQLjOUAje7sqiO1kS8LGXw5aVu52FmbqMnYVmtpQDi8P4VOlkfal6NOiSZtLMaer+aLdigQQ91TZh/OxzJCAwulklzRZpqBVCZJTeuLi/3A/5hn4h7P/RJSaihaB2ebbzKwfGwNnVOPIM29IzwzF5tdg/XT7zRbsEgQyYPQwQALM=";

pub enum ModelProvider {
    Ollama { base_url: String },
    #[allow(dead_code)]
    OpenAI { api_key: String, base_url: Option<String> },
    #[allow(dead_code)]
    Gemini { api_key: String },
    #[allow(dead_code)]
    MLX { base_url: Option<String> },
}

pub struct TempestAiBridge {
    pub reqwest_client: reqwest::Client,
    pub models: Vec<String>,
    pub base_url: String,
    pub auth_token: Option<String>,
    pub raw_history: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl Clone for TempestAiBridge {
    fn clone(&self) -> Self {
        Self {
            reqwest_client: self.reqwest_client.clone(),
            models: self.models.clone(),
            base_url: self.base_url.clone(),
            auth_token: self.auth_token.clone(),
            raw_history: std::sync::Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }
}

impl TempestAiBridge {
    pub fn new(provider: ModelProvider, models: Vec<String>) -> Result<Self> {
        let mut auth_token = None;
        let base_url_str = match &provider {
            ModelProvider::Ollama { base_url } => base_url.clone(),
            ModelProvider::OpenAI { api_key, base_url } => {
                auth_token = Some(api_key.clone());
                base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            }
            ModelProvider::Gemini { api_key } => {
                auth_token = Some(api_key.clone());
                "https://generativelanguage.googleapis.com/v1beta/openai/".to_string()
            }
            ModelProvider::MLX { base_url } => {
                base_url.clone().unwrap_or_else(|| "http://localhost:8000/v1".to_string())
            }
        };

        let req_client = reqwest::Client::new();

        Ok(Self {
            reqwest_client: req_client,
            models,
            base_url: base_url_str,
            auth_token,
            raw_history: Arc::new(Mutex::new(Vec::new())),
        })
    }

    #[allow(dead_code)]
    pub async fn chat(&self, messages: Vec<ollama_rs::generation::chat::ChatMessage>, tools: Option<Vec<ollama_rs::generation::tools::ToolInfo>>) -> Result<String> {
        let serialized_messages = serialize_chat_messages(&messages, &self.raw_history);
        
        let mut body = serde_json::json!({
            "messages": serialized_messages,
            "stream": false,
        });

        if let Some(t_vec) = tools {
            let ai_tools: Vec<serde_json::Value> = t_vec.into_iter().map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.function.name,
                        "description": t.function.description,
                        "parameters": sanitize_schema(serde_json::to_value(&t.function.parameters).unwrap_or_default()),
                    }
                })
            }).collect();
            body["tools"] = serde_json::json!(ai_tools);
        }

        let mut errors = Vec::new();
        for model in &self.models {
            let mut body_for_model = body.clone();
            body_for_model["model"] = serde_json::json!(model.clone());
            
            let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
            let mut req = self.reqwest_client.post(url).json(&body_for_model);
            if let Some(token) = &self.auth_token {
                req = req.bearer_auth(token);
            }
            
            match req.send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        let status = response.status();
                        let err_text = response.text().await.unwrap_or_default();
                        errors.push(format!("Model '{}': HTTP {} - {}", model, status, err_text));
                        continue;
                    }

                    let json: serde_json::Value = response.json().await
                        .map_err(|e| miette!("Failed to parse chat response: {}", e))?;
                    
                    let content = json["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    
                    if content.is_empty() {
                        errors.push(format!("Model '{}': No content in response", model));
                        continue;
                    }
                    
                    return Ok(content);
                }
                Err(e) => {
                    errors.push(format!("Model '{}': Request error - {}", model, e));
                    continue;
                }
            }
        }
        
        let err_msg = if errors.is_empty() {
            "AI Bridge failed: No models configured or available".to_string()
        } else {
            format!("AI Bridge failed to send chat. Attempted models:\n- {}", errors.join("\n- "))
        };
        Err(miette!(err_msg))
    }

    pub async fn stream_chat(&self, messages: Vec<ollama_rs::generation::chat::ChatMessage>, tools: Option<Vec<ollama_rs::generation::tools::ToolInfo>>) -> Result<Pin<Box<dyn Stream<Item = Result<serde_json::Value, miette::Report>> + Send>>> {
        let serialized_messages = serialize_chat_messages(&messages, &self.raw_history);
        
        let mut body = serde_json::json!({
            "messages": serialized_messages,
            "stream": true,
        });

        if let Some(t_vec) = tools {
            let ai_tools: Vec<serde_json::Value> = t_vec.into_iter().map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.function.name,
                        "description": t.function.description,
                        "parameters": sanitize_schema(serde_json::to_value(&t.function.parameters).unwrap_or_default()),
                    }
                })
            }).collect();
            body["tools"] = serde_json::json!(ai_tools);
        }

        let mut errors = Vec::new();
        for model in &self.models {
            let mut body_for_model = body.clone();
            body_for_model["model"] = serde_json::json!(model.clone());
            
            let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
            let mut req = self.reqwest_client.post(url).json(&body_for_model);
            if let Some(token) = &self.auth_token {
                req = req.bearer_auth(token);
            }
            
            let send_result = req.send().await;
            match send_result {
                Ok(response) => {
                    if !response.status().is_success() {
                        let status = response.status();
                        let err_text = response.text().await.unwrap_or_default();
                        errors.push(format!("Model '{}': HTTP {} - {}", model, status, err_text));
                        continue;
                    }

                    let stream = response.bytes_stream();
                    let mut line_buffer = String::new();
                    let raw_history_clone = self.raw_history.clone();

                    return Ok(Box::pin(async_stream::try_stream! {
                        use futures::StreamExt;
                        let mut stream = stream;
                        let mut in_thought = false;
                        
                        let mut accumulated_content = String::new();
                        let mut accumulated_reasoning = String::new();
                        let mut accumulated_tool_calls: Vec<serde_json::Value> = Vec::new();
                        let mut seen_tool_call_ids: Vec<String> = Vec::new();

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
                                        // Accumulate raw assistant chunk fields
                                        if let Some(choices) = chunk_val.get("choices").and_then(|c| c.as_array()) {
                                            for choice in choices {
                                                let delta = choice.get("delta").cloned().unwrap_or_default();
                                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                                    accumulated_content.push_str(content);
                                                }
                                                if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
                                                    accumulated_reasoning.push_str(reasoning);
                                                }
                                                if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                                    for tc in tool_calls {
                                                        let tc_id = tc.get("id").and_then(|id| id.as_str()).map(|s| s.to_string());
                                                        let idx = if let Some(id_str) = tc_id {
                                                            if let Some(pos) = seen_tool_call_ids.iter().position(|x| x == &id_str) {
                                                                pos
                                                            } else {
                                                                seen_tool_call_ids.push(id_str);
                                                                seen_tool_call_ids.len() - 1
                                                            }
                                                        } else if let Some(idx_val) = tc.get("index").and_then(|i| i.as_u64()) {
                                                            idx_val as usize
                                                        } else {
                                                            seen_tool_call_ids.len().saturating_sub(1)
                                                        };
                                                        while accumulated_tool_calls.len() <= idx {
                                                            accumulated_tool_calls.push(serde_json::json!({}));
                                                        }
                                                        merge_tool_call(&mut accumulated_tool_calls[idx], tc);
                                                    }
                                                }
                                            }
                                        }

                                        // Convert Value to ChatCompletionChunk manually to skip rigid validation
                                        let mut repaired = chunk_val.clone();
                                        if let Some(choices) = repaired.get_mut("choices").and_then(|c| c.as_array_mut()) {
                                            for choice in choices {
                                                if let Some(fr) = choice.get_mut("finish_reason")
                                                    && let Some(fr_str) = fr.as_str()
                                                    && !["stop", "length", "tool_calls", "content_filter", "function_call", "null"].contains(&fr_str)
                                                {
                                                    *fr = serde_json::Value::String("stop".to_string());
                                                }
                                                if let Some(delta) = choice.get_mut("delta").and_then(|d| d.as_object_mut()) {
                                                    if let Some(tool_calls) = delta.get_mut("tool_calls").and_then(|t| t.as_array_mut()) {
                                                        for tc in tool_calls {
                                                            let tc_id = tc.get("id").and_then(|id| id.as_str()).map(|s| s.to_string());
                                                            let resolved_idx = if let Some(id_str) = tc_id {
                                                                if let Some(pos) = seen_tool_call_ids.iter().position(|x| x == &id_str) {
                                                                    pos
                                                                } else {
                                                                    seen_tool_call_ids.push(id_str);
                                                                    seen_tool_call_ids.len() - 1
                                                                }
                                                            } else if let Some(idx_val) = tc.get("index").and_then(|i| i.as_u64()) {
                                                                idx_val as usize
                                                            } else {
                                                                seen_tool_call_ids.len().saturating_sub(1)
                                                            };

                                                            if let Some(tc_obj) = tc.as_object_mut() {
                                                                tc_obj.insert("index".to_string(), serde_json::json!(resolved_idx));
                                                            }

                                                            if let Some(func) = tc.get_mut("function").and_then(|f| f.as_object_mut())
                                                                && let Some(existing_name_val) = func.get("name")
                                                            {
                                                                let existing_name = existing_name_val.as_str().unwrap_or("");
                                                                func.insert("name".to_string(), serde_json::Value::String(format!("__idx_{}__{}", resolved_idx, existing_name)));
                                                            }
                                                        }
                                                    }

                                                    // Handle reasoning_content by wrapping it in <think> tags and mapping it to content
                                                    if let Some(reasoning) = delta.remove("reasoning_content")
                                                        && let Some(reasoning_str) = reasoning.as_str()
                                                        && !reasoning_str.is_empty()
                                                    {
                                                        let mut content_token = String::new();
                                                        if !in_thought {
                                                            content_token.push_str("<think>");
                                                            in_thought = true;
                                                        }
                                                        content_token.push_str(reasoning_str);
                                                        delta.insert("content".to_string(), serde_json::Value::String(content_token));
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

                                         yield repaired;
                                    }
                                }
                            }
                        }

                        // Stream completed successfully - construct and push final assistant message to raw history
                        let mut final_assistant_msg = serde_json::json!({
                            "role": "assistant"
                        });

                        if !accumulated_content.is_empty() {
                            final_assistant_msg["content"] = serde_json::Value::String(accumulated_content);
                        } else {
                            final_assistant_msg["content"] = serde_json::Value::Null;
                        }

                        if !accumulated_reasoning.is_empty() {
                            final_assistant_msg["reasoning_content"] = serde_json::Value::String(accumulated_reasoning);
                        }

                        if !accumulated_tool_calls.is_empty() {
                            let mut cleaned_tool_calls: Vec<serde_json::Value> = accumulated_tool_calls
                                .into_iter()
                                .filter(|tc| tc.get("id").is_some())
                                .collect();
                            for tc in &mut cleaned_tool_calls {
                                if let Some(func) = tc.get_mut("function").and_then(|f| f.as_object_mut())
                                    && let Some(name_val) = func.get_mut("name")
                                    && let Some(name_str) = name_val.as_str()
                                    && let Some(stripped) = name_str.strip_prefix("__idx_")
                                    && let Some(end) = stripped.find("__")
                                {
                                    let absolute_end = 6 + end;
                                    *name_val = serde_json::Value::String(name_str[absolute_end+2..].to_string());
                                }
                            }

                            // Gemini streams the thought signature only on the first tool call in a parallel set.
                            // However, it requires all tool calls in the final assistant message history to have a thought signature.
                            // We find the first thought signature in any of the tool calls and copy it to all of them, or use the fallback.
                            let mut thought_sig = None;
                            for tc in &cleaned_tool_calls {
                                if let Some(sig) = tc.get("extra_content")
                                    .and_then(|ec| ec.get("google"))
                                    .and_then(|g| g.get("thought_signature"))
                                    .and_then(|ts| ts.as_str()) {
                                        thought_sig = Some(sig.to_string());
                                        break;
                                    }
                            }

                            let sig = thought_sig.unwrap_or_else(|| FALLBACK_THOUGHT_SIGNATURE.to_string());
                            for tc in &mut cleaned_tool_calls {
                                let has_sig = tc.get("extra_content")
                                    .and_then(|ec| ec.get("google"))
                                    .and_then(|g| g.get("thought_signature"))
                                    .is_some();
                                if !has_sig {
                                    tc["extra_content"] = serde_json::json!({
                                        "google": {
                                            "thought_signature": sig
                                        }
                                    });
                                }
                            }

                            if !cleaned_tool_calls.is_empty() {
                                final_assistant_msg["tool_calls"] = serde_json::json!(cleaned_tool_calls);
                            }
                        }

                        raw_history_clone.lock().push(final_assistant_msg);
                    }));
                }
                Err(e) => {
                    errors.push(format!("Model '{}': Request error - {}", model, e));
                    continue;
                }
            }
        }
        
        let err_msg = if errors.is_empty() {
            "AI Bridge failed: No models configured or available".to_string()
        } else {
            format!("AI Bridge failed to stream chat. Attempted models:\n- {}", errors.join("\n- "))
        };
        Err(miette!(err_msg))
    }

    pub async fn generate_embeddings(&self, text: String) -> Result<Vec<f32>> {
        let mut errors = Vec::new();
        for model in &self.models {
            let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));
            let body = serde_json::json!({
                "model": model,
                "input": [text.clone()],
            });
            
            let mut req = self.reqwest_client.post(&url).json(&body);
            if let Some(token) = &self.auth_token {
                req = req.bearer_auth(token);
            }
            
            match req.send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        let status = response.status();
                        let err_text = response.text().await.unwrap_or_default();
                        errors.push(format!("Model '{}': HTTP {} - {}", model, status, err_text));
                        continue;
                    }

                    let json: serde_json::Value = response.json().await
                        .map_err(|e| miette!("Failed to parse embeddings response: {}", e))?;
                    
                    if let Some(data) = json["data"].as_array().and_then(|a| a.first())
                        && let Some(embedding) = data["embedding"].as_array()
                    {
                        return Ok(embedding.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect());
                    }
                    
                    errors.push(format!("Model '{}': Invalid embeddings response format", model));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("Model '{}': Embeddings request error - {}", model, e));
                    continue;
                }
            }
        }
        
        let err_msg = if errors.is_empty() {
            "AI Bridge embeddings failed: No models configured or available".to_string()
        } else {
            format!("AI Bridge embeddings failed. Attempted models:\n- {}", errors.join("\n- "))
        };
        Err(miette!(err_msg))
    }
}

fn sanitize_schema(schema: serde_json::Value) -> serde_json::Value {
    let mut schema = schema;
    if let Some(obj) = schema.as_object_mut()
        && obj.get("type").and_then(|t| t.as_str()) == Some("object")
        && !obj.contains_key("properties")
    {
        obj.insert("properties".into(), serde_json::json!({}));
    }
    schema
}

fn merge_tool_call(acc: &mut serde_json::Value, delta: &serde_json::Value) {
    if let Some(id) = delta.get("id") {
        acc["id"] = id.clone();
    }
    if let Some(ty) = delta.get("type") {
        acc["type"] = ty.clone();
    }
    if let Some(extra) = delta.get("extra_content") {
        acc["extra_content"] = extra.clone();
    }
    if let Some(func_delta) = delta.get("function") {
        if !acc["function"].is_object() {
            acc["function"] = serde_json::json!({
                "name": "",
                "arguments": ""
            });
        }
        let acc_func = acc["function"].as_object_mut().unwrap();
        if let Some(name) = func_delta.get("name") {
            acc_func.insert("name".to_string(), name.clone());
        }
        if let Some(args_delta) = func_delta.get("arguments").and_then(|a| a.as_str()) {
            let mut current_args = acc_func.get("arguments").and_then(|a| a.as_str()).unwrap_or("").to_string();
            current_args.push_str(args_delta);
            acc_func.insert("arguments".to_string(), serde_json::Value::String(current_args));
        }
    }
}

fn target_role(msg: &ollama_rs::generation::chat::ChatMessage) -> &'static str {
    match msg.role {
        ollama_rs::generation::chat::MessageRole::System => {
            if parse_tool_name_from_system_msg(&msg.content).is_some() {
                "tool"
            } else {
                "system"
            }
        }
        ollama_rs::generation::chat::MessageRole::User => "user",
        ollama_rs::generation::chat::MessageRole::Assistant => "assistant",
        ollama_rs::generation::chat::MessageRole::Tool => "tool",
    }
}

fn is_retrieved_context_system_msg(val: &serde_json::Value) -> bool {
    if val.get("role").and_then(|r| r.as_str()) == Some("system")
        && let Some(content) = val.get("content").and_then(|c| c.as_str())
        && content.starts_with("### [RETRIEVED HISTORICAL CONTEXT]")
    {
        return true;
    }
    false
}

fn is_retrieved_context_chat_msg(msg: &ollama_rs::generation::chat::ChatMessage) -> bool {
    if msg.role == ollama_rs::generation::chat::MessageRole::System
        && msg.content.starts_with("### [RETRIEVED HISTORICAL CONTEXT]")
    {
        return true;
    }
    false
}

fn messages_match(raw_msg: &serde_json::Value, chat_msg: &ollama_rs::generation::chat::ChatMessage) -> bool {
    let raw_role = match raw_msg.get("role").and_then(|r| r.as_str()) {
        Some(r) => r,
        None => return false,
    };
    let target = target_role(chat_msg);
    if raw_role != target {
        return false;
    }
    if target == "user" {
        let raw_content = raw_msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
        if raw_content != chat_msg.content && !raw_content.ends_with(&chat_msg.content) && !chat_msg.content.ends_with(raw_content) {
            return false;
        }
    } else if target == "tool" {
        let raw_content = raw_msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
        if raw_content != chat_msg.content {
            return false;
        }
    }
    true
}

fn serialize_chat_messages(
    messages: &[ollama_rs::generation::chat::ChatMessage],
    raw_history: &Arc<Mutex<Vec<serde_json::Value>>>,
) -> Vec<serde_json::Value> {
    let mut raw_hist = raw_history.lock();

    // 1. Align raw_hist based on messages using a suffix search with two pointers.
    let mut best_offset = raw_hist.len();
    let mut best_msg_idx = 0;
    
    for offset in 0..=raw_hist.len() {
        let mut p_raw = offset;
        let mut p_msg = 0;
        let mut all_match = true;

        while p_raw < raw_hist.len() && p_msg < messages.len() {
            if is_retrieved_context_system_msg(&raw_hist[p_raw]) {
                p_raw += 1;
                continue;
            }
            if is_retrieved_context_chat_msg(&messages[p_msg]) {
                p_msg += 1;
                continue;
            }

            if messages_match(&raw_hist[p_raw], &messages[p_msg]) {
                p_raw += 1;
                p_msg += 1;
            } else {
                all_match = false;
                break;
            }
        }

        while p_raw < raw_hist.len() {
            if is_retrieved_context_system_msg(&raw_hist[p_raw]) {
                p_raw += 1;
            } else {
                break;
            }
        }

        if all_match && p_raw == raw_hist.len() {
            best_offset = offset;
            best_msg_idx = p_msg;
            break;
        }
    }

    // Keep only the matching suffix
    if best_offset > 0 {
        if best_offset < raw_hist.len() {
            *raw_hist = raw_hist[best_offset..].to_vec();
        } else {
            raw_hist.clear();
        }
    }

    // Sync system message contents to ensure latest prompt/schema/context is sent, and ensure assistant tool calls have thought signatures.
    // We use two pointers to skip dynamically injected system messages.
    {
        let mut p_raw = 0;
        let mut p_msg = 0;
        while p_raw < raw_hist.len() && p_msg < best_msg_idx {
            if is_retrieved_context_system_msg(&raw_hist[p_raw]) {
                p_raw += 1;
                continue;
            }
            if is_retrieved_context_chat_msg(&messages[p_msg]) {
                p_msg += 1;
                continue;
            }
            
            let role = raw_hist[p_raw].get("role").and_then(|r| r.as_str());
            if role == Some("system") {
                raw_hist[p_raw]["content"] = serde_json::json!(messages[p_msg].content);
            } else if role == Some("assistant")
                && let Some(tool_calls) = raw_hist[p_raw].get_mut("tool_calls").and_then(|tc| tc.as_array_mut())
            {
                for tc in tool_calls {
                    let has_sig = tc.get("extra_content")
                        .and_then(|ec| ec.get("google"))
                        .and_then(|g| g.get("thought_signature"))
                        .is_some();
                    if !has_sig {
                        tc["extra_content"] = serde_json::json!({
                            "google": {
                                "thought_signature": FALLBACK_THOUGHT_SIGNATURE
                            }
                        });
                    }
                }
            }
            
            p_raw += 1;
            p_msg += 1;
        }
    }

    // 2. Process and serialize the remaining messages
    let mut call_counter = 0;
    for val in raw_hist.iter() {
        if let Some(tool_calls) = val.get("tool_calls").and_then(|t| t.as_array()) {
            call_counter += tool_calls.len();
        }
    }

    let mut unmatched_calls: Vec<serde_json::Value> = Vec::new();
    for val in raw_hist.iter() {
        let role = val.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role == "assistant" {
            if let Some(tool_calls) = val.get("tool_calls").and_then(|t| t.as_array()) {
                for tc in tool_calls {
                    unmatched_calls.push(tc.clone());
                }
            }
        } else if role == "tool"
            && let Some(tool_call_id) = val.get("tool_call_id").and_then(|id| id.as_str())
            && let Some(pos) = unmatched_calls.iter().position(|tc| tc.get("id").and_then(|id| id.as_str()) == Some(tool_call_id))
        {
            unmatched_calls.remove(pos);
        }
    }

    let start_idx = best_msg_idx;
    for msg in &messages[start_idx..] {
        match msg.role {
            ollama_rs::generation::chat::MessageRole::System => {
                let mut is_tool_result = false;
                if let Some(tool_name) = parse_tool_name_from_system_msg(&msg.content)
                    && let Some(pos) = unmatched_calls.iter().rposition(|tc| {
                        tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) == Some(&tool_name)
                    })
                {
                    let tc = unmatched_calls.remove(pos);
                    let call_id = tc.get("id").and_then(|id| id.as_str()).unwrap_or("");
                    
                    raw_hist.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "name": tool_name,
                        "content": msg.content
                    }));
                    is_tool_result = true;
                }
                
                if !is_tool_result {
                    raw_hist.push(serde_json::json!({
                        "role": "system",
                        "content": msg.content
                    }));
                }
            }
            ollama_rs::generation::chat::MessageRole::User => {
                raw_hist.push(serde_json::json!({
                    "role": "user",
                    "content": msg.content
                }));
            }
            ollama_rs::generation::chat::MessageRole::Assistant => {
                let mut msg_json = serde_json::json!({
                    "role": "assistant"
                });

                if !msg.content.is_empty() {
                    msg_json["content"] = serde_json::json!(msg.content);
                } else {
                    msg_json["content"] = serde_json::Value::Null;
                }

                if !msg.tool_calls.is_empty() {
                    let mut tool_calls_json = Vec::new();
                    for call in &msg.tool_calls {
                        call_counter += 1;
                        let call_id = format!("call_{}", call_counter);
                        
                        let mut name = call.function.name.clone();
                        if let Some(stripped) = name.strip_prefix("__idx_")
                            && let Some(end) = stripped.find("__")
                        {
                            let absolute_end = 6 + end;
                            name = name[absolute_end+2..].to_string();
                        }
                        
                        let tc_json = serde_json::json!({
                            "id": call_id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": call.function.arguments.to_string()
                            },
                            "extra_content": {
                                "google": {
                                    "thought_signature": FALLBACK_THOUGHT_SIGNATURE
                                }
                            }
                        });
                        unmatched_calls.push(tc_json.clone());
                        tool_calls_json.push(tc_json);
                    }
                    msg_json["tool_calls"] = serde_json::json!(tool_calls_json);
                }

                raw_hist.push(msg_json);
            }
            ollama_rs::generation::chat::MessageRole::Tool => {
                let mut matched_call_id = None;
                if !unmatched_calls.is_empty() {
                    let tc = unmatched_calls.remove(0);
                    matched_call_id = tc.get("id").and_then(|id| id.as_str()).map(|s| s.to_string());
                }

                let call_id = matched_call_id.unwrap_or_else(|| {
                    call_counter += 1;
                    format!("call_{}", call_counter)
                });

                raw_hist.push(serde_json::json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": msg.content
                }));
            }
        }
    }

    raw_hist.clone()
}

fn parse_tool_name_from_system_msg(content: &str) -> Option<String> {
    if content.starts_with("=== SYSTEM OBSERVATION ===\nTool: ")
        || content.starts_with("=== SYSTEM ERROR ===\nTool: ")
    {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() > 1 {
            let tool_line = lines[1];
            if let Some(stripped) = tool_line.strip_prefix("Tool: ") {
                return Some(stripped.trim().to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ollama_rs::generation::chat::{ChatMessage, MessageRole};
    use ollama_rs::generation::tools::{ToolCall, ToolCallFunction};

    #[test]
    fn test_serialize_chat_messages_preserves_ids_and_aligns() {
        let raw_history = Arc::new(Mutex::new(Vec::new()));

        // Simulate initial System prompt and User turn
        let mut messages = vec![
            ChatMessage::new(MessageRole::System, "Initial System Prompt".to_string()),
            ChatMessage::new(MessageRole::User, "Hello".to_string()),
        ];
        
        let serialized = serialize_chat_messages(&messages, &raw_history);
        assert_eq!(serialized.len(), 2);
        assert_eq!(serialized[0]["role"], "system");
        assert_eq!(serialized[0]["content"], "Initial System Prompt");
        assert_eq!(serialized[1]["role"], "user");
        assert_eq!(serialized[1]["content"], "Hello");

        // Simulate Assistant response with a tool call from the model
        let original_call_id = "gemini_call_abc123";
        {
            let mut raw_hist = raw_history.lock();
            raw_hist.push(serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": original_call_id,
                        "type": "function",
                        "function": {
                            "name": "run_command",
                            "arguments": "{\"command\":\"ls\"}"
                        },
                        "extra_content": {
                            "google": {
                                "thought_signature": "cryptosig456"
                            }
                        }
                    }
                ]
            }));
        }

        // The agent appends the assistant message to messages list (simplified form)
        let mut assistant_msg = ChatMessage::new(MessageRole::Assistant, "".to_string());
        assistant_msg.tool_calls = vec![
            ToolCall {
                function: ToolCallFunction {
                    name: "run_command".to_string(),
                    arguments: serde_json::json!({"command":"ls"}),
                }
            }
        ];
        messages.push(assistant_msg);

        // And then the agent executes the tool and appends the system observation
        let tool_obs = ChatMessage::new(
            MessageRole::System,
            "=== SYSTEM OBSERVATION ===\nTool: run_command\nOutput: file1.txt".to_string(),
        );
        messages.push(tool_obs);

        // Before serializing, simulate system prompt content update/drift (e.g. runway report injected)
        messages[0].content = "Updated System Prompt with Runway Report".to_string();

        // Serialize the full history:
        let serialized = serialize_chat_messages(&messages, &raw_history);
        assert_eq!(serialized.len(), 4);
        
        // Assertions:
        assert_eq!(serialized[0]["role"], "system");
        assert_eq!(serialized[0]["content"], "Updated System Prompt with Runway Report"); // Verified prompt drift updated

        assert_eq!(serialized[1]["role"], "user");
        
        assert_eq!(serialized[2]["role"], "assistant");
        assert_eq!(serialized[2]["tool_calls"][0]["id"], original_call_id);
        assert_eq!(serialized[2]["tool_calls"][0]["extra_content"]["google"]["thought_signature"], "cryptosig456");

        assert_eq!(serialized[3]["role"], "tool");
        assert_eq!(serialized[3]["tool_call_id"], original_call_id);
        assert_eq!(serialized[3]["name"], "run_command");
        assert!(serialized[3]["content"].as_str().unwrap().contains("file1.txt"));

        // Simulate history compaction (truncating the first two messages: system and user)
        messages.remove(0); // remove system prompt
        messages.remove(0); // remove user message
        let compacted_serialized = serialize_chat_messages(&messages, &raw_history);
        
        assert_eq!(compacted_serialized.len(), 2);
        assert_eq!(compacted_serialized[0]["role"], "assistant");
        assert_eq!(compacted_serialized[0]["tool_calls"][0]["id"], original_call_id);
        assert_eq!(compacted_serialized[1]["role"], "tool");
        assert_eq!(compacted_serialized[1]["tool_call_id"], original_call_id);
    }

    #[test]
    fn test_serialize_chat_messages_demangles_tool_names() {
        let raw_history = Arc::new(Mutex::new(Vec::new()));

        let messages = vec![
            ChatMessage::new(MessageRole::System, "System Prompt".to_string()),
            ChatMessage::new(MessageRole::User, "Hello".to_string()),
            {
                let mut assistant_msg = ChatMessage::new(MessageRole::Assistant, "".to_string());
                assistant_msg.tool_calls = vec![
                    ToolCall {
                        function: ToolCallFunction {
                            name: "__idx_0__read_file".to_string(),
                            arguments: serde_json::json!({"path":"foo.txt"}),
                        }
                    }
                ];
                assistant_msg
            }
        ];

        let serialized = serialize_chat_messages(&messages, &raw_history);
        assert_eq!(serialized.len(), 3);
        assert_eq!(serialized[2]["role"], "assistant");
        assert_eq!(serialized[2]["tool_calls"][0]["function"]["name"], "read_file");
    }

    #[test]
    fn test_serialize_chat_messages_with_injected_contexts() {
        let raw_history = Arc::new(Mutex::new(Vec::new()));

        // Turn 1:
        // raw_hist gets populated with decorated user message and injected system message
        let messages_turn1 = vec![
            ChatMessage::new(MessageRole::System, "Initial System Prompt".to_string()),
            // Dynamic context system message
            ChatMessage::new(MessageRole::System, "### [RETRIEVED HISTORICAL CONTEXT]\nSome memory".to_string()),
            // Decorated user message
            ChatMessage::new(MessageRole::User, "### [EDITOR GROUND TRUTH] ###\ncode\n### [END EDITOR CONTEXT] ###\n\n [USER] Hello".to_string()),
        ];

        let serialized_turn1 = serialize_chat_messages(&messages_turn1, &raw_history);
        assert_eq!(serialized_turn1.len(), 3);
        assert_eq!(serialized_turn1[1]["role"], "system");
        assert_eq!(serialized_turn1[2]["role"], "user");

        // Simulate assistant response
        {
            let mut raw_hist = raw_history.lock();
            raw_hist.push(serde_json::json!({
                "role": "assistant",
                "content": "Hi there",
            }));
        }

        // Turn 2:
        // messages list is built from persistent clean history plus current turn context.
        // Therefore, Turn 1 user message is now clean.
        // Turn 1 retrieved context system message is NOT present.
        // Turn 2 user message is clean.
        let messages_turn2 = vec![
            ChatMessage::new(MessageRole::System, "Initial System Prompt".to_string()),
            ChatMessage::new(MessageRole::User, "Hello".to_string()),
            ChatMessage::new(MessageRole::Assistant, "Hi there".to_string()),
            ChatMessage::new(MessageRole::User, "How are you?".to_string()),
        ];

        let serialized_turn2 = serialize_chat_messages(&messages_turn2, &raw_history);
        
        // Suffix alignment should match and preserve all of raw_history (including the assistant signature if any).
        // Let's verify that the new message is appended.
        assert_eq!(serialized_turn2.len(), 5); // system prompt, retrieved context, decorated user, assistant, new user
        assert_eq!(serialized_turn2[3]["role"], "assistant");
        assert_eq!(serialized_turn2[3]["content"], "Hi there");
        assert_eq!(serialized_turn2[4]["role"], "user");
        assert_eq!(serialized_turn2[4]["content"], "How are you?");
    }

    #[test]
    fn test_mlx_provider_url() {
        let provider_default = ModelProvider::MLX { base_url: None };
        let bridge_default = TempestAiBridge::new(provider_default, vec!["mlx-model".to_string()]).unwrap();
        assert_eq!(bridge_default.base_url, "http://localhost:8000/v1");

        let provider_custom = ModelProvider::MLX { base_url: Some("http://127.0.0.1:8080/v1".to_string()) };
        let bridge_custom = TempestAiBridge::new(provider_custom, vec!["mlx-model".to_string()]).unwrap();
        assert_eq!(bridge_custom.base_url, "http://127.0.0.1:8080/v1");
    }
}
