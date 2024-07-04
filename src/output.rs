use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::registry::{self, System};

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
    #[serde(rename = "type")]
    pub ty: String,
    pub systems: HashMap<NixSystem, SystemDependency>,
}

impl Dependency {
    pub fn new(ty: String) -> Self {
        Self {
            ty,
            systems: Default::default(),
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
