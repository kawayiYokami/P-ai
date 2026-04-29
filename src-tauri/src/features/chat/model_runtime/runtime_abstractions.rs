#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProviderToolDefinition {
    name: String,
    description: String,
    parameters: Value,
}

impl ProviderToolDefinition {
    fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

#[allow(dead_code)]
trait RuntimeToolMetadata {
    fn provider_tool_definition(&self) -> ProviderToolDefinition;
}

type RuntimeToolCallFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<ProviderToolResult, String>> + Send + 'a>,
>;
type RuntimeJsonValueFuture<'a, E> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Value, E>> + Send + 'a>,
>;

trait RuntimeToolDyn: Send + Sync {
    fn name(&self) -> String;
    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_>;
}

trait RuntimeJsonTool: RuntimeToolMetadata + Send + Sync {
    const NAME: &'static str;
    type Args: for<'de> Deserialize<'de> + Send;
    type Error: std::fmt::Display + Send;

    fn call_typed(&self, args: Self::Args) -> RuntimeJsonValueFuture<'_, Self::Error>;
}

fn provider_tool_result_from_json_value(value: Value) -> Result<ProviderToolResult, String> {
    let text = serde_json::to_string(&value)
        .map_err(|err| format!("Serialize tool output failed: {err}"))?;
    Ok(ProviderToolResult {
        display_text: text.clone(),
        parts: vec![ProviderToolResultPart::Text { text }],
        is_error: false,
    })
}

fn parse_runtime_tool_args<T>(args_json: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    if args_json.trim().is_empty() {
        serde_json::from_str::<T>("{}")
    } else {
        serde_json::from_str::<T>(args_json)
    }
    .map_err(|err| format!("Parse tool args failed: {err}"))
}

impl<T> RuntimeToolDyn for T
where
    T: RuntimeJsonTool,
{
    fn name(&self) -> String {
        T::NAME.to_string()
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        Box::pin(async move {
            let args = parse_runtime_tool_args::<T::Args>(&args_json)?;
            let output_value = self.call_typed(args).await.map_err(|err| err.to_string())?;
            provider_tool_result_from_json_value(output_value)
        })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProviderToolCallRequest {
    invocation_id: String,
    provider_call_id: Option<String>,
    tool_name: String,
    arguments: Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum ProviderToolResultPart {
    Text {
        text: String,
    },
    Image {
        mime: String,
        data_base64: String,
    },
    Resource {
        mime: Option<String>,
        uri: Option<String>,
        text: String,
    },
    Audio {
        mime: String,
        data_base64: String,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ProviderToolResult {
    display_text: String,
    parts: Vec<ProviderToolResultPart>,
    is_error: bool,
}

#[allow(dead_code)]
impl ProviderToolResult {
    fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            display_text: text.clone(),
            parts: vec![ProviderToolResultPart::Text { text }],
            is_error: false,
        }
    }

    fn error(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            display_text: text.clone(),
            parts: vec![ProviderToolResultPart::Text { text }],
            is_error: true,
        }
    }
}
