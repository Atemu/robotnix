use std::str;
use std::fs;
use clap::{Parser, Subcommand};

mod base;
mod lineage;
mod repo_manifest;
mod repo_lockfile;

use crate::base::{
    Repository,
    RepoProject
};
use crate::lineage::{
    read_device_metadata,
    fetch_device_metadata,
};
use crate::repo_manifest::{
    fetch_git_repo_metadata,
};
use crate::repo_lockfile::{
    incrementally_fetch_projects,
};

#[derive(Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    FetchRepoMetadata {
        #[arg(name = "branch", short, long)]
        branches: Vec<String>,

        #[arg(name = "url", short, long)]
        url: String,

        repo_metadata_file: String,
    },
    FetchRepoDirs {
        #[arg(short, long)]
        branch: String,

        #[arg(long)]
        repo_metadata_file: String,

        repo_dirs_file: String,
    },
    FetchDeviceMetadata {
        device_metadata_file: String,

        #[arg(name = "branch", short, long)]
        branch_whitelist: Option<Vec<String>>,

        #[arg(name = "device", short, long)]
        device_whitelist: Option<Vec<String>>,
    },
    FetchDeviceDirs {
        #[arg(long)]
        device_metadata_file: String,

        #[arg(short, long)]
        branch: String,

        device_dirs_file: String,
    },
}

fn main() {
    let args = Args::parse();

    match args.command.expect("You need to specify a command.") {
        Command::FetchRepoMetadata { branches, url, repo_metadata_file } => {
            fetch_git_repo_metadata(
                &repo_metadata_file,
                &Repository {
                    url: url,
                },
                &branches
            ).unwrap();
        },
        Command::FetchDeviceMetadata { device_metadata_file, branch_whitelist, device_whitelist } => {
            fetch_device_metadata(&device_metadata_file, &branch_whitelist, &device_whitelist).unwrap();
        },
        Command::FetchDeviceDirs { device_metadata_file, branch, device_dirs_file } => {
            let devices = read_device_metadata(&device_metadata_file).unwrap();
            let mut device_dirs = vec![];
            let mut device_names: Vec<String> = devices.keys().map(|x| x.to_string()).collect();
            device_names.sort();
            for device_name in device_names {
                for device_dir in devices[&device_name].deps.iter() {
                    if !device_dirs.contains(device_dir) {
                        device_dirs.push(device_dir.clone());
                    }
                }
            }

            incrementally_fetch_projects(&device_dirs_file, &device_dirs, &branch).unwrap();
        },
        Command::FetchRepoDirs { branch, repo_metadata_file, repo_dirs_file } => {
            let repo_dirs_json = fs::read(&repo_metadata_file).unwrap();
            let repo_dirs: Vec<RepoProject> = serde_json::from_str(
                str::from_utf8(&repo_dirs_json).unwrap()
            ).unwrap();

            incrementally_fetch_projects(&repo_dirs_file, &repo_dirs, &branch).unwrap();
        },
    }
}
