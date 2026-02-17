use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use color_eyre::eyre::{self, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub struct Artifact {
    pub manifest: PackageManifest,
    /// Relative install path for the lockfile (`.git` suffix stripped for git packages)
    pub install_path: PathBuf,
    /// Absolute path where the `.piopm` file was found
    #[serde(skip)]
    pub full_path: PathBuf,
}

/// .piopm package manifest file
#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct PackageManifest {
    #[serde(rename = "type")]
    pub ty: PackageType,
    pub version: String,
    pub spec: PackageSpec,

    #[serde(flatten)]
    _extra: BTreeMap<String, Value>,
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Platform,
    Package,
    Tool,
    Library,
}

impl PackageType {
    pub fn as_str(&self) -> &str {
        match self {
            PackageType::Platform => "platform",
            PackageType::Package | PackageType::Library => "library",
            PackageType::Tool => "tool",
        }
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum PackageSpec {
    External(ExternalSpec),
    PlatformIO(PlatformIOSpec),
}

impl PackageSpec {
    pub fn name(&self) -> &str {
        match self {
            PackageSpec::External(s) => &s.name,
            PackageSpec::PlatformIO(s) => &s.name,
        }
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct PlatformIOSpec {
    pub owner: String,
    pub name: String,
    #[serde(flatten)]
    _extra: BTreeMap<String, Value>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct ExternalSpec {
    pub name: String,
    pub uri: Url,
    #[serde(flatten)]
    _extra: BTreeMap<String, Value>,
}

pub fn extract_artifacts(root: &Path) -> eyre::Result<Vec<Artifact>> {
    let mut artifacts = vec![];
    extract_artifacts_rec(&mut artifacts, &PathBuf::default(), root)?;
    Ok(artifacts)
}

fn extract_artifacts_rec(
    artifacts: &mut Vec<Artifact>,
    parent: &Path,
    dir: &Path,
) -> eyre::Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {dir:?}"))? {
        let entry = entry?;

        let path = fs::canonicalize(entry.path())?;
        if !path.is_dir() {
            continue;
        }

        let parent = parent.join(entry.file_name());

        let piopm = path.join(".piopm");
        if !piopm.exists() {
            extract_artifacts_rec(artifacts, &parent, &path)?;
            continue;
        }

        let json = std::fs::read_to_string(&piopm)?;
        let de = &mut serde_json::Deserializer::from_str(&json);
        let manifest =
            serde_path_to_error::deserialize::<_, PackageManifest>(de).wrap_err_with(|| {
                format!("failed to parse manifest file: {}", piopm.to_string_lossy())
            })?;
        // For git packages, .piopm lives inside .git/; strip that suffix so the
        // install path points to the actual working-tree directory.
        let install_path = if parent.file_name() == Some(OsStr::new(".git")) {
            parent.parent().unwrap_or(&parent).to_path_buf()
        } else {
            parent
        };
        artifacts.push(Artifact {
            manifest,
            install_path,
            full_path: path,
        });
    }

    Ok(())
}
