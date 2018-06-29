#[serde(default)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Args {
    pub account_playback_token: String,
    pub ad3_module: String,
    pub ad_device: String,
    pub ad_logging_flag: String,
    pub ad_preroll: String,
    pub adaptive_fmts: String,
    pub allow_below_the_player_companion: bool,
    pub allow_embed: String,
    pub allow_html5_ads: String,
    pub allow_ratings: String,
    pub apiary_host: String,
    pub apiary_host_firstparty: String,
    pub atc: String,
    pub author: String,
    pub avg_rating: String,
    pub c: String,
    pub cl: String,
    pub core_dbp: String,
    pub cr: String,
    pub csi_page_type: String,
    pub cver: String,
    pub enablecsi: String,
    pub enabled_engage_types: String,
    pub enablejsapi: String,
    pub eventid: String,
    pub external_play_video: String,
    pub fade_in_duration_milliseconds: String,
    pub fade_in_start_milliseconds: String,
    pub fade_out_duration_milliseconds: String,
    pub fade_out_start_milliseconds: String,
    pub fexp: String,
    pub fflags: String,
    pub fmt_list: String,
    pub gapi_hint_params: String,
    pub hl: String,
    pub host_language: String,
    pub idpj: String,
    pub innertube_api_key: String,
    pub innertube_api_version: String,
    pub innertube_context_client_version: String,
    pub is_listed: String,
    pub ismb: String,
    pub itct: String,
    pub iv3_module: String,
    pub iv_invideo_url: String,
    pub iv_load_policy: String,
    pub keywords: String,
    pub ldpj: String,
    pub length_seconds: String,
    #[serde(rename = "loaderUrl")]
    pub loader_url: String,
    pub loudness: String,
    pub midroll_freqcap: String,
    pub midroll_prefetch_size: String,
    pub mpu: bool,
    pub no_get_video_log: String,
    pub of: String,
    pub oid: String,
    pub player_error_log_fraction: String,
    pub player_response: String,
    pub plid: String,
    pub pltype: String,
    pub probe_url: String,
    pub ptk: String,
    pub pyv_ad_channel: String,
    pub relative_loudness: String,
    pub serialized_ad_ux_config: String,
    pub sffb: bool,
    pub show_content_thumbnail: bool,
    pub show_pyv_in_related: bool,
    pub ssl: String,
    pub storyboard_spec: String,
    pub t: String,
    pub tag_for_child_directed: bool,
    pub thumbnail_url: String,
    pub timestamp: String,
    pub title: String,
    pub tmi: String,
    pub token: String,
    pub ucid: String,
    pub url_encoded_fmt_stream_map: String,
    pub video_id: String,
    pub videostats_playback_base_url: String,
    pub view_count: String,
    pub vm: String,
    pub vmap: String,
    pub vss_host: String,
    pub watch_xlb: String,
    pub watermark: String,
    pub xhr_apiary_host: String,
}

#[serde(default)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Assets {
    pub css: String,
    pub js: String,
}

#[serde(default)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Attrs {
    pub id: String,
}

#[serde(default)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct Params {
    pub allowfullscreen: String,
    pub allowscriptaccess: String,
    pub bgcolor: String,
}

#[serde(default)]
#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct VideoConfig {
    pub args: Args,
    pub assets: Assets,
    pub attrs: Attrs,
    pub html5: bool,
    pub params: Params,
    pub sts: i64,
    pub url: String,
}
