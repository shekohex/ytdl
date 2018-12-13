use serde_derive::{Serialize, Deserialize};
#[serde(default)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Assets {
    pub css: String,
    pub js: String,
}

#[serde(default)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct VideoConfig {
    pub assets: Assets,
    pub html5: bool,
    pub sts: i64,
    pub url: String,
}
