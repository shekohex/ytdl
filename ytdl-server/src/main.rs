extern crate env_logger;
extern crate failure;
extern crate futures;
extern crate hyper;
extern crate serde_json;
extern crate url;
extern crate ytdl_lib;
use failure::{err_msg, Error};
use futures::{future, Future};
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::collections::HashMap;
use url::form_urlencoded;
use ytdl_lib::Video;
type Result<T> = std::result::Result<T, Error>;
type BoxFuture = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

fn router(req: Request<Body>) -> BoxFuture {
    let response;
    let internal_server_error = |error: Error| {
        let err = format!("Error: {}", error.cause());
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(err.into_bytes()))
            .unwrap()
    };
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/watch") => {
            response = get_video(&req).unwrap_or_else(internal_server_error);
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

fn get_video(req: &Request<Body>) -> Result<Response<Body>> {
    let query = req
        .uri()
        .query()
        .ok_or_else(|| err_msg("Expected to have query string in url"))?;
    let hash_query: HashMap<String, String> = form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let video_id = hash_query
        .get("v")
        .ok_or_else(|| err_msg("Expected url to have query '?v=video_id'"))?;
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

fn main() -> Result<()> {
    env_logger::init();
    let addr = ([0, 0, 0, 0], 3000).into();
    println!("Starting Server..");
    let server = Server::bind(&addr)
        .serve(|| service_fn(router))
        .map_err(|e| eprintln!("Server error: {}", e));

    println!("Listening on http://{}", addr);
    hyper::rt::run(server);
    Ok(())
}
