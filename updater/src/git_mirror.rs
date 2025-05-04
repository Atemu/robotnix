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

            let repo_path = mirror_path.join(&format!("{}.git", settings.repo.name));
            if !repo_path.try_exists().unwrap() {
                // Initial clone
                println!("Checkout doesn't exist yet, performing initial clone...");
                let output = Command::new("git")
                    .arg("clone")
                    .arg("--bare")
                    .arg("--single-branch")
                    .arg("--depth=1")
                    .arg("--revision")
                    .arg(&settings.git_ref)
                    .arg(&settings.repo.url())
                    .arg(&repo_path)
                    .output()
                    .unwrap();

                if !output.status.success() {
                    println!("{}", std::str::from_utf8(&output.stderr).unwrap());
                }

                // A clone with `--revision` does not create any refs, only
                // updates HEAD. Make the local mirror of the ref we fetched
                // point to the newly fetched HEAD
                Command::new("git")
                    .current_dir(&repo_path)
                    .arg("update-ref")
                    .arg(&settings.git_ref)
                    .arg("HEAD")
                    .output()
                    .unwrap()
            } else {
                let output = Command::new("git")
                    .arg("fetch")
                    .current_dir(&repo_path)
                    .arg("--depth=1")
                    .arg("origin")
                    .arg(&settings.git_ref)
                    .output()
                    .unwrap();

                if !output.status.success() {
                    println!("{}", std::str::from_utf8(&output.stderr).unwrap());
                }

                // A fetch does not create any refs, only updates FETCH_HEAD.
                // Make the local mirror of the ref we fetched point to the
                // newly fetched HEAD
                Command::new("git")
                    .current_dir(&repo_path)
                    .arg("update-ref")
                    .arg(&settings.git_ref)
                    .arg("FETCH_HEAD")
                    .output()
                    .unwrap()
            };
        }
    }
}
