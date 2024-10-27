use std::{collections::BTreeMap, path::Path};

use color_eyre::eyre::{self, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

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

/// .piopm package manifest file
#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    #[serde(rename = "type")]
    pub ty: PackageType,
    pub version: String,
    pub spec: PackageSpec,

    #[serde(flatten)]
    _extra: BTreeMap<String, Value>,
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

pub fn extract_manifests(root: &Path) -> Result<Vec<Manifest>, eyre::Error> {
    let mut manifests = vec![];
    extract_manifests_rec(&mut manifests, &root)?;
    Ok(manifests)
}

fn extract_manifests_rec(manifests: &mut Vec<Manifest>, dir: &Path) -> Result<(), eyre::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let piopm = entry.path().join(".piopm");
        if !piopm.exists() {
            extract_manifests_rec(manifests, &entry.path())?;
            continue;
        }

        let json = std::fs::read_to_string(&piopm)?;
        let de = &mut serde_json::Deserializer::from_str(&json);
        let manifest = serde_path_to_error::deserialize::<_, Manifest>(de).wrap_err_with(|| {
            format!("failed to parse manifest file: {}", piopm.to_string_lossy())
        })?;
        manifests.push(manifest);
        continue;
    }

    Ok(())
}
