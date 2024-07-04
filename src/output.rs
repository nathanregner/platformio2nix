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
struct Dependency {
    #[serde(rename = "type")]
    ty: String,
    systems: HashMap<NixSystem, String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct SystemDependency {
    sha256: String,
    download_url: Url,
}
