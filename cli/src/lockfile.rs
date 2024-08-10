use std::collections::BTreeMap;

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    manifest::{ExternalSpec, Manifest, PackageType},
    registry,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "version")]
pub enum Lockfile {
    V1 {
        dependencies: BTreeMap<String, Dependency>,
    },
}

impl Default for Lockfile {
    fn default() -> Self {
        Self::V1 {
            dependencies: BTreeMap::default(),
        }
    }
}

impl Lockfile {
    pub fn insert(&mut self, dependency: Dependency) {
        let Self::V1 { dependencies } = self;
        dependencies.insert(dependency.name.clone(), dependency);
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
            NixSystem::Aarch64Linux => registry::System::LinuxAarch64,
            NixSystem::Aarch64Darwin => registry::System::DarwinArm64,
            NixSystem::X86_64Linux => registry::System::LinuxX86_64,
            NixSystem::X86_64Darwin => registry::System::DarwinX86_64,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dependency {
    pub name: String,
    pub install_path: String,
    pub version: semver::Version,
    pub manifest: String,
    pub src: Src,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "external")]
pub enum Src {
    Universal(FetchUrl),
    PlatformSpecific(BTreeMap<NixSystem, FetchUrl>),
}

impl Dependency {
    pub fn from_url(manifest: &Manifest, package_spec: &ExternalSpec, sha256: &[u8]) -> Self {
        Self::new(
            manifest,
            package_spec.name.clone(),
            manifest.version.clone(),
            Src::Universal(FetchUrl::new(package_spec.uri.clone(), sha256)),
        )
    }

    pub fn from_registry(manifest: &Manifest, package_spec: &registry::PackageSpec) -> Self {
        let version = &package_spec.version;
        let src = match version.universal() {
            Some(file) => Src::Universal(FetchUrl::from(file)),
            None => Src::PlatformSpecific(
                NixSystem::ALL
                    .iter()
                    .filter_map(|nix_system| {
                        let file = version.supports(&nix_system.to_registry());
                        file.map(|file| (*nix_system, FetchUrl::from(file)))
                    })
                    .collect(),
            ),
        };
        Self::new(
            manifest,
            package_spec.name.clone(),
            version.name.clone(),
            src,
        )
    }

    fn new(manifest: &Manifest, name: String, version: semver::Version, src: Src) -> Self {
        let install_path = format!(
            "{}/{}",
            match manifest.ty {
                PackageType::Platform => "platforms",
                PackageType::Package | PackageType::Tool => "packages",
                PackageType::Library => "libdeps",
            },
            name
        );
        Self {
            name,
            install_path,
            manifest: serde_json::to_string(&manifest).expect("serializable manifest"),
            version,
            src,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FetchUrl {
    pub url: Url,
    pub hash: String,
}

impl FetchUrl {
    pub fn new(url: Url, sha256: &[u8]) -> Self {
        Self {
            url,
            hash: format!("sha256-{}", BASE64_STANDARD.encode(sha256)),
        }
    }
}

impl From<&registry::File> for FetchUrl {
    fn from(file: &registry::File) -> Self {
        Self::new(file.download_url.clone(), &file.checksum.sha256)
    }
}
