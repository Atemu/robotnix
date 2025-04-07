use std::vec::Vec;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Write;
use std::str;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use quick_xml;
use atomic_write_file::AtomicWriteFile;

use crate::base::{
    Repository,
    RepoProject,
    RepoProjectBranchSettings,
    nix_prefetch_git_repo,
    NixPrefetchGitError
};

#[derive(Debug, Serialize, Deserialize)]
pub struct GitRepoRemote {
    #[serde(rename = "@name")]
    name: String,

    #[serde(rename = "@fetch")]
    fetch: String,

    #[serde(rename = "@review")]
    review: Option<String>,

    #[serde(rename = "@revision")]
    default_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitRepoDefaultRemote {
    #[serde(rename = "@remote")]
    remote: String,

    #[serde(rename = "@revision")]
    default_ref: Option<String>,

    #[serde(rename = "@sync-c")]
    sync_c: Option<String>,

    #[serde(rename = "@sync-j")]
    sync_j: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitRepoFile {
    #[serde(rename = "@src")]
    src: String,

    #[serde(rename = "@dest")]
    dest: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitRepoProject {
    #[serde(rename = "@path")]
    pub path: String,

    #[serde(rename = "@name")]
    pub repo_name: String,

    #[serde(rename = "@groups")]
    pub groups: Option<String>,

    #[serde(rename = "@remote")]
    pub remote: Option<String>,

    #[serde(rename = "@revision")]
    pub git_ref: Option<String>,

    #[serde(rename = "linkfile", default)]
    pub linkfiles: Vec<GitRepoFile>,

    #[serde(rename = "copyfile", default)]
    pub copyfiles: Vec<GitRepoFile>,
}

// TODO use Path and PathBuf everywhere where they're applicable
#[derive(Debug, Serialize, Deserialize)]
pub struct GitRepoInclude {
    #[serde(rename = "@name")]
    name: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "manifest")]
pub struct GitRepoManifest {
    #[serde(default, rename = "remote")]
    pub remotes: Vec<GitRepoRemote>,

    #[serde(rename = "default")]
    pub default_remote: Option<GitRepoDefaultRemote>,

    #[serde(rename = "project", default)]
    pub projects: Vec<GitRepoProject>,

    #[serde(rename = "include", default)]
    pub includes: Vec<GitRepoInclude>,
}

#[derive(Debug)]
pub enum ReadManifestError {
    FileRead(io::Error),
    Utf8(str::Utf8Error),
    Parser(quick_xml::errors::serialize::DeError),
    MissingDefaultRemote,
    MissingDefaultRef,
    UnknownRemote(String),
    MoreThanOneDefaultRemote,
}

impl GitRepoManifest {
    pub fn read(manifest_path: &Path, filename: &Path) -> Result<GitRepoManifest, ReadManifestError> {
        let manifest_xml_bytes = fs::read(manifest_path.join(filename))
            .map_err(|e| ReadManifestError::FileRead(e))?;

        let manifest_xml = str::from_utf8(&manifest_xml_bytes)
            .map_err(|e| ReadManifestError::Utf8(e))?;

        let manifest: GitRepoManifest = quick_xml::de::from_str(&manifest_xml)
            .map_err(|e| ReadManifestError::Parser(e))?;

        Ok(manifest)
    }


    // produces a merged/flattened manifest out of a manifest and its dependencies/includes
    pub fn read_and_flatten(manifest_path: &Path, filename: &Path) -> Result<GitRepoManifest, ReadManifestError> {
        let mut manifest = GitRepoManifest::read(manifest_path, filename)?;

        for include in manifest.includes.iter() {
            let mut submanifest = GitRepoManifest::read_and_flatten(manifest_path, &include.name)?;

            manifest.remotes.append(&mut submanifest.remotes);
            manifest.projects.append(&mut submanifest.projects);
            
            // manifests have default remotes which they specify using the `default` xml tag
            // Included manifests can have default remotes too
            // When flattening, we only want one default_remote
            if let Some(default_remote) = submanifest.default_remote {
                if let None = manifest.default_remote {
                    manifest.default_remote = Some(default_remote);
                } else {
                    return Err(ReadManifestError::MoreThanOneDefaultRemote);
                }
            }
        }

        // We flattened it, so no more includes
        manifest.includes = vec![];
        Ok(manifest)
    }

    // Utility function that generates a mapping of remote names to normalised
    // remote URLs for a given manifest root URL
    fn get_remote_specs(&self, root_url: &str) -> HashMap<String, RemoteSpec> {
        let mut remote_specs = HashMap::new();
        for remote in self.remotes.iter() {
            let is_default_remote = self.default_remote
                .as_ref()
                .map(|x| x.remote == remote.name)
                .unwrap_or(false);
            let default_ref = if is_default_remote {
                self.default_remote
                    .as_ref()
                    .unwrap()
                    .default_ref
                    .as_ref()
                    .or(remote.default_ref.as_ref())
            } else {
                remote.default_ref.as_ref()
            };
            let remote_url_stripped = remote.fetch.strip_suffix('/').unwrap_or(&remote.fetch).to_string();
            let root_url_stripped = root_url.strip_suffix('/').unwrap_or(&root_url).to_string();
            remote_specs.insert(remote.name.clone(), RemoteSpec {
                url: {
                    if remote.fetch != ".." {
                        remote_url_stripped
                    } else {
                        let url_parts: Vec<String> = root_url
                            .split("/")
                            .map(|x| x.to_string())
                            .collect();
                        url_parts[0..url_parts.len()-2].join("/")
                    }
                },
                default_ref: default_ref.map(|x| x.to_string()),
            });
        }

        remote_specs
    }

    // Get URL and ref for one specific entry in the manifest
    // TODO type alias?
    pub fn get_url_and_ref(&self, remote: &Option<String>, ref_override: &Option<String>, root_url: &str) -> Result<(String, String), ReadManifestError> {
        let remote_specs = self.get_remote_specs(root_url);
        let remote_name = remote
            .as_ref()
            .unwrap_or(
                &self.default_remote.as_ref().ok_or(ReadManifestError::MissingDefaultRemote)?.remote
            );
        let remote_spec = remote_specs.get(remote_name)
            .ok_or(ReadManifestError::UnknownRemote(remote_name.to_string()))?;

        let git_ref = ref_override
            .as_ref()
            .unwrap_or(
                remote_spec.default_ref.as_ref().unwrap_or(
                    self.default_remote.as_ref().ok_or(
                        ReadManifestError::MissingDefaultRef
                    )?
                    .default_ref.as_ref().ok_or(
                        ReadManifestError::MissingDefaultRef
                    )?
                )
            )
            .clone();

        Ok((remote_spec.url.clone(), git_ref))
    }

    // Modifies the projects map to include the branch-specific repo project data for a given branch
    fn get_projects(&self, projects: &mut HashMap<String, RepoProject>, root_url: &str, branch: &str) -> Result<(), FetchGitRepoMetadataError> {
        for project in self.projects.iter() {
            let (remote_url, git_ref) = self.
                get_url_and_ref(&project.remote, &project.git_ref, root_url)
                .map_err(|e| FetchGitRepoMetadataError::ReadManifest(e))?;
            let project_url = format!("{}/{}", &remote_url, &project.repo_name);

            if !projects.contains_key(&project.path) {
                projects.insert(project.path.clone(), RepoProject {
                    path: project.path.clone(),
                    nonfree: false,
                    branch_settings: HashMap::new(),
                });
            }

            let branch_settings = &mut projects
                .get_mut(&project.path.clone())
                .unwrap()
                .branch_settings;
            branch_settings.insert(branch.to_string(), RepoProjectBranchSettings {
                repo: Repository {
                    url: project_url,
                },
                copyfiles: {
                    let mut files = HashMap::new();
                    for c in project.copyfiles.iter() {
                        files.insert(c.dest.clone(), c.src.clone());
                    }
                    files
                },
                linkfiles: {
                    let mut files = HashMap::new();
                    for l in project.linkfiles.iter() {
                        files.insert(l.dest.clone(), l.src.clone());
                    }
                    files
                },
                groups: project.groups.as_ref().map(
                            |x| x.split(",").map(|y| y.to_string()).collect()
                        ).unwrap_or(vec![]),
                git_ref: git_ref,
            });
        }

        Ok(())
    }
}

// This represents two xml attrs that always come in pairs:
// `fetch` and `revision`
struct RemoteSpec {
    url: String,
    default_ref: Option<String>,
}



#[derive(Debug)]
pub enum FetchGitRepoMetadataError {
    PrefetchGit(NixPrefetchGitError),
    ReadManifest(ReadManifestError),
    UnknownRemote(String),
    MissingDefaultRemote,
    MissingDefaultRef,
    FileWrite(io::Error),
    Parser(serde_json::Error),
}

// Fetches metadata for one git-repo manifest
// Potentially multiple branches
// Writes metadata into filename
pub fn fetch_git_repo_metadata(filename: &str, manifest_repo: &Repository, branches: &[String]) -> Result<Vec<RepoProject>, FetchGitRepoMetadataError> {
    let mut projects: HashMap<String, RepoProject> = HashMap::new();

    for branch in branches.iter() {
        println!("Fetching manifest repo {} (branch {})", &manifest_repo.url, &branch);
        let fetchgit_args = nix_prefetch_git_repo(manifest_repo, &format!("refs/heads/{branch}"), None)
            .map_err(|e| FetchGitRepoMetadataError::PrefetchGit(e))?;

        let manifest = GitRepoManifest::read_and_flatten(
            &Path::new(&fetchgit_args.path()),
            Path::new("default.xml")
        ).map_err(|e| FetchGitRepoMetadataError::ReadManifest(e))?;

        manifest.get_projects(&mut projects, &manifest_repo.url, branch)?;
    }

    let mut projects: Vec<RepoProject> = projects.values().cloned().collect();
    projects.sort_by_key(|p| p.path.clone());

    let projects_json = serde_json::to_string_pretty(&projects)
        .map_err(|e| FetchGitRepoMetadataError::Parser(e))?;
    let mut file = AtomicWriteFile::options().open(filename)
        .map_err(|e| FetchGitRepoMetadataError::FileWrite(e))?;
    file.write(projects_json.as_bytes())
        .map_err(|e| FetchGitRepoMetadataError::FileWrite(e))?;
    file.commit()
        .map_err(|e| FetchGitRepoMetadataError::FileWrite(e))?;

    Ok(projects)
}

// DONE
