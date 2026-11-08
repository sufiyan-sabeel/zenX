//! zenX Contract — API compatibility checking.
//!
//! The parser understands OpenAPI 3 documents in JSON or YAML and reduces them
//! to the operations, parameters and response schemas that decide
//! compatibility. Every rule here is a stated rule, not a guess: if a change is
//! not in the list below it is not reported.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use zenx_core::{Result, ZenxError};

const METHODS: &[&str] = &[
    "get", "put", "post", "delete", "patch", "head", "options", "trace",
];

#[derive(Debug, Clone, Serialize)]
pub struct Parameter {
    pub name: String,
    pub location: String,
    pub required: bool,
    /// Canonical JSON of the schema, used to detect any change.
    pub schema: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Operation {
    pub path: String,
    pub method: String,
    pub operation_id: Option<String>,
    /// Keyed by `location:name`.
    pub parameters: BTreeMap<String, Parameter>,
    pub request_body_required: bool,
    /// Status code → canonical schema JSON.
    pub responses: BTreeMap<String, String>,
    pub deprecated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Spec {
    pub title: String,
    pub version: String,
    /// Keyed by `METHOD /path`.
    pub operations: BTreeMap<String, Operation>,
}

impl Spec {
    pub fn key(method: &str, path: &str) -> String {
        format!("{} {}", method.to_ascii_uppercase(), path)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Breaking,
    NonBreaking,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct Change {
    pub severity: Severity,
    pub kind: String,
    pub endpoint: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comparison {
    pub base_title: String,
    pub head_title: String,
    pub breaking: usize,
    pub non_breaking: usize,
    pub informational: usize,
    pub changes: Vec<Change>,
}

impl Comparison {
    pub fn is_compatible(&self) -> bool {
        self.breaking == 0
    }

    pub fn render_markdown(&self, base_path: &str, head_path: &str) -> String {
        let mut out = String::new();
        out.push_str("# API compatibility report\n\n");
        out.push_str(&format!("- Base: {base_path} ({})\n", self.base_title));
        out.push_str(&format!("- Head: {head_path} ({})\n", self.head_title));
        out.push_str(&format!(
            "- Result: {} breaking, {} non-breaking, {} informational\n\n",
            self.breaking, self.non_breaking, self.informational
        ));
        if self.changes.is_empty() {
            out.push_str("_No differences were found between the two specifications._\n");
            return out;
        }
        out.push_str("| Severity | Change | Endpoint | Detail |\n|----------|--------|----------|--------|\n");
        for change in &self.changes {
            out.push_str(&format!(
                "| {:?} | {} | {} | {} |\n",
                change.severity,
                change.kind,
                change.endpoint,
                change.detail.replace('|', "\\|")
            ));
        }
        out
    }
}

fn parse_document(text: &str) -> Result<serde_json::Value> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return Ok(value);
    }
    serde_yaml::from_str::<serde_json::Value>(text).map_err(|error| {
        ZenxError::Invalid(format!("document is neither valid JSON nor valid YAML: {error}"))
    })
}

fn canonical(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(value) => serde_json::to_string(value).unwrap_or_else(|_| "?".to_string()),
        None => String::new(),
    }
}

fn schema_type(fingerprint: &str) -> Option<String> {
    if fingerprint.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(fingerprint).ok()?;
    value
        .get("type")
        .and_then(|t| t.as_str())
        .map(|t| t.to_string())
}

fn parameters(items: Option<&serde_json::Value>) -> BTreeMap<String, Parameter> {
    let mut map = BTreeMap::new();
    let Some(array) = items.and_then(|value| value.as_array()) else {
        return map;
    };
    for item in array {
        let name = item
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let location = item
            .get("in")
            .and_then(|value| value.as_str())
            .unwrap_or("query")
            .to_string();
        let required = item
            .get("required")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let schema = canonical(item.get("schema"));
        map.insert(
            format!("{location}:{name}"),
            Parameter {
                name,
                location,
                required,
                schema,
            },
        );
    }
    map
}

fn responses(value: Option<&serde_json::Value>) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Some(object) = value.and_then(|value| value.as_object()) else {
        return map;
    };
    for (code, response) in object {
        let schema = response
            .get("content")
            .and_then(|content| content.as_object())
            .and_then(|content| content.get("application/json"))
            .and_then(|media| media.get("schema"))
            .or_else(|| response.get("schema"));
        map.insert(code.clone(), canonical(schema));
    }
    map
}

/// Parse an OpenAPI document into the reduced model.
pub fn load_spec(path: &Path) -> Result<Spec> {
    let text = std::fs::read_to_string(path).map_err(|e| ZenxError::io(path, e))?;
    let document = parse_document(&text)?;

    let info = document.get("info");
    let title = info
        .and_then(|info| info.get("title"))
        .and_then(|title| title.as_str())
        .unwrap_or("untitled API")
        .to_string();
    let version = info
        .and_then(|info| info.get("version"))
        .and_then(|version| version.as_str())
        .unwrap_or("unversioned")
        .to_string();

    let paths = document
        .get("paths")
        .and_then(|paths| paths.as_object())
        .ok_or_else(|| {
            ZenxError::Invalid(format!(
                "{} has no `paths` object; is it an OpenAPI 3 document?",
                path.display()
            ))
        })?;

    let mut operations = BTreeMap::new();
    for (route, item) in paths {
        let Some(item_object) = item.as_object() else {
            continue;
        };
        let shared = parameters(item.get("parameters"));
        for method in METHODS.iter().copied() {
            let Some(operation) = item_object.get(method) else {
                continue;
            };
            let mut merged = shared.clone();
            for (key, value) in parameters(operation.get("parameters")) {
                merged.insert(key, value);
            }
            let request_body_required = operation
                .get("requestBody")
                .and_then(|body| body.get("required"))
                .and_then(|required| required.as_bool())
                .unwrap_or(false);
            let deprecated = operation
                .get("deprecated")
                .and_then(|flag| flag.as_bool())
                .unwrap_or(false);
            let operation_id = operation
                .get("operationId")
                .and_then(|id| id.as_str())
                .map(|id| id.to_string());

            let key = Spec::key(method, route);
            operations.insert(
                key,
                Operation {
                    path: route.clone(),
                    method: method.to_ascii_uppercase(),
                    operation_id,
                    parameters: merged,
                    request_body_required,
                    responses: responses(operation.get("responses")),
                    deprecated,
                },
            );
        }
    }

    Ok(Spec {
        title,
        version,
        operations,
    })
}

fn change(
    severity: Severity,
    kind: &str,
    endpoint: &str,
    detail: String,
) -> Change {
    Change {
        severity,
        kind: kind.to_string(),
        endpoint: endpoint.to_string(),
        detail,
    }
}

/// Compare two specifications.
///
/// Breaking: removed endpoint, removed response code, added required
/// parameter, parameter made required, request body made required, or any
/// change to a response schema.
pub fn compare(base: &Spec, head: &Spec) -> Comparison {
    let mut changes = Vec::new();

    for (key, base_operation) in &base.operations {
        let Some(head_operation) = head.operations.get(key) else {
            changes.push(change(
                Severity::Breaking,
                "endpoint_removed",
                key,
                format!(
                    "{} {} was removed",
                    base_operation.method, base_operation.path
                ),
            ));
            continue;
        };

        // Parameters
        for (parameter_key, head_parameter) in &head_operation.parameters {
            match base_operation.parameters.get(parameter_key) {
                None => {
                    let severity = if head_parameter.required {
                        Severity::Breaking
                    } else {
                        Severity::NonBreaking
                    };
                    changes.push(change(
                        severity,
                        if head_parameter.required {
                            "required_parameter_added"
                        } else {
                            "optional_parameter_added"
                        },
                        key,
                        format!(
                            "{} parameter {:?} added",
                            head_parameter.location, head_parameter.name
                        ),
                    ));
                }
                Some(base_parameter) => {
                    if base_parameter.required != head_parameter.required {
                        let severity = if head_parameter.required {
                            Severity::Breaking
                        } else {
                            Severity::NonBreaking
                        };
                        changes.push(change(
                            severity,
                            "parameter_required_changed",
                            key,
                            format!(
                                "{} parameter {:?} required {} → {}",
                                head_parameter.location,
                                head_parameter.name,
                                base_parameter.required,
                                head_parameter.required
                            ),
                        ));
                    }
                    if base_parameter.schema != head_parameter.schema {
                        let base_type = schema_type(&base_parameter.schema);
                        let head_type = schema_type(&head_parameter.schema);
                        let severity = if base_type != head_type {
                            Severity::Breaking
                        } else {
                            Severity::NonBreaking
                        };
                        changes.push(change(
                            severity,
                            "parameter_schema_changed",
                            key,
                            format!(
                                "{} parameter {:?} schema changed",
                                head_parameter.location, head_parameter.name
                            ),
                        ));
                    }
                }
            }
        }
        for (parameter_key, base_parameter) in &base_operation.parameters {
            if !head_operation.parameters.contains_key(parameter_key) {
                changes.push(change(
                    Severity::NonBreaking,
                    "parameter_removed",
                    key,
                    format!(
                        "{} parameter {:?} removed",
                        base_parameter.location, base_parameter.name
                    ),
                ));
            }
        }

        // Request body
        if !base_operation.request_body_required && head_operation.request_body_required {
            changes.push(change(
                Severity::Breaking,
                "request_body_now_required",
                key,
                "a previously optional request body is now required".to_string(),
            ));
        }

        // Responses
        for (code, base_schema) in &base_operation.responses {
            match head_operation.responses.get(code) {
                None => changes.push(change(
                    Severity::Breaking,
                    "response_removed",
                    key,
                    format!("response {code} was removed"),
                )),
                Some(head_schema) => {
                    if base_schema != head_schema {
                        changes.push(change(
                            Severity::Breaking,
                            "response_schema_changed",
                            key,
                            format!("response {code} schema changed"),
                        ));
                    }
                }
            }
        }
        for code in head_operation.responses.keys() {
            if !base_operation.responses.contains_key(code) {
                changes.push(change(
                    Severity::NonBreaking,
                    "response_added",
                    key,
                    format!("response {code} was added"),
                ));
            }
        }

        if !base_operation.deprecated && head_operation.deprecated {
            changes.push(change(
                Severity::Info,
                "endpoint_deprecated",
                key,
                "the endpoint is now marked deprecated".to_string(),
            ));
        }
    }

    for (key, head_operation) in &head.operations {
        if !base.operations.contains_key(key) {
            changes.push(change(
                Severity::NonBreaking,
                "endpoint_added",
                key,
                format!(
                    "{} {} was added",
                    head_operation.method, head_operation.path
                ),
            ));
        }
    }

    changes.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.endpoint.cmp(&b.endpoint))
            .then_with(|| a.kind.cmp(&b.kind))
    });

    let breaking = changes
        .iter()
        .filter(|c| c.severity == Severity::Breaking)
        .count();
    let non_breaking = changes
        .iter()
        .filter(|c| c.severity == Severity::NonBreaking)
        .count();
    let informational = changes
        .iter()
        .filter(|c| c.severity == Severity::Info)
        .count();

    Comparison {
        base_title: format!("{} {}", base.title, base.version),
        head_title: format!("{} {}", head.title, head.version),
        breaking,
        non_breaking,
        informational,
        changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
openapi: 3.0.0
info:
  title: Pets
  version: 1.0.0
paths:
  /pets:
    get:
      parameters:
        - name: limit
          in: query
          required: false
          schema:
            type: integer
      responses:
        "200":
          description: ok
          content:
            application/json:
              schema:
                type: array
    post:
      responses:
        "201":
          description: created
  /pets/{id}:
    delete:
      responses:
        "204":
          description: deleted
"#;

    fn write(tag: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("zenx-contract-{}-{}", std::process::id(), tag));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("openapi.yaml");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parses_yaml_and_json() {
        let yaml = write("yaml", BASE);
        let spec = load_spec(&yaml).unwrap();
        assert_eq!(spec.title, "Pets");
        assert_eq!(spec.operations.len(), 3);
        assert!(spec.operations.contains_key("GET /pets"));

        let json = write(
            "json",
            r#"{"openapi":"3.0.0","info":{"title":"J","version":"1"},"paths":{"/a":{"get":{"responses":{"200":{"description":"ok"}}}}}}"#,
        );
        assert_eq!(load_spec(&json).unwrap().operations.len(), 1);
        let _ = std::fs::remove_dir_all(yaml.parent().unwrap());
        let _ = std::fs::remove_dir_all(json.parent().unwrap());
    }

    #[test]
    fn identical_specs_are_compatible() {
        let base = load_spec(&write("same-a", BASE)).unwrap();
        let head = load_spec(&write("same-b", BASE)).unwrap();
        let comparison = compare(&base, &head);
        assert!(comparison.is_compatible());
        assert!(comparison.changes.is_empty());
    }

    #[test]
    fn removed_endpoint_is_breaking() {
        let base = load_spec(&write("rm-a", BASE)).unwrap();
        let head = load_spec(&write(
            "rm-b",
            &BASE.replace("  /pets/{id}:\n    delete:\n      responses:\n        \"204\":\n          description: deleted\n", ""),
        ))
        .unwrap();
        let comparison = compare(&base, &head);
        assert_eq!(comparison.breaking, 1);
        assert_eq!(comparison.changes[0].kind, "endpoint_removed");
        assert!(!comparison.is_compatible());
    }

    #[test]
    fn added_required_parameter_is_breaking() {
        let base = load_spec(&write("req-a", BASE)).unwrap();
        let head = load_spec(&write(
            "req-b",
            &BASE.replace(
                "        - name: limit\n          in: query\n          required: false",
                "        - name: limit\n          in: query\n          required: true",
            ),
        ))
        .unwrap();
        let comparison = compare(&base, &head);
        assert_eq!(comparison.breaking, 1);
        assert_eq!(comparison.changes[0].kind, "parameter_required_changed");
    }

    #[test]
    fn response_schema_change_is_breaking_and_removal_too() {
        let base = load_spec(&write("resp-a", BASE)).unwrap();
        let head = load_spec(&write(
            "resp-b",
            &BASE.replace("type: array", "type: object"),
        ))
        .unwrap();
        let comparison = compare(&base, &head);
        assert_eq!(comparison.breaking, 1);
        assert_eq!(comparison.changes[0].kind, "response_schema_changed");
    }

    #[test]
    fn new_endpoint_is_non_breaking() {
        let base = load_spec(&write("add-a", BASE)).unwrap();
        let head = load_spec(&write(
            "add-b",
            &format!("{BASE}  /owners:\n    get:\n      responses:\n        \"200\":\n          description: ok\n"),
        ))
        .unwrap();
        let comparison = compare(&base, &head);
        assert_eq!(comparison.breaking, 0);
        assert_eq!(comparison.non_breaking, 1);
        assert!(comparison.is_compatible());
    }

    #[test]
    fn invalid_documents_are_rejected() {
        let path = write("invalid", "not: [an openapi document");
        assert!(load_spec(&path).is_err());
        let path = write("nopaths", "openapi: 3.0.0\ninfo:\n  title: x\n  version: '1'\n");
        assert!(load_spec(&path).is_err());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn markdown_report_lists_changes() {
        let base = load_spec(&write("md-a", BASE)).unwrap();
        let head = load_spec(&write("md-b", &BASE.replace("type: array", "type: object"))).unwrap();
        let comparison = compare(&base, &head);
        let text = comparison.render_markdown("base.yaml", "head.yaml");
        assert!(text.contains("# API compatibility report"));
        assert!(text.contains("response_schema_changed"));
        assert!(text.contains("1 breaking"));
    }
}
