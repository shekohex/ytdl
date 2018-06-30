#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
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
use std::fmt;
use std::io::Read;
use std::iter::FromIterator;
use std::sync::Mutex;
use url::form_urlencoded::parse;
use video_model::VideoConfig;
type Result<T> = std::result::Result<T, Error>;
type VideoInfo = HashMap<String, String>;
type VideoSorces = Vec<VideoInfo>;
type TokensContainer = HashMap<String, Vec<(String, usize)>>;

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

lazy_static! {
    // act like a Singleton Container for pre computed tokens.
    static ref TOKENSCONTAINER: Mutex<TokensContainer> = Mutex::new(TokensContainer::new());
    static ref JS_QUOTE_STR: String = format!("(?:{}|{})", JS_SINGLE_QUOTE, JS_DOUBLE_QUOTE);
    static ref JS_PROP_STR: String = format!("(?:\\.{}|\\[{}\\])", JS_VAR_STR, JS_QUOTE_STR);
    static ref ACTIONS_FUNC_REGEXP: String = format!(
        "function(?: {})?\\(a\\)\\{}a=a\\.split\\\
         ({}\\);\\s*((?:(?:a=)?{}{}\\(a,\\d+\\);)+)\
         return a\\.join\\({}\\)\\{}",
        JS_VAR_STR, "{", JS_EMPTY_STR, JS_VAR_STR, JS_PROP_STR, JS_EMPTY_STR, "}"
    );
    static ref JS_KEY_STR: String = format!("(?:{}|{})", JS_VAR_STR, JS_QUOTE_STR);
    static ref ACTIONS_OBJ_REGEXP: String = format!(
        "var ({})=\\{}((?:(?:{}{}|{}{}|{}{}|{}{}),?\\r?\\n?)+)\\{};",
        JS_VAR_STR,
        "{",
        JS_KEY_STR,
        JS_REVRSE_STR,
        JS_KEY_STR,
        JS_SLICE_STR,
        JS_KEY_STR,
        JS_SPLICE_STR,
        JS_KEY_STR,
        JS_SWAP_STR,
        "}"
    );
    static ref REVERSE_REGEX_STR: String = format!("(?m:^|,)({}){}", JS_KEY_STR, JS_REVRSE_STR);
    static ref SLICE_REGEX_STR: String = format!("(?m:^|,)({}){}", JS_KEY_STR, JS_SLICE_STR);
    static ref SPLICE_REGEX_STR: String = format!("(?m:^|,)({}){}", JS_KEY_STR, JS_SPLICE_STR);
    static ref SWAP_REGEX_STR: String = format!("(?m:^|,)({}){}", JS_KEY_STR, JS_SWAP_STR);

    // we don't have to re compile the regax every request.
    static ref ACTION_REGEX: Regex = Regex::new(&ACTIONS_OBJ_REGEXP).unwrap();
    static ref FUNC_REGEX: Regex = Regex::new(&ACTIONS_FUNC_REGEXP).unwrap();
    static ref REVERSE_REGEX: Regex = Regex::new(&REVERSE_REGEX_STR).unwrap();
    static ref SLICE_REGEX: Regex = Regex::new(&SLICE_REGEX_STR).unwrap();
    static ref SPLICE_REGEX: Regex = Regex::new(&SPLICE_REGEX_STR).unwrap();
    static ref SWAP_REGEX: Regex = Regex::new(&SWAP_REGEX_STR).unwrap();
}

impl fmt::Display for JS_QUOTE_STR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl fmt::Display for JS_PROP_STR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl fmt::Display for JS_KEY_STR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
    #[inline]
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
        let tokens = get_tokens(&self.config.assets.js)?;
        for mut source in &mut self.sources {
            let mut signature;
            {
                let s = source.entry("s".to_string()).or_insert_with(String::new);
                if s == &"".to_string() {
                    continue;
                }
                signature = decipher(&tokens, s)?;
            }
            source.insert("signature".to_string(), signature);
        }
        self.initialized = true;
        info!("Video initialized successfully");
        Ok(())
    }
    #[inline]
    fn get_video_info(&mut self) -> Result<()> {
        let url: String = format!("{}?video_id={}", YOUTUBE_INFO_URL, self.id);
        let mut res: Response = reqwest::get(&url)?;
        let mut data = Vec::new();
        res.read_to_end(&mut data)?;
        self.info = parse(&data).into_owned().collect();
        let status = self
            .info
            .get("status")
            .ok_or_else(|| err_msg("Cannot get Status"))?;
        debug!("Video Status {}", status);
        if status == &"fail".to_string() {
            Err(format_err!(
                "Video Status {}, Maybe a bad ID ? or video not found!",
                status
            ))?
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

// Extract signature deciphering tokens from html5player file.
#[inline]
fn get_tokens<'b>(html5_player_url: &'b str) -> Result<Vec<(String, usize)>> {
    let re = Regex::new(r"player[-_]([a-zA-Z0-9\-_]+)")?;
    let player_id = re
        .captures(html5_player_url)
        .ok_or_else(|| err_msg("Error while getting Video Player ID from URL"))?
        .get(1)
        .ok_or_else(|| err_msg("There is No Player id"))?
        .as_str();
    let mut container = TOKENSCONTAINER.lock().unwrap();
    debug!("Player Id {:?}", player_id);
    {
        if let Some(cached_tokens) = container.get(player_id) {
            debug!("Found Cached Tokens for player {}", player_id);
            return Ok(cached_tokens.to_vec());
        }
    }
    // get the file and Calculate the tokens
    let player_url = YOUTUBE_BASE_URL.to_string() + html5_player_url;
    let mut res: Response = reqwest::get(&player_url)?;
    let mut file = String::new();
    res.read_to_string(&mut file)?;
    let tokens = exteract_actions(&file)?;
    debug!("Tokens {:?}", tokens);
    container.insert(player_id.to_string(), tokens.clone());
    Ok(tokens)
}

// Decipher a signature based on action tokens.
#[inline]
fn decipher<'c>(tokens: &[(String, usize)], signature: &'c str) -> Result<String> {
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
 #[inline]
fn exteract_actions(html5_player_file: &str) -> Result<Vec<(String, usize)>> {
    let obj_result = actions_obj_regex(html5_player_file)?;
    debug!("obj_result => {:?}", obj_result);
    let func_result = actions_func_regex(html5_player_file)?;
    let obj = obj_result.get(1).map_or("", |v| v.as_str());
    let obj_body = obj_result.get(2).map_or("", |v| v.as_str());
    let func_body = func_result.get(1).map_or("", |v| v.as_str());
    let reverse_key = multi_regex(&REVERSE_REGEX, obj_body)?;
    let slice_key = multi_regex(&SLICE_REGEX, obj_body)?;
    let splice_key = multi_regex(&SPLICE_REGEX, obj_body)?;
    let swap_key = multi_regex(&SWAP_REGEX, obj_body)?;
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
    debug!("reverse_key => {:?}", reverse_key);
    debug!("slice_key => {:?}", slice_key);
    debug!("splice_key => {:?}", splice_key);
    debug!("swap_key => {:?}", swap_key);
    debug!("keys => {}", keys);
    debug!("myreg => {}", myreg);
    Ok(tokens)
}
#[inline]
fn actions_obj_regex<'t>(text: &'t str) -> Result<Captures<'t>> {
    let captures = ACTION_REGEX
        .captures(text)
        .ok_or_else(|| err_msg("Error While Matching 'obj' in Html5player file"))?;
    Ok(captures)
}
#[inline]
fn actions_func_regex<'t>(text: &'t str) -> Result<Captures<'t>> {
    let captures = FUNC_REGEX
        .captures(text)
        .ok_or_else(|| err_msg("Error While Matching 'func' in Html5player file"))?;
    Ok(captures)
}
#[inline]
fn multi_regex<'t>(regex: &'static Regex, text: &'t str) -> Result<&'t str> {
    match regex.captures(text) {
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
