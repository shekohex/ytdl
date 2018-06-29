#[macro_use]
extern crate log;
extern crate env_logger;
extern crate failure;
extern crate ytdl_lib;
use failure::{err_msg, Error};
use std::env;
use ytdl_lib::Video;
type Result<T> = std::result::Result<T, Error>;

fn main() -> Result<()> {
    env::set_var("RUST_LOG", "ytdl");
    env_logger::init();
    let args: Vec<String> = env::args().skip(1).collect();
    let video_id = args
        .get(0)
        .ok_or_else(|| err_msg("Expected Video Id as an Argument?"))?;
    let mut v = Video::new(&video_id);
    info!("Getting Video Information for video ID #{}", v.id());
    v.initialize()?;
    let soruces = v
        .video_sources()
        .ok_or_else(|| err_msg("Error While Getting Video Messages"))?;
    println!("{:#?}", soruces);
    Ok(())
}
