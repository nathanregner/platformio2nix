use std::path::PathBuf;

use color_eyre::eyre::{self, Context};
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest::{Client, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use serde::{
    de::{DeserializeOwned, Visitor},
    Deserialize,
};
use sha2::{Digest, Sha256};

use crate::{
    lockfile::Dependency,
    manifest::{Artifact, ExternalSpec, PackageManifest, PackageType, PlatformIOSpec},
};

pub struct RegistryClient {
    client: ClientWithMiddleware,
    registry_url: Url,
}

impl Default for RegistryClient {
    fn default() -> Self {
        let cache_path = xdg::BaseDirectories::with_prefix("platformio2nix")
            .expect("valid base directories")
            .create_cache_directory("registry")
            .expect("valid cache directory");
        let client = ClientBuilder::new(Client::new())
            .with(Cache(HttpCache {
                mode: CacheMode::ForceCache,
                manager: CACacheManager { path: cache_path },
                options: HttpCacheOptions::default(),
            }))
            .build();
        Self {
            client,
            registry_url: Url::parse("https://api.registry.platformio.org")
                .expect("valid default registry"),
        }
    }
}

impl RegistryClient {
    pub async fn resolve(&self, artifact: Artifact) -> eyre::Result<Dependency> {
        match &artifact.manifest.spec {
            crate::manifest::PackageSpec::PlatformIO(PlatformIOSpec { owner, name, .. }) => {
                let package_spec = self
                    .get_package_spec(
                        owner,
                        artifact.manifest.ty,
                        name,
                        Some(artifact.manifest.version.to_string()),
                    )
                    .await?;
                Ok(Dependency::from_registry(
                    &artifact.manifest,
                    artifact.install_path,
                    package_spec,
                ))
            }
            crate::manifest::PackageSpec::External(package_spec) => {
                self.get_external(&artifact.manifest, artifact.install_path, package_spec)
                    .await
            }
        }
    }

    pub async fn get_external(
        &self,
        manifest: &PackageManifest,
        install_path: PathBuf,
        package_spec: &ExternalSpec,
    ) -> eyre::Result<Dependency> {
        let response = self.client.get(package_spec.uri.clone()).send().await?;
        let response = response.error_for_status()?;
        let bytes = response.bytes().await?;
        let mut hash = Sha256::new();
        hash.update(bytes);
        let hash = hash.finalize();
        Ok(Dependency::from_url(
            manifest,
            install_path,
            package_spec,
            &hash,
        ))
    }

    pub async fn get_package_spec(
        &self,
        owner: &str,
        ty: PackageType,
        name: &str,
        version: Option<String>,
    ) -> eyre::Result<PackageSpec> {
        let mut url = self.registry_url.clone();
        url.path_segments_mut()
            .expect("base path")
            .push("v3")
            .push("packages")
            .push(owner)
            .push(ty.as_str())
            .push(name);
        if let Some(version) = version {
            url.query_pairs_mut().append_pair("version", &version);
        }
        log::info!("Fetching package spec: {}", url);
        let response = self.client.get(url).send().await?;
        extract_json(response).await
    }
}

async fn extract_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, eyre::Error> {
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        let url = response.url().clone();
        let text = response.text().await?;
        eyre::bail!("HTTP {} for {}: {text}", status, url);
    }
    let text = response.text().await?;
    let de = &mut serde_json::Deserializer::from_str(&text);
    let body = serde_path_to_error::deserialize::<_, T>(de).with_context(|| text)?;
    Ok(body)
}

#[derive(Deserialize, Debug)]
pub struct PackageSpec {
    pub name: String,
    pub version: VersionSpec,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VersionSpec {
    pub name: String,
    pub files: Vec<File>,
}

impl VersionSpec {
    pub fn supports(&self, system: &System) -> Option<&File> {
        self.files
            .iter()
            .filter(|f| f.system.supports(system))
            .next()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct File {
    pub system: SystemSpec,
    pub download_url: Url,
    pub checksum: Checksum,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Checksum {
    #[serde(with = "hex")]
    pub sha256: Vec<u8>,
}

#[derive(Deserialize, Hash, Eq, PartialEq, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum System {
    DarwinX86_64,
    DarwinArm64,
    LinuxX86_64,
    LinuxAarch64,
    LinuxI686,
    #[serde(untagged)]
    Other(String),
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum SystemSpec {
    Wildcard,
    Systems(Vec<System>),
}

impl SystemSpec {
    pub fn supports(&self, system: &System) -> bool {
        match self {
            SystemSpec::Wildcard => true,
            SystemSpec::Systems(systems) => systems.contains(&system),
        }
    }
}

impl<'de> Deserialize<'de> for SystemSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = SystemSpec;

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v {
                    "*" => Ok(SystemSpec::Wildcard),
                    _ => Err(E::invalid_value(serde::de::Unexpected::Str(v), &"\"*\"")),
                }
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let systems = serde::de::Deserialize::deserialize(
                    serde::de::value::SeqAccessDeserializer::new(seq),
                )?;
                Ok(SystemSpec::Systems(systems))
            }

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("\"*\" or an array of systems")
            }
        }

        deserializer.deserialize_any(V)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn get() {
    //     let client = RegistryClient::default();
    //     let spec = client
    //         .get(
    //             "platformio",
    //             "tool",
    //             "toolchain-atmelavr",
    //             // Some("~1.70300.0".parse().unwrap()),
    //             None,
    //         )
    //         .unwrap();
    //     println!("{:?}", spec);
    // }

    #[test]
    fn deserialize_system() {
        fn assert(input: &str, expected: System) {
            assert_eq!(
                serde_json::from_str::<System>(&format!(r#""{input}""#)).unwrap(),
                expected
            );
        }

        assert("darwin_x86_64", System::DarwinX86_64);
        assert("darwin_arm64", System::DarwinArm64);
        assert("linux_x86_64", System::LinuxX86_64);
        assert("linux_aarch64", System::LinuxAarch64);
    }

    #[test]
    fn deserialize_platform_package_spec_atmelavr() {
        let json = include_str!("./test/platform-atmelavr.json");
        let de = &mut serde_json::Deserializer::from_str(json);
        let spec = serde_path_to_error::deserialize::<_, PackageSpec>(de);
        // TODO: snapshot test?
        match spec {
            Ok(spec) => {
                println!("{:?}", spec);
            }
            Err(err) => {
                panic!("failed to deserialize: {err}");
            }
        }
    }

    #[test]
    fn deserialize_toolchain_package_spec_atmelavr() {
        let json = include_str!("./test/toolchain-atmelavr.json");
        let de = &mut serde_json::Deserializer::from_str(json);
        let spec = serde_path_to_error::deserialize::<_, PackageSpec>(de);
        // TODO: snapshot test?
        match spec {
            Ok(spec) => {
                println!("{:?}", spec);
            }
            Err(err) => {
                panic!("failed to deserialize: {err}");
            }
        }
    }

    #[test]
    fn deserialize_lib_package_spec_invalid_semver() {
        let json = include_str!("./test/simplefoc.json");
        let de = &mut serde_json::Deserializer::from_str(json);
        let spec = serde_path_to_error::deserialize::<_, PackageSpec>(de);
        // TODO: snapshot test?
        match spec {
            Ok(spec) => {
                println!("{:?}", spec);
            }
            Err(err) => {
                panic!("failed to deserialize: {err}");
            }
        }
    }
}
