use std::process::Output;

use color_eyre::eyre::{self, Context};
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest::{Client, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use serde::{
    de::{DeserializeOwned, Visitor},
    Deserialize,
};
use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::{
    lockfile::Dependency,
    manifest::{Artifact, ExternalSpec, PackageManifest, PackageType},
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
        let manifest = &artifact.manifest;
        let name = manifest.spec.name();
        log::info!("Resolving {name}...");
        match &manifest.spec {
            crate::manifest::PackageSpec::External(spec) => {
                match spec.uri.as_str().strip_prefix("git+") {
                    Some(_) => self.get_git(manifest, name, &artifact.full_path).await,
                    None => self.get_external(manifest, spec).await,
                }
            }
            crate::manifest::PackageSpec::PlatformIO(spec) => {
                let package_spec = self
                    .get_package_spec(
                        &spec.owner,
                        manifest.ty,
                        name,
                        Some(manifest.version.to_string()),
                    )
                    .await?;
                Ok(Dependency::from_registry(manifest.clone(), package_spec))
            }
        }
    }

    async fn get_git(
        &self,
        manifest: &PackageManifest,
        name: &str,
        piopm_path: &std::path::Path,
    ) -> eyre::Result<Dependency> {
        // The .piopm is inside .git/; the working tree is its parent.
        let repo_path = if piopm_path.file_name() == Some(std::ffi::OsStr::new(".git")) {
            piopm_path.parent().unwrap_or(piopm_path)
        } else {
            piopm_path
        };

        let remote = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["remote", "get-url", "origin"])
            .output_success()
            .await
            .context("running git remote get-url")?;

        let base_url = Url::parse(String::from_utf8(remote.stdout)?.trim())?;

        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["rev-parse", "HEAD"])
            .output_success()
            .await
            .context("running git rev-parse")?;

        let rev = String::from_utf8(output.stdout)?.trim().to_string();
        log::info!("Resolved git {name} to rev {rev}");

        let file_url = format!("file://{}", repo_path.display());
        let prefetch = Command::new(env!("NIX_PREFETCH_GIT"))
            .args([
                "--url",
                &file_url,
                "--rev",
                &rev,
                "--fetch-submodules",
                "--quiet",
            ])
            .output_success()
            .await
            .context("running nix-prefetch-git")?;

        #[derive(Deserialize)]
        struct PrefetchOutput {
            hash: String,
        }
        let PrefetchOutput { hash } =
            serde_json::from_slice(&prefetch.stdout).context("parsing nix-prefetch-git output")?;

        Ok(Dependency::from_git(
            manifest.clone(),
            name.to_string(),
            base_url,
            rev,
            hash,
        ))
    }

    async fn get_external(
        &self,
        manifest: &PackageManifest,
        package_spec: &ExternalSpec,
    ) -> eyre::Result<Dependency> {
        let response = self.client.get(package_spec.uri.clone()).send().await?;
        let response = response.error_for_status()?;
        let bytes = response.bytes().await?;
        let mut hash = Sha256::new();
        hash.update(bytes);
        let hash = hash.finalize();
        Ok(Dependency::from_url(manifest.clone(), package_spec, &hash))
    }

    async fn get_package_spec(
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

trait CommandExt {
    async fn output_success(&mut self) -> eyre::Result<Output>
    where
        Self: Sized;
}

impl CommandExt for Command {
    async fn output_success(&mut self) -> eyre::Result<Output> {
        let output = self.output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eyre::bail!("process exited with {}: {stderr}", output.status);
        }
        Ok(output)
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
    pub files: Vec<File>,
}

impl VersionSpec {
    pub fn supports(&self, system: &System) -> Option<&File> {
        self.files.iter().find(|f| f.system.supports(system))
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
            SystemSpec::Systems(systems) => systems.contains(system),
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
