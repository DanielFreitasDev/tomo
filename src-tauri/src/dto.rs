//! Wire DTOs shared by commands and events. Field names match the frontend
//! contract.ts exactly.

use serde::Serialize;
use tomo_core::fsops::CollectionTree;
use tomo_core::model::ResponseData;

#[derive(Serialize)]
pub struct CollectionTreeDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub nodes: serde_json::Value,
    pub invalid: serde_json::Value,
    pub environments: Vec<String>,
    pub selected_environment: Option<String>,
}

impl CollectionTreeDto {
    pub fn build(
        id: &str,
        tree: &CollectionTree,
        environments: Vec<String>,
        selected: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: tree.collection.meta.name.clone(),
            path: id.to_string(),
            nodes: serde_json::to_value(&tree.nodes).unwrap_or(serde_json::Value::Null),
            invalid: serde_json::to_value(&tree.invalid).unwrap_or(serde_json::Value::Null),
            environments,
            selected_environment: selected,
        }
    }
}

#[derive(Serialize)]
pub struct ReadRequestDto {
    pub request: serde_json::Value,
    pub hash: String,
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SaveResultDto {
    Saved {
        hash: String,
    },
    Conflict {
        current_text: String,
        current_hash: String,
    },
}

/// Response metadata sent to the UI — the body itself goes via ipc::Response.
#[derive(Serialize)]
pub struct ResponseMetaDto {
    pub status: u16,
    pub status_text: String,
    pub http_version: String,
    pub headers: Vec<(String, String)>,
    pub final_url: String,
    pub timing: serde_json::Value,
    pub body: BodyMetaDto,
    pub warnings: serde_json::Value,
    pub console: serde_json::Value,
    pub tests: serde_json::Value,
    pub asserts: serde_json::Value,
    pub script_error: Option<String>,
    pub cookies: serde_json::Value,
}

#[derive(Serialize)]
pub struct BodyMetaDto {
    pub total_size: u64,
    pub preview_size: u64,
    pub truncated: bool,
    pub has_spill: bool,
    pub can_download_full: bool,
    pub mime: Option<String>,
    pub charset: Option<String>,
    pub is_binary: bool,
}

impl ResponseMetaDto {
    pub fn from_data(data: &ResponseData, cookies: serde_json::Value) -> Self {
        Self {
            status: data.status,
            status_text: data.status_text.clone(),
            http_version: data.http_version.clone(),
            headers: data.headers.clone(),
            final_url: data.final_url.clone(),
            timing: serde_json::to_value(data.timing).unwrap_or(serde_json::Value::Null),
            body: BodyMetaDto {
                total_size: data.body.total_size,
                preview_size: data.body.bytes.len() as u64,
                truncated: data.body.truncated,
                has_spill: data.body.spill_path.is_some(),
                can_download_full: true,
                mime: data.body.mime.clone(),
                charset: data.body.charset.clone(),
                is_binary: data.body.is_binary,
            },
            warnings: serde_json::to_value(&data.warnings).unwrap_or(serde_json::Value::Null),
            console: serde_json::to_value(&data.console).unwrap_or(serde_json::Value::Null),
            tests: serde_json::to_value(&data.tests).unwrap_or(serde_json::Value::Null),
            asserts: serde_json::to_value(&data.asserts).unwrap_or(serde_json::Value::Null),
            script_error: data.script_error.clone(),
            cookies,
        }
    }
}
