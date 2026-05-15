use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(
    default,
    rename_all(serialize = "camelCase", deserialize = "snake_case")
)]
pub struct NexusModInfo {
    pub mod_id: u64,
    pub name: String,
    pub summary: String,
    pub description: Option<String>,
    pub picture_url: Option<String>,
    pub mod_downloads: u64,
    pub mod_unique_downloads: u64,
    pub endorsement_count: u64,
    pub version: String,
    pub author: String,
    pub uploaded_by: String,
    pub category_id: u64,
    pub created_timestamp: u64,
    pub updated_timestamp: u64,
    pub available: bool,
    pub status: String,
}
