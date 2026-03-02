use anyhow::{Context, Result};
use futures::{AsyncBufReadExt, AsyncReadExt, StreamExt, io::BufReader, stream::BoxStream};
use http_client::{AsyncBody, HttpClient, HttpRequest, HttpRequestExt, Method};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use settings::KeepAlive;

pub const OLLAMA_API_URL: &str = "http://localhost:11434";

#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub name: String,
    pub display_name: Option<String>,
    pub max_tokens: u64,
    pub keep_alive: Option<KeepAlive>,
    pub supports_tools: Option<bool>,
    pub supports_vision: Option<bool>,
    pub supports_thinking: Option<bool>,
}

fn get_max_tokens(_name: &str) -> u64 {
    const DEFAULT_TOKENS: u64 = 4096;
    DEFAULT_TOKENS
}

impl Model {
    pub fn new(
        name: &str,
        display_name: Option<&str>,
        max_tokens: Option<u64>,
        supports_tools: Option<bool>,
        supports_vision: Option<bool>,
        supports_thinking: Option<bool>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            display_name: display_name
                .map(ToString::to_string)
                .or_else(|| name.strip_suffix(":latest").map(ToString::to_string)),
            max_tokens: max_tokens.unwrap_or_else(|| get_max_tokens(name)),
            keep_alive: Some(KeepAlive::indefinite()),
            supports_tools,
            supports_vision,
            supports_thinking,
        }
    }

    pub fn id(&self) -> &str {
        &self.name
    }

    pub fn display_name(&self) -> &str {
        self.display_name.as_ref().unwrap_or(&self.name)
    }

    pub fn max_token_count(&self) -> u64 {
        self.max_tokens
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMessage {
    Assistant {
        content: String,
        tool_calls: Option<Vec<OllamaToolCall>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<String>>,
        thinking: Option<String>,
    },
    User {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<String>>,
    },
    System {
        content: String,
    },
    Tool {
        tool_name: String,
        content: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaToolCall {
    pub id: String,
    pub function: OllamaFunctionCall,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaFunctionCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct OllamaFunctionTool {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OllamaTool {
    Function { function: OllamaFunctionTool },
}

#[derive(Serialize, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub keep_alive: KeepAlive,
    pub options: Option<ChatOptions>,
    pub tools: Vec<OllamaTool>,
    pub think: Option<bool>,
}

// https://github.com/ollama/ollama/blob/main/docs/modelfile.md#valid-parameters-and-values
#[derive(Serialize, Default, Debug)]
pub struct ChatOptions {
    pub num_ctx: Option<u64>,
    pub num_predict: Option<isize>,
    pub stop: Option<Vec<String>>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponseDelta {
    pub model: String,
    pub created_at: String,
    pub message: ChatMessage,
    pub done_reason: Option<String>,
    pub done: bool,
    pub prompt_eval_count: Option<u64>,
    pub eval_count: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct LocalModelsResponse {
    pub models: Vec<LocalModelListing>,
}

#[derive(Serialize, Deserialize)]
pub struct LocalModelListing {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
    pub digest: String,
    pub details: ModelDetails,
}

#[derive(Serialize, Deserialize)]
pub struct LocalModel {
    pub modelfile: String,
    pub parameters: String,
    pub template: String,
    pub details: ModelDetails,
}

#[derive(Serialize, Deserialize)]
pub struct ModelDetails {
    pub format: String,
    pub family: String,
    pub families: Option<Vec<String>>,
    pub parameter_size: String,
    pub quantization_level: String,
}

#[derive(Debug)]
pub struct ModelShow {
    pub capabilities: Vec<String>,
    pub context_length: Option<u64>,
    pub architecture: Option<String>,
}

impl<'de> Deserialize<'de> for ModelShow {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct ModelShowVisitor;

        impl<'de> Visitor<'de> for ModelShowVisitor {
            type Value = ModelShow;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a ModelShow object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut capabilities: Vec<String> = Vec::new();
                let mut architecture: Option<String> = None;
                let mut context_length: Option<u64> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "capabilities" => {
                            capabilities = map.next_value()?;
                        }
                        "model_info" => {
                            let model_info: Value = map.next_value()?;
                            if let Value::Object(obj) = model_info {
                                architecture = obj
                                    .get("general.architecture")
                                    .and_then(|v| v.as_str())
                                    .map(String::from);

                                if let Some(arch) = &architecture {
                                    context_length = obj
                                        .get(&format!("{}.context_length", arch))
                                        .and_then(|v| v.as_u64());
                                }
                            }
                        }
                        _ => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                Ok(ModelShow {
                    capabilities,
                    context_length,
                    architecture,
                })
            }
        }

        deserializer.deserialize_map(ModelShowVisitor)
    }
}

impl ModelShow {
    pub fn supports_tools(&self) -> bool {
        // .contains expects &String, which would require an additional allocation
        self.capabilities.iter().any(|v| v == "tools")
    }

    pub fn supports_vision(&self) -> bool {
        self.capabilities.iter().any(|v| v == "vision")
    }

    pub fn supports_thinking(&self) -> bool {
        self.capabilities.iter().any(|v| v == "thinking")
    }
}

pub async fn stream_chat_completion(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
    request: ChatRequest,
) -> Result<BoxStream<'static, Result<ChatResponseDelta>>> {
    let uri = format!("{api_url}/api/chat");
    let request = HttpRequest::builder()
        .method(Method::POST)
        .uri(uri)
        .header("Content-Type", "application/json")
        .when_some(api_key, |builder, api_key| {
            builder.header("Authorization", format!("Bearer {api_key}"))
        })
        .body(AsyncBody::from(serde_json::to_string(&request)?))?;

    let mut response = client.send(request).await?;
    if response.status().is_success() {
        let reader = BufReader::new(response.into_body());

        Ok(reader
            .lines()
            .map(|line| match line {
                Ok(line) => serde_json::from_str(&line).context("Unable to parse chat response"),
                Err(e) => Err(e.into()),
            })
            .boxed())
    } else {
        let mut body = String::new();
        response.body_mut().read_to_string(&mut body).await?;
        anyhow::bail!(
            "Failed to connect to Ollama API: {} {}",
            response.status(),
            body,
        );
    }
}

pub async fn get_models(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<LocalModelListing>> {
    let uri = format!("{api_url}/api/tags");
    let request = HttpRequest::builder()
        .method(Method::GET)
        .uri(uri)
        .header("Accept", "application/json")
        .when_some(api_key, |builder, api_key| {
            builder.header("Authorization", format!("Bearer {api_key}"))
        })
        .body(AsyncBody::default())?;

    let mut response = client.send(request).await?;

    let mut body = String::new();
    response.body_mut().read_to_string(&mut body).await?;

    anyhow::ensure!(
        response.status().is_success(),
        "Failed to connect to Ollama API: {} {}",
        response.status(),
        body,
    );
    let response: LocalModelsResponse =
        serde_json::from_str(&body).context("Unable to parse Ollama tag listing")?;
    Ok(response.models)
}

/// Fetch details of a model, used to determine model capabilities
pub async fn show_model(
    client: &dyn HttpClient,
    api_url: &str,
    api_key: Option<&str>,
    model: &str,
) -> Result<ModelShow> {
    let uri = format!("{api_url}/api/show");
    let request = HttpRequest::builder()
        .method(Method::POST)
        .uri(uri)
        .header("Content-Type", "application/json")
        .when_some(api_key, |builder, api_key| {
            builder.header("Authorization", format!("Bearer {api_key}"))
        })
        .body(AsyncBody::from(
            serde_json::json!({ "model": model }).to_string(),
        ))?;

    let mut response = client.send(request).await?;
    let mut body = String::new();
    response.body_mut().read_to_string(&mut body).await?;

    anyhow::ensure!(
        response.status().is_success(),
        "Failed to connect to Ollama API: {} {}",
        response.status(),
        body,
    );
    let details: ModelShow = serde_json::from_str(body.as_str())?;
    Ok(details)
}
