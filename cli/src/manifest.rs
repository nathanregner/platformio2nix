use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use color_eyre::eyre::{self, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub struct Artifact {
    pub manifest: PackageManifest,
    pub install_path: PathBuf,
}

/// .piopm package manifest file
#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum PackageSpec {
    PlatformIO(PlatformIOSpec),
    External(ExternalSpec),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PlatformIOSpec {
    pub owner: String,
    pub name: String,
    #[serde(flatten)]
    _extra: BTreeMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExternalSpec {
    pub name: String,
    pub uri: Url,
    #[serde(flatten)]
    _extra: BTreeMap<String, Value>,
}

pub fn extract_artifacts(root: &Path) -> eyre::Result<Vec<Artifact>> {
    let mut artifacts = vec![];
    extract_artifacts_rec(&mut artifacts, &root, &root)?;
    Ok(artifacts)
}

fn extract_artifacts_rec(
    artifacts: &mut Vec<Artifact>,
    root: &Path,
    dir: &Path,
) -> eyre::Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {dir:?}"))? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();

        let piopm = path.join(".piopm");
        if !piopm.exists() {
            extract_artifacts_rec(artifacts, &root, &path)?;
            continue;
        }

        let json = std::fs::read_to_string(&piopm)?;
        let de = &mut serde_json::Deserializer::from_str(&json);
        let manifest =
            serde_path_to_error::deserialize::<_, PackageManifest>(de).wrap_err_with(|| {
                format!("failed to parse manifest file: {}", piopm.to_string_lossy())
            })?;
        let install_path = path
            .strip_prefix(root)
            .wrap_err_with(|| {
                format!("File {dir:?} is not a child of {root:?}: followed a symlink?")
            })?
            .to_path_buf();
        artifacts.push(Artifact {
            manifest,
            install_path,
        });
    }

    Ok(())
}
