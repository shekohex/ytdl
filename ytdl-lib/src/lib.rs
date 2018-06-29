#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate url;
pub mod video_model;
use failure::{err_msg, Error};
use regex::{Captures, Regex};
use reqwest::Response;
use std::collections::HashMap;
use std::io::Read;
use std::iter::FromIterator;
use url::form_urlencoded::parse;
use video_model::VideoConfig;
type Result<T> = std::result::Result<T, Error>;
type VideoInfo = HashMap<String, String>;
type VideoSorces = Vec<VideoInfo>;
const YOUTUBE_INFO_URL: &str = "https://www.youtube.com/get_video_info";
const YOUTUBE_BASE_URL: &str = "https://www.youtube.com";
const JS_VAR_STR: &str = "[a-zA-Z_\\$][a-zA-Z_0-9]*";
const JS_SINGLE_QUOTE: &str = "'[^'\\\\]*(:?\\\\[\\s\\S][^'\\\\]*)*'";
const JS_DOUBLE_QUOTE: &str = r#""[^"\\]*(:?\\[\s\S][^"\\]*)*""#;
const JS_EMPTY_STR: &str = r#"(?:''|"")"#;
const JS_REVRSE_STR: &str = ":function\\(a\\)\\{(?:return )?a\\.reverse\\(\\)\\}";
const JS_SLICE_STR: &str = ":function\\(a,b\\)\\{return a\\.slice\\(b\\)\\}";
const JS_SPLICE_STR: &str = ":function\\(a,b\\)\\{a\\.splice\\(0,b\\)\\}";
const JS_SWAP_STR: &str = ":function\\(a,b\\)\\{var c=a\\[0\\];a\\[0\\]=a\\\
                           [b(?:%a\\.length)?\\];a\\[b(?:%a\\.length)?\\]=c(?:;return a)?\\}";

#[derive(Debug, Default)]
pub struct Video {
    id: String,
    info: VideoInfo,
    config: VideoConfig,
    initialized: bool,
    sources: VideoSorces,
}

impl Video {
    pub fn new(id: &str) -> Self {
        Video {
            id: id.to_string(),
            info: VideoInfo::new(),
            config: VideoConfig::default(),
            sources: VideoSorces::new(),
            initialized: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn video_sources(&self) -> Option<&VideoSorces> {
        if self.initialized {
            Some(&self.sources)
        } else {
            error!("Video not initialized !");
            None
        }
    }

    pub fn video_config(&self) -> Option<&VideoConfig> {
        if self.initialized {
            Some(&self.config)
        } else {
            error!("Video not initialized !");
            None
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            warn!("Video is already initialized !");
            return Ok(());
        }
        // We need to get it
        self.get_video_info()?;
        let url: String = format!("{}/watch?v={}", YOUTUBE_BASE_URL, self.id);
        let mut res: Response = reqwest::get(&url)?;
        let mut page = String::new();
        res.read_to_string(&mut page)?;
        let json_str = between(&page, "ytplayer.config = ", "</script>")
            .ok_or_else(|| err_msg("Json String Not found"))?;
        let config_pos = json_str
            .rfind(";ytplayer.load")
            .ok_or_else(|| err_msg("Config Postion Not found"))?;
        let config_str = json_str
            .get(0..config_pos)
            .ok_or_else(|| err_msg("Config String Not found"))?;
        self.config = serde_json::from_str(config_str)?;
        let mut html5player = Html5PlayerDecipher::default();
        let tokens = html5player.get_tokens(&self.config.assets.js)?;
        for mut source in self.sources.iter_mut() {
            let mut signature;
            {
                let s = source.entry("s".to_string()).or_insert(String::new());
                signature = html5player.decipher(&tokens, s)?;
            }
            source.insert("signature".to_string(), signature);
        }
        self.initialized = true;
        info!("Video initialized successfully");
        Ok(())
    }

    fn get_video_info(&mut self) -> Result<()> {
        let url: String = format!("{}?video_id={}", YOUTUBE_INFO_URL, self.id);
        let mut res: Response = reqwest::get(&url)?;
        let mut data = Vec::new();
        res.read_to_end(&mut data)?;
        let parsed = parse(&data);
        for (k, v) in parsed {
            self.info.insert(k.to_string(), v.to_string());
        }
        let status = self
            .info
            .get("status")
            .ok_or_else(|| err_msg("Cannot get Status"))?;
        debug!("Video Status {}", status);
        if status == &"fail".to_string() {
            Err(format_err!("Video Status {}", status))?
        }
        let sources = self
            .info
            .get("url_encoded_fmt_stream_map")
            .ok_or_else(|| err_msg("url_encoded_fmt_stream_map not found"))?;
        let sources = sources.split(',');
        for (i, source) in sources.enumerate() {
            let parsed = parse(&source.as_bytes());
            let mut element = HashMap::new();
            for (k, v) in parsed {
                element.insert(k.to_string(), v.to_string());
            }
            self.sources.insert(i, element);
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct Html5PlayerDecipher {
    cache: HashMap<String, Vec<(String, usize)>>,
}

impl Html5PlayerDecipher {
    // Extract signature deciphering tokens from html5player file.
    pub fn get_tokens<'b>(&'b mut self, html5_player_url: &'b str) -> Result<Vec<(String, usize)>> {
        let re = Regex::new(r"player[-_]([a-zA-Z0-9\-_]+)")?;
        let player_id = re
            .captures(html5_player_url)
            .ok_or_else(|| err_msg("Error while getting Video Player ID from URL"))?
            .get(1)
            .ok_or_else(|| err_msg("There is No Player id"))?
            .as_str();
        debug!("Player Id {:?}", player_id);
        {
            if let Some(cached_tokens) = self.cache.get(player_id) {
                debug!(
                    "Found Cached Tokens {:#?} for player {}",
                    cached_tokens, player_id
                );
                return Ok(cached_tokens.to_vec());
            }
        }
        // get the file and Calculate the tokens
        let player_url = YOUTUBE_BASE_URL.to_string() + html5_player_url;
        let mut res: Response = reqwest::get(&player_url)?;
        let mut file = String::new();
        res.read_to_string(&mut file)?;
        let tokens = self.exteract_actions(&file)?;
        debug!("Tokens {:#?}", tokens);
        self.cache.insert(player_id.to_string(), tokens.clone());
        Ok(tokens)
    }

    // Decipher a signature based on action tokens.
    pub fn decipher<'c>(&self, tokens: &[(String, usize)], signature: &'c str) -> Result<String> {
        let mut sig: Vec<char> = signature.chars().collect();
        for (op, n) in tokens.iter() {
            let n = *n;
            match op.as_str() {
                "reverse" => sig.reverse(),
                "splice" => {
                    sig.drain(0..n);
                }
                "slice" => sig = sig.get(n..).unwrap_or_default().to_vec(),
                "swap" => sig.swap(0, n),
                _ => warn!("Unknow Op ({}, {})", op, n),
            }
        }
        let seg = String::from_iter(sig);
        Ok(seg)
    }

    /**
     * Extracts the actions that should be taken to decipher a signature.
     *
     * This searches for a function that performs string manipulations on
     * the signature. We already know what the 3 possible changes to a signature
     * are in order to decipher it. There is
     *
     * - Reversing the string.
     * - Removing a number of characters from the beginning.
     * - Swapping the first character with another position.
     *
     * After retrieving the function that does this, we can see what actions
     * it takes on a signature.
     */
    fn exteract_actions(&self, html5_player_file: &str) -> Result<Vec<(String, usize)>> {
        debug!("File Size {}", html5_player_file.len());
        let obj_result = actions_obj_regex(html5_player_file)?;
        debug!("obj_result => {:?}", obj_result);
        let func_result = actions_func_regex(html5_player_file)?;
        let obj = obj_result.get(1).map_or("", |v| v.as_str());
        let obj_body = obj_result.get(2).map_or("", |v| v.as_str());
        let func_body = func_result.get(1).map_or("", |v| v.as_str());
        let reverse_key = multi_regex(JS_REVRSE_STR, obj_body)?;
        let slice_key = multi_regex(JS_SLICE_STR, obj_body)?;
        let splice_key = multi_regex(JS_SPLICE_STR, obj_body)?;
        let swap_key = multi_regex(JS_SWAP_STR, obj_body)?;
        let keys = format!(
            "({})",
            [reverse_key, slice_key, splice_key, swap_key].join("|")
        );
        let myreg = format!(
            r#"(?:a=)?{}(?:\.{keys}|\['{keys}'\]|\["{keys}"\])\(a,(\d+)\)"#,
            obj,
            keys = keys
        );
        let re = Regex::new(&myreg)?;
        let mut tokens = Vec::new();
        let build_tokens = |key: &str, q: &str| -> (String, usize) {
            let n: usize = q.parse().unwrap_or(0);
            debug!("Key {} => Value {}", key, q);
            match key {
                _ if swap_key == key => ("swap".to_string(), n),
                _ if reverse_key == key => ("reverse".to_string(), n),
                _ if slice_key == key => ("slice".to_string(), n),
                _ if splice_key == key => ("splice".to_string(), n),
                _ => ("".to_string(), 0),
            }
        };
        for m in re.captures_iter(func_body) {
            match (m.get(1), m.get(2), m.get(3), m.get(4)) {
                (Some(k1), _, _, Some(v)) => tokens.push(build_tokens(k1.as_str(), v.as_str())),
                (_, Some(k2), _, Some(v)) => tokens.push(build_tokens(k2.as_str(), v.as_str())),
                (_, _, Some(k3), Some(v)) => tokens.push(build_tokens(k3.as_str(), v.as_str())),
                (_, _, _, _) => Err(err_msg("Unknown Pattern !"))?,
            }
        }
        debug!("obj => {}", obj);
        debug!("obj_body => {}", obj_body);
        debug!("func_body => {}", func_body);
        debug!("reverse_key => {:#?}", reverse_key);
        debug!("slice_key => {:#?}", slice_key);
        debug!("splice_key => {:#?}", splice_key);
        debug!("swap_key => {:#?}", swap_key);
        debug!("keys => {}", keys);
        debug!("myreg => {}", myreg);
        Ok(tokens)
    }
}

fn actions_obj_regex<'t>(text: &'t str) -> Result<Captures<'t>> {
    let js_quote_str = format!("(?:{}|{})", JS_SINGLE_QUOTE, JS_DOUBLE_QUOTE);
    let js_key_str = format!("(?:{}|{})", JS_VAR_STR, js_quote_str);
    let actions_obj_regexp = format!(
        "var ({})=\\{}((?:(?:{}{}|{}{}|{}{}|{}{}),?\\r?\\n?)+)\\{};",
        JS_VAR_STR,
        "{",
        js_key_str,
        JS_REVRSE_STR,
        js_key_str,
        JS_SLICE_STR,
        js_key_str,
        JS_SPLICE_STR,
        js_key_str,
        JS_SWAP_STR,
        "}"
    );
    let captures = Regex::new(&actions_obj_regexp)?
        .captures(text)
        .ok_or_else(|| err_msg("Error While Matching 'obj' in Html5player file"))?;
    Ok(captures)
}

fn actions_func_regex<'t>(text: &'t str) -> Result<Captures<'t>> {
    let js_quote_str = format!("(?:{}|{})", JS_SINGLE_QUOTE, JS_DOUBLE_QUOTE);
    let js_prop_str = format!("(?:\\.{}|\\[{}\\])", JS_VAR_STR, js_quote_str);
    let actions_func_regexp = format!(
        "function(?: {})?\\(a\\)\\{}a=a\\.split\\\
         ({}\\);\\s*((?:(?:a=)?{}{}\\(a,\\d+\\);)+)\
         return a\\.join\\({}\\)\\{}",
        JS_VAR_STR, "{", JS_EMPTY_STR, JS_VAR_STR, js_prop_str, JS_EMPTY_STR, "}"
    );
    let captures = Regex::new(&actions_func_regexp)?
        .captures(text)
        .ok_or_else(|| err_msg("Error While Matching 'func' in Html5player file"))?;
    Ok(captures)
}

fn multi_regex<'t>(jsfn: &'static str, text: &'t str) -> Result<&'t str> {
    let js_quote_str = format!("(?:{}|{})", JS_SINGLE_QUOTE, JS_DOUBLE_QUOTE);
    let js_key_str = format!("(?:{}|{})", JS_VAR_STR, js_quote_str);
    let re_str = &format!("(?m:^|,)({}){}", js_key_str, jsfn);
    debug!("Multi Regex Text {}", text);
    debug!("Multi Regex {}", re_str);
    let re = Regex::new(re_str)?;
    match re.captures(text) {
        Some(captures) => Ok(captures.get(1).map_or(" ", |v| v.as_str())),
        None => Ok(" "),
    }
}

// Get the value between left and righ from haystack
fn between<'a>(haystack: &'a str, left: &str, right: &str) -> Option<&'a str> {
    let from = haystack.find(left)? + left.len();
    let to = haystack.rfind(right)?;
    haystack.get(from..to)
}
