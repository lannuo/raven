use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};
use strum::EnumString;

#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq, JsonSchema, MergeFrom)]
#[serde(untagged)]
pub enum KeepAlive {
    /// Keep model alive for N seconds
    Seconds(isize),
    /// Keep model alive for a fixed duration. Accepts durations like "5m", "10m", "1h", "1d", etc.
    Duration(String),
}

impl KeepAlive {
    /// Keep model alive until a new model is loaded or until Ollama shuts down
    pub fn indefinite() -> Self {
        Self::Seconds(-1)
    }
}

impl Default for KeepAlive {
    fn default() -> Self {
        Self::indefinite()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, EnumString, JsonSchema, MergeFrom)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum OpenAiReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

#[with_fallible_options]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct OpenRouterProvider {
    order: Option<Vec<String>>,
    #[serde(default = "default_true")]
    allow_fallbacks: bool,
    #[serde(default)]
    require_parameters: bool,
    #[serde(default)]
    data_collection: DataCollection,
    only: Option<Vec<String>>,
    ignore: Option<Vec<String>>,
    quantizations: Option<Vec<String>>,
    sort: Option<String>,
}

#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize, JsonSchema, MergeFrom)]
#[serde(rename_all = "lowercase")]
pub enum DataCollection {
    #[default]
    Allow,
    Disallow,
}

fn default_true() -> bool {
    true
}

#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, MergeFrom,
)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ModelMode {
    #[default]
    Default,
    Thinking {
        /// The maximum number of tokens to use for reasoning. Must be lower than the model's `max_output_tokens`.
        budget_tokens: Option<u32>,
    },
}
