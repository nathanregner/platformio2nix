use std::collections::BTreeMap;

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    manifest::{Manifest, PackageType},
    registry::{self, System, VersionSpec},
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "version")]
pub enum Lockfile {
    V1 {
        dependencies: BTreeMap<String, Dependency>,
    },
}

impl Lockfile {
    pub fn new(dependencies: Vec<Dependency>) -> Self {
        let dependencies = dependencies
            .into_iter()
            .map(|dep| (dep.name.clone(), dep))
            .collect();
        Self::V1 { dependencies }
    }
}

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Ord, PartialOrd, Clone, Copy, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum NixSystem {
    Aarch64Linux,
    Aarch64Darwin,
    #[serde(rename = "x86_64-linux")]
    X86_64Linux,
    #[serde(rename = "x86_64-darwin")]
    X86_64Darwin,
}

impl NixSystem {
    pub const ALL: [NixSystem; 4] = [
        NixSystem::Aarch64Linux,
        NixSystem::Aarch64Darwin,
        NixSystem::X86_64Linux,
        NixSystem::X86_64Darwin,
    ];

    pub fn to_registry(self) -> registry::System {
        match self {
            NixSystem::Aarch64Linux => System::LinuxAarch64,
            NixSystem::Aarch64Darwin => System::DarwinArm64,
            NixSystem::X86_64Linux => System::LinuxX86_64,
            NixSystem::X86_64Darwin => System::DarwinX86_64,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dependency {
    pub name: String,
    pub version: semver::Version,
    // #[serde(rename = "type")]
    // pub ty: PackageType,
    pub manifest: String,
    pub systems: BTreeMap<NixSystem, SystemDependency>,
}

impl Dependency {
    pub fn new(manifest: &Manifest, version: &VersionSpec) -> Self {
        let systems = NixSystem::ALL
            .iter()
            .filter_map(|nix_system| {
                let file = version.supports(&nix_system.to_registry());
                file.map(|file| (*nix_system, SystemDependency::from(file)))
            })
            .collect();
        Self {
            name: format!(
                "{}/{}",
                match manifest.ty {
                    PackageType::Platform => "platforms",
                    PackageType::Package | PackageType::Tool => "packages",
                    PackageType::Library => "libdeps",
                },
                manifest.spec.name
            ),
            manifest: serde_json::to_string(&manifest).expect("serializable manifest"),
            version: version.name.clone(),
            systems,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemDependency {
    pub url: Url,
    pub hash: String,
}

impl From<&registry::File> for SystemDependency {
    fn from(file: &registry::File) -> Self {
        Self {
            url: file.download_url.clone(),
            hash: format!("sha256-{}", BASE64_STANDARD.encode(&file.checksum.sha256)),
        }
    }
}
