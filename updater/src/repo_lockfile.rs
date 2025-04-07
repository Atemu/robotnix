use std::collections::HashMap;
use std::str;
use std::fs;
use std::io;
use std::io::Write;
use serde_json;
use atomic_write_file::AtomicWriteFile;

use crate::base::{
    RepoProject,
    FetchgitArgs,
    nix_prefetch_git_repo,
    NixPrefetchGitError,
    GetRevOfBranchError,
};

pub type PathInSourceTree = String;
pub type RepoLockfile = HashMap<PathInSourceTree, Option<FetchgitArgs>>;

#[derive(Debug)]
pub enum SaveRepoLockfileError {
    FileWrite(io::Error),
    Serialize(serde_json::Error),
}

fn save_repo_lockfile(filename: &str, lockfile: &RepoLockfile) -> Result<(), SaveRepoLockfileError> {
    let mut file = AtomicWriteFile::options()
        .open(filename)
        .map_err(|e| SaveRepoLockfileError::FileWrite(e))?;
    let buf = serde_json::to_string_pretty(&lockfile)
        .map_err(|e| SaveRepoLockfileError::Serialize(e))?;

    file.write(buf.as_bytes()).map_err(|e| SaveRepoLockfileError::FileWrite(e))?;
    file.commit().map_err(|e| SaveRepoLockfileError::FileWrite(e))?;
    
    Ok(())
}

#[derive(Debug)]
pub enum IncrementallyFetchReposError {
    Utf8(str::Utf8Error),
    Parser(serde_json::Error),
    NixPrefetch(NixPrefetchGitError),
    SaveLockfile(SaveRepoLockfileError),
}

pub fn incrementally_fetch_projects(filename: &str, projects: &[RepoProject], branch: &str) -> Result<RepoLockfile, IncrementallyFetchReposError> {
    let mut lockfile: RepoLockfile = match fs::read(filename) {
        Ok(lockfile_json) => {
            let lockfile_json_str = str::from_utf8(&lockfile_json)
                .map_err(|e| IncrementallyFetchReposError::Utf8(e))?;
            serde_json::from_str(&lockfile_json_str)
                .map_err(|e| IncrementallyFetchReposError::Parser(e))?
        },
        Err(_) => {
            println!("Error reading saved lockfile, starting from scratch...");
            HashMap::new()
        }
    };

    for (i, project) in projects.iter().enumerate() {
        let settings = match project.branch_settings.get(branch) {
            Some(settings) => settings,
            None => continue,
        };
        println!("Fetching repo {} ({}/{})", settings.repo.url, i+1, projects.len());
        let old = if let Some(Some(fetchgit_args)) = lockfile.get(&project.path) {
            Some(fetchgit_args.clone())
        } else {
            None
        };

        let new = match nix_prefetch_git_repo(&settings.repo, &settings.git_ref, old) {
            Ok(args) => Some(args),
            Err(e) => return Err(IncrementallyFetchReposError::NixPrefetch(e)),
        };

        lockfile.insert(project.path.clone(), new);

        save_repo_lockfile(filename, &lockfile)
            .map_err(|e| IncrementallyFetchReposError::SaveLockfile(e))?;
    }

    Ok(lockfile)
}

// DONE
