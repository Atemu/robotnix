use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use std::process::Command;

use crate::base::RepoProject;

pub fn parse_robotnix_git_mirrors_env(robotnix_git_mirrors: &str) -> HashMap<String, PathBuf> {
    robotnix_git_mirrors
        .split("|")
        .map(|x| {
            let mut fields = x.split("=");
            (fields.next().unwrap().to_string(), PathBuf::from(fields.next().unwrap()))
        })
        .collect()
}

pub fn update_git_mirrors(projects: &[RepoProject], branches: &[String], mirrors: &HashMap<String, PathBuf>) {
    for branch in branches.iter() {
        println!("At branch {branch}");
        for project in projects.iter() {
            let settings = match project.branch_settings.get(branch) {
                Some(settings) => settings,
                None => continue,
            };
            println!("Mirroring {}...", settings.repo.url());

            let mirror_path = mirrors.get(&settings.repo.base_url).unwrap();
            if !mirror_path.try_exists().unwrap() {
                fs::create_dir(&mirror_path).unwrap();
            }

            let repo_branch = if settings.git_ref.starts_with("refs/heads/") {
                settings.git_ref.strip_prefix("refs/heads/").unwrap()
            } else if settings.git_ref.starts_with("refs/tags") {
                settings.git_ref.strip_prefix("refs/tags/").unwrap()
            } else {
                &settings.git_ref
            };

            let repo_path = mirror_path.join(&format!("{}.git", settings.repo.name));
            let output = if !repo_path.try_exists().unwrap() {
                // Initial clone
                println!("Checkout doesn't exist yet, performing initial clone...");
                Command::new("git")
                    .arg("clone")
                    .arg("--bare")
                    .arg("--single-branch")
                    .arg("--branch")
                    .arg(repo_branch)
                    .arg(&settings.repo.url())
                    .arg(&repo_path)
                    .output()
                    .unwrap()
            } else {
                Command::new("git")
                    .arg("fetch")
                    .arg("-C")
                    .arg(&repo_path)
                    .arg("origin")
                    .arg(&settings.git_ref)
                    .output()
                    .unwrap()
            };
            if !output.status.success() {
                println!("{}", std::str::from_utf8(&output.stderr).unwrap());
            }
        }
    }
}
