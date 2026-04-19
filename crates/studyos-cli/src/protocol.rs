use std::path::Path;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest<T>
where
    T: Serialize,
{
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: &'static str,
    pub params: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification<T>
where
    T: Serialize,
{
    pub jsonrpc: &'static str,
    pub method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<T>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitializeParams {
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<InitializeCapabilities>,
}

impl InitializeParams {
    pub fn studyos_default() -> Self {
        Self {
            client_info: ClientInfo {
                name: "studyos",
                version: env!("CARGO_PKG_VERSION"),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: Some(true),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub name: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitializeCapabilities {
    #[serde(rename = "experimentalApi", skip_serializing_if = "Option::is_none")]
    pub experimental_api: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadStartParams {
    pub cwd: String,
    #[serde(rename = "approvalPolicy")]
    pub approval_policy: &'static str,
    pub sandbox: &'static str,
    #[serde(
        rename = "developerInstructions",
        skip_serializing_if = "Option::is_none"
    )]
    pub developer_instructions: Option<String>,
}

impl ThreadStartParams {
    pub fn for_start(cwd: &Path, developer_instructions: &str) -> Self {
        Self {
            cwd: cwd.display().to_string(),
            approval_policy: "never",
            sandbox: "workspace-write",
            developer_instructions: Some(developer_instructions.to_string()),
        }
    }

    pub fn for_resume(thread_id: &str, cwd: &Path) -> ThreadResumeParams {
        ThreadResumeParams {
            thread_id: thread_id.to_string(),
            cwd: cwd.display().to_string(),
            approval_policy: "never",
            sandbox: "workspace-write",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadResumeParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub cwd: String,
    #[serde(rename = "approvalPolicy")]
    pub approval_policy: &'static str,
    pub sandbox: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct TurnStartParams {
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub cwd: String,
    #[serde(rename = "sandboxPolicy")]
    pub sandbox_policy: WorkspaceWriteSandboxPolicy,
    pub input: Vec<TextUserInput>,
    #[serde(rename = "outputSchema")]
    pub output_schema: Value,
}

impl TurnStartParams {
    pub fn structured_prompt(
        thread_id: &str,
        prompt: &str,
        output_schema: Value,
        cwd: &Path,
    ) -> Self {
        Self {
            thread_id: thread_id.to_string(),
            cwd: cwd.display().to_string(),
            sandbox_policy: WorkspaceWriteSandboxPolicy {
                type_name: "workspaceWrite",
                network_access: true,
                exclude_tmpdir_env_var: None,
                writable_roots: None,
            },
            input: vec![TextUserInput {
                type_name: "text",
                text: prompt.to_string(),
                text_elements: None,
            }],
            output_schema,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceWriteSandboxPolicy {
    #[serde(rename = "type")]
    pub type_name: &'static str,
    #[serde(rename = "networkAccess")]
    pub network_access: bool,
    #[serde(
        rename = "excludeTmpdirEnvVar",
        skip_serializing_if = "Option::is_none"
    )]
    pub exclude_tmpdir_env_var: Option<bool>,
    #[serde(rename = "writableRoots", skip_serializing_if = "Option::is_none")]
    pub writable_roots: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextUserInput {
    #[serde(rename = "type")]
    pub type_name: &'static str,
    pub text: String,
    #[serde(rename = "text_elements", skip_serializing_if = "Option::is_none")]
    pub text_elements: Option<Vec<TextElement>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextElement {
    #[serde(rename = "byteRange")]
    pub byte_range: ByteRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

pub fn jsonrpc_request<T>(id: u64, method: &'static str, params: T) -> JsonRpcRequest<T>
where
    T: Serialize,
{
    JsonRpcRequest {
        jsonrpc: "2.0",
        id,
        method,
        params,
    }
}

pub fn jsonrpc_notification<T>(method: &'static str, params: Option<T>) -> JsonRpcNotification<T>
where
    T: Serialize,
{
    JsonRpcNotification {
        jsonrpc: "2.0",
        method,
        params,
    }
}

#[cfg(test)]
mod tests {
    use super::{InitializeParams, ThreadStartParams, TurnStartParams};
    use serde_json::{Value, json};

    const INITIALIZE_SCHEMA: &str =
        include_str!("../schemas/codex-app-server/v1/InitializeParams.json");
    const THREAD_START_SCHEMA: &str =
        include_str!("../schemas/codex-app-server/v2/ThreadStartParams.json");
    const TURN_START_SCHEMA: &str =
        include_str!("../schemas/codex-app-server/v2/TurnStartParams.json");

    #[test]
    fn initialize_params_match_vendored_schema() {
        let value =
            serde_json::to_value(InitializeParams::studyos_default()).expect("serialize params");
        let schema = parse_schema(INITIALIZE_SCHEMA);
        assert_matches_schema_shape(&value, &schema, "InitializeParams");
    }

    #[test]
    fn thread_start_params_match_vendored_schema() {
        let params = ThreadStartParams::for_start(Path::new("/tmp/studyos"), "teach strictly");
        let value = serde_json::to_value(params).expect("serialize params");
        let schema = parse_schema(THREAD_START_SCHEMA);
        assert_matches_schema_shape(&value, &schema, "ThreadStartParams");
    }

    #[test]
    fn turn_start_params_match_vendored_schema() {
        let params = TurnStartParams::structured_prompt(
            "thread_123",
            "Ask a matrix question",
            json!({"type":"object"}),
            Path::new("/tmp/studyos"),
        );
        let value = serde_json::to_value(params).expect("serialize params");
        let schema = parse_schema(TURN_START_SCHEMA);
        assert_matches_schema_shape(&value, &schema, "TurnStartParams");

        let sandbox_policy = value.get("sandboxPolicy").expect("missing sandbox policy");
        let sandbox_schema = schema
            .get("definitions")
            .and_then(|definitions| definitions.get("SandboxPolicy"))
            .expect("missing SandboxPolicy definition");
        assert_one_of_branch_matches(
            sandbox_policy,
            sandbox_schema,
            "TurnStartParams.sandboxPolicy",
        );

        let input = value
            .get("input")
            .and_then(Value::as_array)
            .expect("missing input array");
        let first_item = input.first().expect("missing text input");
        let user_input_schema = schema
            .get("definitions")
            .and_then(|definitions| definitions.get("UserInput"))
            .expect("missing UserInput definition");
        assert_one_of_branch_matches(first_item, user_input_schema, "TurnStartParams.input[0]");
    }

    fn parse_schema(raw: &str) -> Value {
        serde_json::from_str(raw).expect("schema should parse")
    }

    fn assert_matches_schema_shape(value: &Value, schema: &Value, label: &str) {
        let Some(object) = value.as_object() else {
            panic!("{label} should serialize to an object");
        };

        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for key in required {
            let key = key.as_str().expect("required key should be string");
            assert!(
                object.contains_key(key),
                "{label} is missing required key `{key}`"
            );
        }

        let properties = schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("schema should define object properties");
        for key in object.keys() {
            assert!(
                properties.contains_key(key),
                "{label} serialized unexpected key `{key}` not present in vendored schema"
            );
        }
    }

    fn assert_one_of_branch_matches(value: &Value, schema: &Value, label: &str) {
        let branches = schema
            .get("oneOf")
            .and_then(Value::as_array)
            .expect("schema should expose oneOf branches");
        let mut matched = false;

        for branch in branches {
            if let Some(properties) = branch.get("properties").and_then(Value::as_object) {
                if value
                    .as_object()
                    .map(|object| {
                        object
                            .keys()
                            .all(|key| properties.contains_key(key) || branch_allows_ref(branch))
                    })
                    .unwrap_or(false)
                {
                    let required = branch
                        .get("required")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let has_required = required.iter().all(|required_key| {
                        value
                            .as_object()
                            .expect("validated as object")
                            .contains_key(required_key.as_str().expect("required key string"))
                    });
                    if has_required {
                        matched = true;
                        break;
                    }
                }
            }
        }

        assert!(matched, "{label} did not match any vendored schema branch");
    }

    fn branch_allows_ref(branch: &Value) -> bool {
        branch.get("$ref").is_some()
    }

    use std::path::Path;
}
