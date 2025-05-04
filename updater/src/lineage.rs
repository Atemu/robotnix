use std::collections::HashMap;
use std::str;
use std::fs;
use std::fs::File;
use std::path::Path;
use atomic_write_file::AtomicWriteFile;
use std::io;
use std::io::Write;
use std::io::BufReader;
use serde::{Serialize, Deserialize};
use serde_json;
use quick_xml;
use reqwest;

use crate::base::{
    Repository,
    RepoProject,
    RepoProjectBranchSettings,
    NixPrefetchGitError,
    nix_prefetch_git_repo,
    FetchgitArgs,
};

use crate::repo_manifest::{
    GitRepoRemote,
    GitRepoDefaultRemote,
    GitRepoManifest,
    ReadManifestError,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceMetadata {
    pub branch: String,
    pub vendor: String,
    pub name: String,
    pub variant: String,
    pub deps: Vec<RepoProject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceDir {
    deps: HashMap<String, FetchgitArgs>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HudsonDevice {
    model: String,
    oem: String,
    name: String,
}

fn get_proprietary_repos_for_device(muppets_manifest: &GitRepoManifest, device: &str, branch: &str) -> Vec<RepoProject> {
    let mut repos = vec![];
    for entry in muppets_manifest.projects.iter() {
        let mut found = false;
        if let Some(groups) = &entry.groups {
            for m_group in groups.split(",") {
                if m_group == format!("muppets_{device}") {
                    found = true;
                    break;
                }
            }
            if found {
                let (remote_url, git_ref) = muppets_manifest.get_url_and_ref(
                    &entry.remote,
                    &entry.git_ref,
                    "https://github.com/TheMuppets/muppets_manifests"
                ).unwrap();
                repos.push(RepoProject {
                    path: entry.path.clone(),
                    nonfree: true,
                    branch_settings: {
                        let mut branch_settings = HashMap::new();
                        branch_settings.insert(branch.to_string(), RepoProjectBranchSettings {
                            repo: Repository::new(remote_url.clone(), entry.repo_name.clone()),
                            git_ref: git_ref,
                            linkfiles: HashMap::new(),
                            copyfiles: HashMap::new(),
                            // TODO dedupe group parsing code by writing a custom serde parser
                            groups: entry.groups.as_ref().map(
                                |x| x.split(",").map(|y| y.to_string()).collect()
                            ).unwrap_or(vec![]),
                        });
                        branch_settings
                    },
                });
            }
        }
    }

    repos
}

#[derive(Debug)]
pub enum FetchDeviceMetadataError {
    PrefetchGit(NixPrefetchGitError),
    FileRead(io::Error),
    FileWrite(io::Error),
    ReadManifest(ReadManifestError),
    Utf8(std::str::Utf8Error),
    InvalidLineageBuildTargets,
    Parser(serde_json::Error),
    ModelNotFoundInUpdaterDir(String),
    UnknownBranch(String),
    HttpError(reqwest::Error),
}

fn fetch_lineage_manifests_for_branches(branches: &[String]) -> Result<HashMap<String, GitRepoManifest>, FetchDeviceMetadataError> {
    let mut lineage_manifests = HashMap::new();
    for branch in branches.iter() {
        println!("Fetching LineageOS manifest repo (branch {})", &branch);
        let fetchgit_args = nix_prefetch_git_repo(
            &Repository::new("https://github.com".to_string(),
            "LineageOS/android".to_string()),
            &format!("refs/heads/{branch}"),
            None,
            false
        ).map_err(|e| FetchDeviceMetadataError::PrefetchGit(e))?;

        let manifest = GitRepoManifest::read_and_flatten(
            &Path::new(&fetchgit_args.path()),
            Path::new("default.xml")
        ).map_err(|e| FetchDeviceMetadataError::ReadManifest(e))?;

        lineage_manifests.insert(branch.to_string(), manifest);
    }

    Ok(lineage_manifests)
}

fn fetch_muppets_manifests_for_branches(branches: &[String]) -> Result<HashMap<String, GitRepoManifest>, FetchDeviceMetadataError> {
    let mut muppets_manifests = HashMap::new();
    for branch in branches.iter() {
        if !muppets_manifests.contains_key(branch) {
            println!("Fetching TheMuppets manifest (branch {branch})...");
            let muppets = nix_prefetch_git_repo(
                &Repository::new("https://github.com".to_string(), "TheMuppets/manifests".to_string()),
                &format!("refs/heads/{branch}"),
                None,
                false
            ).map_err(|e| FetchDeviceMetadataError::PrefetchGit(e))?;

            let mut muppets_manifest = GitRepoManifest::read(Path::new(&muppets.path()), Path::new("muppets.xml"))
                .map_err(|e| FetchDeviceMetadataError::ReadManifest(e))?;
            muppets_manifest.remotes.push(GitRepoRemote {
                name: "github".to_string(),
                fetch: "https://github.com".to_string(),
                default_ref: None,
                review: None,
            });
            muppets_manifest.default_remote = Some(GitRepoDefaultRemote {
                remote: "github".to_string(),
                default_ref: Some(branch.to_string()),
                sync_j: None,
                sync_c: None,
            });

            muppets_manifests.insert(branch.clone(), muppets_manifest);
        }
    }

    Ok(muppets_manifests)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LineageDependency {
    repository: String,
    target_path: String,

    #[serde(default)]
    branch: Option<String>,

    #[serde(default)]
    remote: Option<String>,
}

fn parse_build_targets(hudson_path: &str) -> Result<Vec<(String, String, String)>, FetchDeviceMetadataError> {
    let text_bytes = fs::read(format!("{}/lineage-build-targets", &hudson_path))
        .map_err(|e| FetchDeviceMetadataError::FileRead(e))?;
    let text = std::str::from_utf8(&text_bytes)
        .map_err(|e| FetchDeviceMetadataError::Utf8(e))?;
    let mut build_targets = vec![];
    for line in text.split("\n") {
        if line.starts_with("#") || line == "" {
            continue;
        }
        let fields: Vec<&str> = line.split(" ").collect();
        let device  = fields.get(0).ok_or(FetchDeviceMetadataError::InvalidLineageBuildTargets)?.to_string();
        let variant = fields.get(1).ok_or(FetchDeviceMetadataError::InvalidLineageBuildTargets)?.to_string();
        let branch  = fields.get(2).ok_or(FetchDeviceMetadataError::InvalidLineageBuildTargets)?.to_string();

        build_targets.push((device, variant, branch));
    }
    
    Ok(build_targets)
}


fn recursively_lookup_dependencies(repo_name: &str, branch: &str, main_branch: &str, dep_cache: &mut HashMap<String, Vec<LineageDependency>>) -> Result<Vec<LineageDependency>, FetchDeviceMetadataError> {
    println!("Fetching deps of {} (branch {})...", repo_name, branch);

    let url = format!("https://raw.githubusercontent.com/LineageOS/{repo_name}/refs/heads/{branch}/lineage.dependencies");
    let mut deps = match dep_cache.get(&url) {
        None => {
            let response = reqwest::blocking::get(&url)
                .map_err(|e| FetchDeviceMetadataError::HttpError(e))?;
            let deps = if response.status().is_success() {
                let json = response.text().map_err(|e| FetchDeviceMetadataError::HttpError(e))?;
                serde_json::from_str(&json)
                    .map_err(|e| FetchDeviceMetadataError::Parser(e))?
            } else {
                vec![]
            };
            dep_cache.insert(url, deps.clone());
            deps

        },
        Some(deps) => {
            println!("Cache hit.");
            deps.clone()
        },
    };

    let mut children_deps = vec![];
    for dep in deps.iter() {
        // If a repo isn't on the LineageOS GitHub org, we assume that it doesn't have a
        // lineage.dependencies.
        if let Some(_) = dep.remote {
            continue;
        }

        let next_deps = recursively_lookup_dependencies(
            &dep.repository,
            &dep.branch.as_ref().unwrap_or(&main_branch.to_string()),
            main_branch,
            dep_cache
        )?;

        for next_dep in next_deps {
            if !deps.iter().any(|x| x.target_path == next_dep.target_path) {
                children_deps.push(next_dep);
            }
        }
    }
    deps.append(&mut children_deps);

    Ok(deps)
}

fn fetch_lineage_dependencies(manifest: &GitRepoManifest, vendor: &str, device_name: &str, branch: &str, dep_cache: &mut HashMap<String, Vec<LineageDependency>>) -> Result<Vec<LineageDependency>, FetchDeviceMetadataError> {
    // Currently, we need to infer the vendor code from the human-readable vendor name (e.g.
    // `bananapi` from "Banana Pi". It would be cool to programmatically pull this from somewhere
    // though.
    // TODO softcode these overrides (maybe a JSON config file or something)
    let mut vendor_name = vendor.to_lowercase().replace(" ", "");
    if device_name == "deadpool" || device_name == "wade" || device_name == "dopinder" {
        vendor_name = "askey".to_string();
    } else if device_name == "deb" || device_name == "debx" {
        vendor_name = "asus".to_string();
    } else if device_name == "ingot" {
        vendor_name = "osom".to_string();
    }

    if vendor_name == "lg" {
        vendor_name = "lge".to_string();
    } else if vendor_name == "f(x)tec" {
        vendor_name = "fxtec".to_string();
    }

    let repo_name = format!("android_device_{vendor_name}_{device_name}");
    let mut deps = recursively_lookup_dependencies(
        &repo_name,
        branch,
        branch,
        dep_cache,
    )?;

    deps.insert(0, LineageDependency {
        repository: repo_name,
        target_path: format!("device/{vendor_name}/{device_name}"),
        branch: Some(branch.to_string()),
        remote: Some("github".to_string()),
    });

    Ok(deps)
}

pub fn fetch_device_metadata(device_metadata_path: &str, branch_whitelist: &Option<Vec<String>>, device_whitelist: &Option<Vec<String>>) -> Result<HashMap<String, DeviceMetadata>, FetchDeviceMetadataError> {
    println!("Fetching LineageOS hudson...");
    let hudson = nix_prefetch_git_repo(
        &Repository::new("https://github.com".to_string(), "LineageOS/hudson".to_string()),
        &"refs/heads/main",
        None,
        false
    ).map_err(|e| FetchDeviceMetadataError::PrefetchGit(e))?;

    let build_targets = parse_build_targets(&hudson.path())?;
    let mut all_branches = vec![];
    for (_, _, branch) in build_targets.iter() {
        if !all_branches.contains(branch) && branch_whitelist.as_ref().map(|xs| xs.contains(branch)).unwrap_or(true) {
            all_branches.push(branch.to_string())
        }
    }
    let lineage_manifests = fetch_lineage_manifests_for_branches(all_branches.as_ref())?;
    let muppets_manifests = fetch_muppets_manifests_for_branches(all_branches.as_ref())?;

    let reader = BufReader::new(File::open(format!("{}/updater/devices.json", &hudson.path()))
        .map_err(|e| FetchDeviceMetadataError::FileRead(e))?);
    let hudson_devices: Vec<HudsonDevice> = serde_json::from_reader(reader)
        .map_err(|e| FetchDeviceMetadataError::Parser(e))?;

    let mut device_metadata = HashMap::new();
    let mut dep_cache = HashMap::new();
    // TODO make this multi-branch as soon as I find out where to get the information about the
    // device's supported branches from.
    for (i, (device, variant, branch)) in build_targets.iter().enumerate() {
        if branch_whitelist.as_ref().map(|xs| !xs.contains(branch)).unwrap_or(false) || device_whitelist.as_ref().map(|xs| !xs.contains(device)).unwrap_or(false) {
            continue;
        }
        println!("At device {} ({}/{})", device, i+1, build_targets.len());
        let hudson_device = hudson_devices.iter().filter(|x| x.model == *device).next().ok_or(FetchDeviceMetadataError::ModelNotFoundInUpdaterDir(device.to_string()))?;
        let manifest = lineage_manifests.get(branch).unwrap();
        let real_branch = {
            // TODO currently we need to infer this, but there should be a better way.
            // TODO softcode this
            if branch == "lineage-21.0" {
                "lineage-21"
            } else {
                branch
            }
        };
        let deps = fetch_lineage_dependencies(
            &manifest,
            &hudson_device.oem,
            device,
            &real_branch,
            &mut dep_cache,
        )?;

        let mut projects = vec![];
        for dep in deps {
            let custom_ref = dep.branch.map(|x| format!("refs/heads/{x}"));
            let (remote_url, git_ref) = manifest.get_url_and_ref(
                &dep.remote,
                &custom_ref, 
                &"https://github.com/LineageOS/android"
            ).map_err(|e| FetchDeviceMetadataError::ReadManifest(e))?;

            // TODO softcode this too
            let git_ref = if git_ref == "refs/heads/lineage-21.0" {
                "refs/heads/lineage-21".to_string()
            } else {
                git_ref
            };

            let repo_name = if remote_url == "https://github.com" {
                format!("LineageOS/{}", dep.repository)
            } else {
                dep.repository.clone()
            };

            let project = RepoProject {
                nonfree: false,
                path: dep.target_path,
                branch_settings: {
                    let mut branch_settings = HashMap::new();
                    branch_settings.insert(branch.to_string(), RepoProjectBranchSettings {
                        repo: Repository::new(
                                  remote_url,
                                  repo_name,
                              ),
                        git_ref: git_ref,
                        groups: vec![],
                        copyfiles: HashMap::new(),
                        linkfiles: HashMap::new(),
                    });
                    branch_settings
                },
            };
            projects.push(project);
        }

        projects.append(&mut get_proprietary_repos_for_device(
                muppets_manifests.get(branch).unwrap(),
                &device,
                branch,
        ));

        device_metadata.insert(device.clone(), DeviceMetadata { 
            name: hudson_device.name.clone(),
            branch: branch.to_string(),
            variant: variant.to_string(),
            vendor: hudson_device.oem.clone(),
            deps: projects,
        });
    }

    let mut file = AtomicWriteFile::options()
        .open(device_metadata_path)
        .map_err(|e| FetchDeviceMetadataError::FileWrite(e))?;
    let buf = serde_json::to_string_pretty(&device_metadata)
        .map_err(|e| FetchDeviceMetadataError::Parser(e))?;

    file.write(buf.as_bytes()).map_err(|e| FetchDeviceMetadataError::FileWrite(e))?;
    file.commit().map_err(|e| FetchDeviceMetadataError::FileWrite(e))?;

    Ok(device_metadata)
}


#[derive(Debug)]
pub enum ReadDeviceMetadataError {
    ReadFile(io::Error),
    Parser(serde_json::Error),
}

pub fn read_device_metadata(path: &str) -> Result<HashMap<String, DeviceMetadata>, ReadDeviceMetadataError> {
    let file = File::open(path).map_err(|e| ReadDeviceMetadataError::ReadFile(e))?;
    let reader = BufReader::new(file);

    serde_json::from_reader(reader).map_err(|e| ReadDeviceMetadataError::Parser(e))
}
