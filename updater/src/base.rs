use std::collections::HashMap;
use std::io;
use std::process::Command;
use serde::{Serialize, Deserialize};
use serde_json;
use git2;

#[derive(Debug, Serialize, Deserialize)]
pub enum Variant {
    #[serde(rename = "userdebug")]
    Userdebug,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FetchgitArgs {
    url: String,
    rev: String,
    date: i64, // Y292B problem here we come!
    path: String,
    hash: String,
    #[serde(rename = "fetchLFS")]
    fetch_lfs: bool,
    #[serde(rename = "fetchSubmodules")]
    fetch_submodules: bool,
    #[serde(rename = "deepClone")]
    deep_clone: bool,
    #[serde(rename = "leaveDotGit")]
    leave_dot_git: bool,
}

impl FetchgitArgs {
    // TODO In the future, we might consider only storing fields that actually are arguments
    // fetchgit and deriving the store path based on the hash. (for instance with tvix?)
    pub fn path(&self) -> String {
        self.path.clone()
    }
}

// Git repository
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Repository {
    pub url: String,
}

// Data structure for each branch where the repo comes from and the git rev that should be used
// Mirrors the repo.xml contents for one specific repository/"project"
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct RepoProjectBranchSettings {
    pub repo: Repository,
    pub git_ref: String,
    pub linkfiles: HashMap<String, String>, // dst -> src
    pub copyfiles: HashMap<String, String>, // dst -> src
    pub groups: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct RepoProject {
    pub path: String, // Where it should be in the source tree
    pub nonfree: bool,
    pub branch_settings: HashMap<String, RepoProjectBranchSettings>, // global_branch -> branch_info
}

#[derive(Debug)]
pub enum GetRevOfBranchError {
    Libgit(git2::Error),
    BranchNotFound(String),
}

fn is_commit_hash(git_ref: &str) -> bool {
    git_ref.len() == 40 && git_ref.chars().all(|x| x.is_ascii_hexdigit())
}

pub fn get_rev_of_ref(repo: &Repository, git_ref: &str) -> Result<String, GetRevOfBranchError> {
    if is_commit_hash(&git_ref) {
        return Ok(git_ref.to_string());
    }
    let git_ref = {
        if !git_ref.starts_with("refs/") {
            format!("refs/heads/{git_ref}")
        } else {
            git_ref.to_string()
        }
    };

    let mut remote = git2::Remote::create_detached(repo.url.clone())
        .map_err(|e| GetRevOfBranchError::Libgit(e))?;
    remote.connect(git2::Direction::Fetch)
        .map_err(|e| GetRevOfBranchError::Libgit(e))?;
    let list_result = remote.list()
        .map_err(|e| GetRevOfBranchError::Libgit(e))?;
    for remote_head in list_result.iter() {
        if remote_head.name() == git_ref {
            return Ok(format!("{:?}", remote_head.oid()))
        }
    }
    Err(GetRevOfBranchError::BranchNotFound(git_ref))
}

#[derive(Debug)]
pub enum NixPrefetchGitError {
    GetRevOfBranch(GetRevOfBranchError),
    IOError(io::Error),
    Parser(serde_json::Error),
}

// Incrementally via prev parameter
pub fn nix_prefetch_git_repo(repo: &Repository, git_ref: &str, prev: Option<FetchgitArgs>) -> Result<FetchgitArgs, NixPrefetchGitError> {
    let rev = get_rev_of_ref(repo, git_ref)
        .map_err(|e| NixPrefetchGitError::GetRevOfBranch(e))?;
    
    let fetch = if let Some(ref fetchgit_args) = prev {
        fetchgit_args.rev != rev
    } else {
        true
    };

    if fetch {
        let output = Command::new("nix-prefetch-git")
            .arg(&"--fetch-lfs")
            .arg(&repo.url)
            .arg("--rev")
            .arg(&rev)
            .output()
            .map_err(|e| NixPrefetchGitError::IOError(e))?;

        Ok(serde_json::from_slice(&output.stdout).map_err(|e| NixPrefetchGitError::Parser(e))?)
    } else {
        Ok(prev.unwrap())
    }
}
