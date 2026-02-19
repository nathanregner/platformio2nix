use std::{path::Path, process::Output};

use color_eyre::eyre::{self, Context};
use reqwest::Url;
use serde::{Deserialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::{
    lockfile::{Dependency, Lockfile, Src, UniversalSrc},
    manifest::{Artifact, ExternalSpec, PackageManifest, PackageType},
    registry,
};

pub struct Resolver {
    client: reqwest::Client,
    registry_url: Url,
    cache: Option<Lockfile>,
}

impl Resolver {
    pub fn new(cache: Option<Lockfile>) -> Self {
        Self {
            client: reqwest::Client::new(),
            registry_url: Url::parse("https://api.registry.platformio.org")
                .expect("valid default registry"),
            cache,
        }
    }

    pub async fn resolve(&self, artifact: Artifact) -> eyre::Result<Dependency> {
        if let Some(dependency) = self.try_resolve_cached(&artifact).await {
            return Ok(dependency);
        }
        self.resolve_artifact(artifact).await
    }

    /// Try to reuse a cached `Dependency` from the existing lockfile.
    async fn try_resolve_cached(&self, artifact: &Artifact) -> Option<Dependency> {
        let Lockfile::V2 { dependencies } = self.cache.as_ref()?;
        let install_path = artifact.install_path.to_string_lossy();
        let cached = dependencies.get(install_path.as_ref())?;

        if cached.manifest != artifact.manifest {
            log::info!("Lockfile entry for {install_path} has a stale manifest, re-resolving");
            return None;
        }

        // For git packages also verify the current checkout rev matches what's locked.
        if let Src::Universal(UniversalSrc::Git(git)) = &cached.src {
            match self.get_rev(&artifact.full_path).await {
                Ok(rev) if rev == git.rev => {}
                Ok(rev) => {
                    log::info!(
                        "Lockfile entry for {install_path} has stale rev \
                     (locked: {}, current: {rev}), re-resolving",
                        git.rev
                    );
                    return None;
                }
                Err(e) => {
                    log::info!("Could not determine git rev for {install_path}: {e}, re-resolving");
                    return None;
                }
            }
        }

        log::info!("Reusing hashes for {install_path} (pass --disable-cache to bypass)");
        Some(cached.clone())
    }

    async fn resolve_artifact(&self, artifact: Artifact) -> eyre::Result<Dependency> {
        let manifest = &artifact.manifest;
        let name = manifest.spec.name();
        log::info!("Resolving {name}...");
        match &manifest.spec {
            crate::manifest::PackageSpec::PlatformIO(spec) => {
                let package_spec = self
                    .resolve_registry(&spec.owner, manifest.ty, name, manifest.version.to_string())
                    .await?;
                Ok(Dependency::from_registry(manifest.clone(), package_spec))
            }
            crate::manifest::PackageSpec::External(spec) => {
                match spec.uri.as_str().strip_prefix("git+") {
                    Some(_) => self.resolve_git(&artifact).await,
                    None => self.resolve_url(manifest, spec).await,
                }
            }
        }
    }

    async fn resolve_registry(
        &self,
        owner: &str,
        ty: PackageType,
        name: &str,
        version: String,
    ) -> eyre::Result<registry::PackageSpec> {
        let mut url = self.registry_url.clone();
        url.path_segments_mut()
            .expect("base path")
            .push("v3")
            .push("packages")
            .push(owner)
            .push(ty.as_str())
            .push(name);
        url.query_pairs_mut().append_pair("version", &version);
        log::info!("Fetching package spec: {}", url);
        let response = self.client.get(url).send().await?;
        extract_json(response).await
    }

    async fn resolve_url(
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

    async fn resolve_git(&self, artifact: &Artifact) -> eyre::Result<Dependency> {
        let name = artifact.manifest.spec.name();
        let repo_path = &artifact.full_path;

        let remote = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["remote", "get-url", "origin"])
            .output_success()
            .await
            .context("running git remote get-url")?;

        let base_url = Url::parse(String::from_utf8(remote.stdout)?.trim())?;

        let rev = self
            .get_rev(repo_path)
            .await
            .context("running git rev-parse")?;

        let file_url = format!("file://{}", repo_path.display());
        let prefetch = Command::new("nix-prefetch-git")
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
            artifact.manifest.clone(),
            name.to_string(),
            base_url,
            rev,
            hash,
        ))
    }

    async fn get_rev(&self, repo_path: &Path) -> eyre::Result<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .args(["rev-parse", "HEAD"])
            .output_success()
            .await
            .context("running git rev-parse")?;

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
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
