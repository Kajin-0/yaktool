use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    List,
    Search,
    FindLarge,
    CountFiles,
    CountDirectories,
    Move,
    Clarify,
    Unsupported,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Location {
    DirectoryAlias(String),
    PathBytes(Vec<u8>),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "relation",
    content = "days",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Age {
    OlderThan(u32),
    NewerThan(u32),
    Today,
    ThisWeek,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    pub extensions: Vec<String>,
    pub min_size_bytes: Option<u64>,
    pub age: Option<Age>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub schema_version: String,
    pub action: Action,
    pub source: Option<Location>,
    pub filters: Filters,
    pub destination: Option<Location>,
    pub message: Option<String>,
}
impl Intent {
    pub fn new(action: Action) -> Self {
        Self {
            schema_version: "yaktool.intent.v1".into(),
            action,
            source: None,
            filters: Filters::default(),
            destination: None,
            message: None,
        }
    }
}
