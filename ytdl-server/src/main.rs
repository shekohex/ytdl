#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_json;
extern crate env_logger;
extern crate failure;
extern crate futures;
extern crate hyper;
extern crate regex;
extern crate url;
use regex::Regex;
extern crate ytdl_lib;
use failure::{err_msg, Error};
use futures::{future, Future};
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::ops::Sub;
use std::process::Command;
use url::form_urlencoded;
use ytdl_lib::Video;
type Result<T> = std::result::Result<T, Error>;
type BoxFuture = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;
const TIME_REGEX_STR: &str = r"(?m)^(?:(?:([01]?\d|2[0-3]):)?([0-5]?\d):)?([0-5]?\d)$";
const GOOGLE_VIDEO_URL: &str = r#"^https?.+?\.googlevideo\.com/videoplayback"#;

lazy_static! {
    static ref TIME_REGEX: Regex = Regex::new(&TIME_REGEX_STR).unwrap();
    static ref GOOGLE_VIDEO_URL_REGEX: Regex = Regex::new(&GOOGLE_VIDEO_URL).unwrap();
    static ref HTTP_HELP: String = serde_json::to_string_pretty(&json!({
        "endpoints": [
            {
                "path": "/",
                "method": "GET",
                "required_params": "",
                "description": "View this endpoint."
            },
            {
                "path": "/watch",
                "method": "GET",
                "required_params": {
                    "v": "the video id"
                },
                "description": "get the video downloads urls"
            },
            {
                "path": "/extract",
                "method": "GET",
                "required_params": {
                    "url": "the exteracted video url from /watch endpoint",
                    "start": "the start time in HH:MM:SS format",
                    "end": "the end time in HH:MM:SS format"
                },
                "description": "extract gif from the video"
            }
        ]
    })).unwrap();
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct HumanTime(i16, i8, i8);

impl Sub for HumanTime {
    type Output = HumanTime;

    fn sub(self, other: HumanTime) -> HumanTime {
        let h = self.0 - other.0;
        let m = self.1 - other.1;
        let s = self.2 - other.2;
        HumanTime(h, m, s)
    }
}

impl fmt::Display for HumanTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}:{}", self.0, self.1, self.2)
    }
}

impl HumanTime {
    // parse time from format HH:MM:SS to use it in ffmpeg
    pub fn from(time: &str) -> Result<HumanTime> {
        let captures = TIME_REGEX.captures(time).ok_or_else(|| {
            err_msg(
                r"Wrong Start or End Time format.
        it should be on 'HH:MM:SS' format, can you check it again ?",
            )
        })?;
        let h: i16 = captures.get(1).map_or("0", |v| v.as_str()).parse()?;
        let m: i8 = captures.get(2).map_or("0", |v| v.as_str()).parse()?;
        let s: i8 = captures.get(3).map_or("0", |v| v.as_str()).parse()?;
        Ok(HumanTime(h, m, s))
    }

    pub fn calculate_duration(&self, since: HumanTime) -> Result<HumanTime> {
        let result = since - *self;
        if result.0 < 0 || result.1 < 0 || result.2 < 0 {
            Err(err_msg(
                "Doh :( , look at start and end time, and try again, Idiot :'D ",
            ))?
        }
        Ok(result)
    }
}

fn router(req: Request<Body>) -> BoxFuture {
    let response;
    let internal_server_error = |error: Error| {
        let err = format!(r#"<h2 style="color: red;"> Error </h2>: {}"#, error.cause());
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(err.into_bytes()))
            .unwrap()
    };
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            let body: &str = &HTTP_HELP.as_str();
            response = Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap();
        }

        (&Method::GET, "/watch") => {
            response = get_video(&req).unwrap_or_else(internal_server_error);
        }

        (&Method::GET, "/extract") => {
            response = extract_gif(&req).unwrap_or_else(internal_server_error);
        }
        _ => {
            response = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap();
        }
    }
    Box::new(future::ok(response))
}

fn validate_query<'a>(query: &'a HashMap<String, String>, key: &str) -> Result<&'a String> {
    let err = format!("Expected url to have query key '{}'", key);
    query.get(key).ok_or_else(|| err_msg(err))
}

fn get_video(req: &Request<Body>) -> Result<Response<Body>> {
    let query = req
        .uri()
        .query()
        .ok_or_else(|| err_msg("Expected to have query string in url"))?;
    let hash_query: HashMap<String, String> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let video_id = validate_query(&hash_query, "v")?;
    let mut video = Video::new(&video_id);
    video.initialize()?;
    let video_sources = video
        .video_sources()
        .ok_or_else(|| err_msg("Ops, Error While Getting Video Sources."))?;
    let body = serde_json::to_string_pretty(video_sources)?;
    let response = Response::builder()
        .header("Content-Type", "application/json")
        .status(StatusCode::OK)
        .body(Body::from(body))?;
    Ok(response)
}

fn extract_gif(req: &Request<Body>) -> Result<Response<Body>> {
    let query = req
        .uri()
        .query()
        .ok_or_else(|| err_msg("Expected to have query string in url"))?;
    let hash_query: HashMap<String, String> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let video_url = validate_query(&hash_query, "url")?;
    if !GOOGLE_VIDEO_URL_REGEX.is_match(video_url) {
        Err(err_msg(
            "Maybe not a youtube video url? it should be from googlevideo.com",
        ))?
    }
    let start_time = validate_query(&hash_query, "start")?;
    let end_time = validate_query(&hash_query, "end")?;
    let start = HumanTime::from(start_time.as_str())?;
    let end = HumanTime::from(end_time.as_str())?;
    let duration = start.calculate_duration(end)?;
    if duration.0 >= 1 || duration.1 >= 1 {
        Err(err_msg(
            "The difference between start and end, should be less than 1 min, sorry !",
        ))?
    }
    let body = make_gif(&video_url, &start, &duration)?;
    let response = Response::builder()
        .header("Content-Type", "image/gif")
        .header("Content-Disposition", r#"inline; filename="extracted.gif""#)
        .status(StatusCode::OK)
        .body(Body::from(body))?;
    Ok(response)
}

fn make_gif<'b>(url: &str, start: &HumanTime, duration: &HumanTime) -> Result<Vec<u8>> {
    let start_time: &str = &start.to_string();
    let duration_str: &str = &duration.to_string();
    let command = Command::new("ffmpeg")
        .args(&["-v", "error"])
        .args(&["-ss", start_time])
        .args(&["-t", duration_str])
        .args(&["-i", url])
        .args(&["-f", "gif"])
        .arg("-hide_banner")
        .args(&["-vf", "scale=340:-1"])
        .arg("pipe:1")
        .output()?;
    if command.status.success() {
        return Ok(command.stdout);
    } else {
        let err = String::from_utf8_lossy(&command.stderr);
        print!("Err: {}\n", err);
        Err(err_msg(
            r"Error While Making the gif, maybe a bad url ? or missing signture !
            and oh, please make sure that the url is encoded correctly",
        ))?
    }
    Ok(b"".to_vec())
}
/// Look up our server port number in PORT, for compatibility with Heroku.
fn get_server_port() -> u16 {
    env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080)
}

fn main() -> Result<()> {
    env_logger::init();
    let port = get_server_port();
    let addr = ([0, 0, 0, 0], port).into();
    println!("Starting Server..");
    let server = Server::bind(&addr)
        .serve(|| service_fn(router))
        .map_err(|e| eprintln!("Server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);
    Ok(())
}
