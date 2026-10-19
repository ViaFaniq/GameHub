use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Game {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub installed: bool,
    pub artwork: Option<String>,
    pub path: Option<String>,
    pub playtime: i64,
    pub platform: Option<String>,
    pub command: Option<String>,
    pub runner_type: Option<String>,
    pub proton_version: Option<String>,
    pub use_global_args: Option<bool>,
    pub arguments: Option<String>,
    pub dll_overrides: Option<String>,
    pub steam_id: Option<String>,
    pub artwork_source: Option<String>,
}

#[derive(Serialize, Default)]
pub struct LibraryPayload {
    pub games: Vec<Game>,
    pub running: Vec<String>,
    pub protons: HashMap<String, String>,
    pub settings: HashMap<String, String>,
    pub errors: Vec<String>,
}
