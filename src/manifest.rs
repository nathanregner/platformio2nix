use std::{collections::HashMap, path::Path};

use color_eyre::eyre::{self, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Platform,
    Package,
    Tool,
}

impl PackageType {
    pub fn as_str(&self) -> &str {
        match self {
            PackageType::Platform => "platform",
            PackageType::Package => "package",
            PackageType::Tool => "tool",
        }
    }
}

/// .piopm package manifest file
#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    #[serde(rename = "type")]
    pub ty: PackageType,
    pub version: semver::Version,
    pub spec: PackageSpec,

    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PackageSpec {
    pub owner: String,
    pub name: String,

    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

pub fn extract_manifests(dir: &Path) -> Result<Vec<Manifest>, eyre::Error> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut manifests = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let path = entry.path().join(".piopm");
        if !path.exists() {
            continue;
        }

        let json = std::fs::read_to_string(&path)?;
        let de = &mut serde_json::Deserializer::from_str(&json);
        let manifest = serde_path_to_error::deserialize::<_, Manifest>(de).wrap_err_with(|| {
            format!("failed to parse manifest file: {}", path.to_string_lossy())
        })?;
        manifests.push(manifest);
    }

    Ok(manifests)
}
