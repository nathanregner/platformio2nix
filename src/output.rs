use std::collections::HashMap;

use semver::Version;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::registry::{self, PackageSpec, System, VersionSpec};

#[derive(Serialize, Deserialize, Hash, Eq, PartialEq, Clone, Copy, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum NixSystem {
    Aarch64Linux,
    Aarch64Darwin,
    X86_64Linux,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Dependency {
    pub name: String,
    pub version: semver::Version,
    #[serde(rename = "type")]
    pub ty: String,
    pub systems: HashMap<NixSystem, SystemDependency>,
}

impl Dependency {
    pub fn new(spec: &PackageSpec, version: &VersionSpec) -> Self {
        let systems = NixSystem::ALL
            .iter()
            .filter_map(|nix_system| {
                let file = version.supports(&nix_system.to_registry());
                file.map(|file| (*nix_system, SystemDependency::from(file)))
            })
            .collect();
        Self {
            name: spec.name.clone(),
            ty: spec.ty.clone(),
            version: version.name.clone(),
            systems,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SystemDependency {
    pub sha256: String,
    pub download_url: Url,
}

impl From<&registry::File> for SystemDependency {
    fn from(file: &registry::File) -> Self {
        Self {
            sha256: file.checksum.sha256.clone(),
            download_url: file.download_url.clone(),
        }
    }
}
