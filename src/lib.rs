//! OpenAI Unofficial Provider Extension for ABK
//!
//! This extension provides OpenAI-compatible API communication for ABK agents.
//! It supports:
//! - Standard OpenAI chat completions API
//! - Function/tool calling
//! - Streaming responses
//!
//! Headers: Only standard headers (Authorization, Content-Type)
//! NO GitHub Copilot-specific headers (X-Request-Id, X-Initiator, etc.)

wit_bindgen::generate!({
    world: "provider-extension",
    path: "wit",
});

use exports::abk::extension::core::{ExtensionMetadata, Guest as CoreGuest};
use exports::abk::extension::provider::{
    AssistantMessage as WitAssistantMessage, Config as WitConfig, ContentDelta as WitContentDelta,
    Guest as ProviderGuest, Message as WitMessage, ProviderError, Tool as WitTool,
    ToolCall as WitToolCall,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The OpenAI provider extension implementation
struct OpenAIProvider;

// Export the component
export!(OpenAIProvider);

// ===== Core Interface Implementation =====

impl CoreGuest for OpenAIProvider {
    fn get_metadata() -> ExtensionMetadata {
        ExtensionMetadata {
            id: "openai-unofficial".to_string(),
            name: "OpenAI Unofficial Provider".to_string(),
            version: "0.1.0".to_string(),
            api_version: "0.3.0".to_string(),
            description: "OpenAI-compatible API provider with streaming and function calling"
                .to_string(),
        }
    }

    fn list_capabilities() -> Vec<String> {
        vec!["provider".to_string()]
    }

    fn init() -> Result<(), String> {
        Ok(())
    }
}

// ===== Provider Interface Implementation =====

impl ProviderGuest for OpenAIProvider {
    /// Get provider metadata as JSON
    fn get_provider_metadata() -> String {
        let metadata = json!({
            "name": "openai-unofficial",
            "version": "0.1.0",
            "description": "OpenAI-compatible API provider - works with any OpenAI-compatible endpoint",
            "supported_models": "any",
            "features": {
                "streaming": true,
                "function_calling": true,
                "vision": false
            },
            "default_model": "gpt-4o-mini"
        });
        serde_json::to_string(&metadata).unwrap_or_default()
    }

    /// Format request for OpenAI API
    fn format_request(
        messages: Vec<WitMessage>,
        config: WitConfig,
        tools: Option<Vec<WitTool>>,
    ) -> Result<String, ProviderError> {
        // Convert messages to OpenAI format
        let openai_messages: Vec<Value> = messages
            .iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();

        // Build request body
        let mut body = json!({
            "model": config.default_model,
            "messages": openai_messages,
        });

        // Add tools if provided
        if let Some(tools) = tools {
            if !tools.is_empty() {
                let openai_tools: Vec<Value> = tools
                    .iter()
                    .filter_map(|tool| {
                        // Parse parameters JSON string
                        let params: Value = serde_json::from_str(&tool.parameters).ok()?;
                        Some(json!({
                            "type": "function",
                            "function": {
                                "name": tool.name,
                                "description": tool.description,
                                "parameters": params
                            }
                        }))
                    })
                    .collect();

                if !openai_tools.is_empty() {
                    body["tools"] = json!(openai_tools);
                }
            }
        }

        serde_json::to_string(&body).map_err(|e| ProviderError {
            message: format!("Failed to serialize request: {}", e),
            code: Some("SERIALIZATION_ERROR".to_string()),
        })
    }

    /// Parse response from OpenAI API
    fn parse_response(body: String, _model: String) -> Result<WitAssistantMessage, ProviderError> {
        let response: OpenAIResponse = serde_json::from_str(&body).map_err(|e| ProviderError {
            message: format!("Failed to parse response: {}", e),
            code: Some("PARSE_ERROR".to_string()),
        })?;

        if response.choices.is_empty() {
            return Err(ProviderError {
                message: "No choices in response".to_string(),
                code: Some("EMPTY_RESPONSE".to_string()),
            });
        }

        let message = &response.choices[0].message;

        // Extract content
        let content = message.content.clone();

        // Extract tool calls
        let tool_calls: Vec<WitToolCall> = message
            .tool_calls
            .as_ref()
            .map(|calls| {
                calls
                    .iter()
                    .map(|call| WitToolCall {
                        id: call.id.clone(),
                        name: call.function.name.clone(),
                        arguments: call.function.arguments.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(WitAssistantMessage {
            content,
            tool_calls,
        })
    }

    /// Handle streaming chunk (SSE format)
    fn handle_stream_chunk(chunk: String) -> Option<WitContentDelta> {
        let chunk = chunk.trim();

        // Check for SSE format
        if !chunk.starts_with("data: ") && !chunk.contains("\ndata: ") {
            return None;
        }

        // Extract data part
        let data = if let Some(stripped) = chunk.strip_prefix("data: ") {
            stripped
        } else if let Some(pos) = chunk.find("\ndata: ") {
            &chunk[pos + 7..]
        } else {
            return None;
        };

        // Check for done marker
        if data == "[DONE]" {
            return Some(WitContentDelta {
                delta_type: "done".to_string(),
                content: None,
                tool_call_index: None,
                tool_call: None,
                error: None,
            });
        }

        // Parse JSON
        let json: Value = serde_json::from_str(data).ok()?;

        // Extract from choices array
        let choices = json["choices"].as_array()?;
        if choices.is_empty() {
            return None;
        }

        let delta = &choices[0]["delta"];

        // Check for content delta
        if let Some(content) = delta["content"].as_str() {
            return Some(WitContentDelta {
                delta_type: "content".to_string(),
                content: Some(content.to_string()),
                tool_call_index: None,
                tool_call: None,
                error: None,
            });
        }

        // Check for tool call delta
        if let Some(tool_calls) = delta["tool_calls"].as_array() {
            if !tool_calls.is_empty() {
                let tc = &tool_calls[0];
                // Extract the index from the tool call (OpenAI sends this)
                let index = tc["index"].as_u64().map(|i| i as u32);
                let id = tc["id"].as_str().map(|s| s.to_string());
                let name = tc["function"]["name"].as_str().map(|s| s.to_string());
                let arguments = tc["function"]["arguments"].as_str().map(|s| s.to_string());

                // Only return if we have meaningful data
                if id.is_some() || name.is_some() || arguments.is_some() {
                    return Some(WitContentDelta {
                        delta_type: "tool_call".to_string(),
                        content: None,
                        tool_call_index: index,
                        tool_call: Some(WitToolCall {
                            id: id.unwrap_or_default(),
                            name: name.unwrap_or_default(),
                            arguments: arguments.unwrap_or_default(),
                        }),
                        error: None,
                    });
                }
            }
        }

        None
    }

    /// Get API URL for OpenAI
    fn get_api_url(base_url: String, _model: String) -> String {
        let base = base_url.trim_end_matches('/');
        format!("{}/chat/completions", base)
    }

    /// Check if streaming is supported
    fn supports_streaming(_model: String) -> bool {
        // All OpenAI models support streaming
        true
    }

    /// Format request from JSON (handles complex messages with tool_call_id, etc.)
    fn format_request_from_json(
        messages_json: String,
        model: String,
        tools_json: Option<String>,
        tool_choice_json: Option<String>,
        max_tokens: Option<u32>,
        temperature: f32,
        enable_streaming: bool,
    ) -> Result<String, ProviderError> {
        // Parse messages from JSON
        let messages: Vec<Value> =
            serde_json::from_str(&messages_json).map_err(|e| ProviderError {
                message: format!("Failed to parse messages JSON: {}", e),
                code: Some("JSON_PARSE_ERROR".to_string()),
            })?;

        // Convert InternalMessage format to OpenAI format
        let openai_messages: Vec<Value> = messages
            .into_iter()
            .map(|msg| {
                let role = msg["role"].as_str().unwrap_or("user");

                // Handle different message types
                match role {
                    "tool" => {
                        // Tool result message
                        let tool_call_id = msg["tool_call_id"].as_str().unwrap_or("");
                        let content = match &msg["content"] {
                            Value::String(s) => s.clone(),
                            Value::Object(_) | Value::Array(_) => {
                                serde_json::to_string(&msg["content"]).unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                        json!({
                            "role": "tool",
                            "tool_call_id": tool_call_id,
                            "content": content
                        })
                    }
                    "assistant" => {
                        // Assistant message - may have tool_calls OpenAI requires content to be null when tool_calls present
                        let mut assistant_msg = json!({
                            "role": "assistant",
                            "content": null,
                        });

                        // Add content if present
                        if let Some(content) = msg["content"].as_str() {
                            if !content.is_empty() {
                                assistant_msg["content"] = json!(content);
                            }
                        }

                        // Add tool_calls if present (from metadata or direct)
                        if let Some(tool_calls) = msg["metadata"]["tool_calls"].as_array() {
                            if !tool_calls.is_empty() {
                                assistant_msg["tool_calls"] = json!(tool_calls);
                            }
                        }

                        assistant_msg
                    }
                    _ => {
                        // Regular user/system message
                        let content = match &msg["content"] {
                            Value::String(s) => s.clone(),
                            Value::Object(_) | Value::Array(_) => {
                                serde_json::to_string(&msg["content"]).unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                        json!({
                            "role": role,
                            "content": content
                        })
                    }
                }
            })
            .collect();

        // Build request body
        let mut body = json!({
            "model": model,
            "messages": openai_messages,
            "temperature": temperature,
        });

        // Add max_tokens if provided
        if let Some(tokens) = max_tokens {
            body["max_tokens"] = json!(tokens);
        }

        // Add streaming if enabled
        if enable_streaming {
            body["stream"] = json!(true);
        }

        // Add tools if provided
        if let Some(tools_str) = tools_json {
            if let Ok(tools) = serde_json::from_str::<Vec<Value>>(&tools_str) {
                if !tools.is_empty() {
                    // Convert to OpenAI format
                    let openai_tools: Vec<Value> = tools
                        .into_iter()
                        .map(|tool| {
                            json!({
                                "type": "function",
                                "function": {
                                    "name": tool["name"],
                                    "description": tool["description"],
                                    "parameters": tool["parameters"]
                                }
                            })
                        })
                        .collect();
                    body["tools"] = json!(openai_tools);
                }
            }
        }

        // Add tool_choice if provided
        if let Some(choice_str) = tool_choice_json {
            if let Ok(choice) = serde_json::from_str::<Value>(&choice_str) {
                // Convert ToolChoice enum to OpenAI format
                if choice.is_string() {
                    let choice_str = choice.as_str().unwrap_or("auto");
                    match choice_str {
                        "Auto" | "auto" => body["tool_choice"] = json!("auto"),
                        "Required" | "required" => body["tool_choice"] = json!("required"),
                        "None" | "none" => body["tool_choice"] = json!("none"),
                        _ => {}
                    }
                } else if let Some(name) = choice["Specific"].as_str() {
                    body["tool_choice"] = json!({
                        "type": "function",
                        "function": { "name": name }
                    });
                } else if choice.get("type").is_some() {
                    // Already in OpenAI format
                    body["tool_choice"] = choice;
                }
            }
        }

        serde_json::to_string(&body).map_err(|e| ProviderError {
            message: format!("Failed to serialize request: {}", e),
            code: Some("SERIALIZATION_ERROR".to_string()),
        })
    }
}

// ===== OpenAI Response Types =====

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    #[allow(dead_code)]
    id: Option<String>,
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    call_type: String,
    function: FunctionCall,
}

#[derive(Debug, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_basic_request() {
        let messages = vec![WitMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }];

        let config = WitConfig {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            default_model: "gpt-4o".to_string(),
        };

        let result = OpenAIProvider::format_request(messages, config, None).unwrap();
        let parsed: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(parsed["model"], "gpt-4o");
        assert_eq!(parsed["messages"][0]["role"], "user");
        assert_eq!(parsed["messages"][0]["content"], "Hello");
    }

    #[test]
    fn test_parse_text_response() {
        let response = json!({
            "id": "chatcmpl-123",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help?"
                },
                "finish_reason": "stop"
            }]
        });

        let result =
            OpenAIProvider::parse_response(response.to_string(), "gpt-4o".to_string()).unwrap();
        assert_eq!(result.content, Some("Hello! How can I help?".to_string()));
        assert!(result.tool_calls.is_empty());
    }

    #[test]
    fn test_parse_tool_call_response() {
        let response = json!({
            "id": "chatcmpl-123",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"NYC\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let result =
            OpenAIProvider::parse_response(response.to_string(), "gpt-4o".to_string()).unwrap();
        assert_eq!(result.content, None);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "get_weather");
    }

    #[test]
    fn test_handle_stream_content() {
        let chunk = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let delta = OpenAIProvider::handle_stream_chunk(chunk.to_string());
        assert!(delta.is_some());
        let delta = delta.unwrap();
        assert_eq!(delta.delta_type, "content");
        assert_eq!(delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_handle_stream_done() {
        let chunk = "data: [DONE]";
        let delta = OpenAIProvider::handle_stream_chunk(chunk.to_string());
        assert!(delta.is_some());
        assert_eq!(delta.unwrap().delta_type, "done");
    }

    #[test]
    fn test_get_api_url() {
        let url = OpenAIProvider::get_api_url(
            "https://api.openai.com/v1".to_string(),
            "gpt-4o".to_string(),
        );
        assert_eq!(url, "https://api.openai.com/v1/chat/completions");
    }
}
