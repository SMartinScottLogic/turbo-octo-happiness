#[macro_use]
extern crate log;
extern crate chrono;
extern crate env_logger;

use chrono::Local;
use env_logger::{Builder, Env};
use std::ffi::OsStr;
use std::io::prelude::*;
use clap::Parser;

mod organise_fs;
mod libc_wrapper;
use crate::organise_fs::OrganiseFS;

#[derive(Debug, Clone, Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Config {
    /// Path to source files from.
    pub root: String,
    /// Path to mount filesystem at.
    pub mountpoint: String,
    /// Only find duplicates immediately within supplied directories.
    #[clap(short, long)]
    pub non_recursive: bool,
    /// Should empty files be considered duplicates.
    #[clap(short = '0', long)]
    pub include_empty: bool,
    /// Show sizes of files within duplicate groups.
    #[clap(short='S', long)]
    pub show_sizes: bool,
    /// prompt user for files to preserve and delete all others.
    #[clap(short='p', long)]
    pub prompt: bool,
    /// purge files into trash, rather than permanently.
    #[clap(short='t', long)]
    pub trash: bool,
}

fn main() {
    let config = Config::parse();

    let env = Env::default().filter_or("RUST_LOG", "info");
    Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    info!("Starting organiseFS...");

    let organise_fs = OrganiseFS::new(&config.root);

    let fuse_args: Vec<&OsStr> = vec![OsStr::new("-o"), OsStr::new("auto_unmount")];
    fuse_mt::mount(fuse_mt::FuseMT::new(organise_fs, 1), &config.mountpoint, &fuse_args).unwrap();
}
