mod lockfile;
mod manifest;
mod registry;

use std::ffi::OsStr;

use clap::Parser;
use color_eyre::eyre::{self, Context};
use lockfile::{Dependency, Lockfile, Src, UniversalSrc};
use manifest::{extract_artifacts, Artifact};
use registry::RegistryClient;
use serde::Deserialize;
use std::{
    env::{self},
    path::{Path, PathBuf},
};
use tokio::process::Command;

/// Generate a platformio2nix lockfile to stdout
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directory containing toolchains and global libraries.
    ///
    /// Default: $PLATFORMIO_CORE_DIR, ~/.platformio
    ///
    /// https://docs.platformio.org/en/latest/projectconf/sections/platformio/options/directory/core_dir.html
    #[arg(short, long)]
    core_dir: Option<PathBuf>,
    /// Directory containing compiled objects, static libraries, firmware, and external library dependencies.
    ///
    /// Default: $PLATFORMIO_WORKSPACE_DIR, ./.pio
    ///
    /// https://docs.platformio.org/en/latest/projectconf/sections/platformio/options/directory/workspace_dir.html
    #[arg(short, long)]
    workspace_dir: Option<PathBuf>,
    /// Existing lockfile to use as a hash cache.
    ///
    /// Default: platformio2nix.lock in the current directory (if it exists)
    #[arg(short, long)]
    lockfile: Option<PathBuf>,
}

impl Args {
    fn core_dir(&self) -> eyre::Result<PathBuf> {
        if let Some(core_dir) = &self.core_dir {
            return Ok(core_dir.to_owned());
        }

        if let Ok(core_dir) = env::var("PLATFORMIO_CORE_DIR") {
            return Ok(PathBuf::from(core_dir));
        }

        if let Some(home_dir) = env::home_dir() {
            return Ok(home_dir.join(".platformio"));
        }

        eyre::bail!("Failed to detect core_dir, consider passing --core-dir")
    }

    fn workspace_dir(&self) -> eyre::Result<Option<PathBuf>> {
        if let Some(workspace_dir) = self.workspace_dir.as_deref() {
            return Ok(Some(workspace_dir.to_owned()));
        }

        if let Ok(workspace_dir) = env::var("PLATFORMIO_WORKSPACE_DIR") {
            return Ok(Some(PathBuf::from(workspace_dir)));
        }

        let pwd = env::current_dir()?;
        let mut pwd = Some(&*pwd);
        while let Some(dir) = pwd {
            let workspace_dir = dir.join(".pio");
            if workspace_dir.is_dir() {
                return Ok(Some(workspace_dir.to_owned()));
            }
            pwd = dir.parent();
        }

        Ok(None)
    }

    fn lockfile_path(&self) -> PathBuf {
        self.lockfile
            .clone()
            .unwrap_or_else(|| PathBuf::from("platformio2nix.lock"))
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Repository {
    Git { url: String },
}

fn load_existing_lockfile(path: &Path) -> Option<Lockfile> {
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(path) {
        Err(e) => {
            log::warn!("Failed to read lockfile {}: {e}", path.display());
            None
        }
        Ok(json) => match serde_json::from_str(&json) {
            Ok(lockfile) => Some(lockfile),
            Err(e) => {
                log::warn!(
                    "Lockfile {} is invalid, ignoring cache: {e}",
                    path.display()
                );
                None
            }
        },
    }
}

/// Returns the current HEAD rev of the git repo containing `piopm_path`.
async fn get_current_git_rev(piopm_path: &Path) -> eyre::Result<String> {
    let repo_path = if piopm_path.file_name() == Some(OsStr::new(".git")) {
        piopm_path.parent().unwrap_or(piopm_path)
    } else {
        piopm_path
    };
    let output = Command::new("git")
        .args(["-C", &repo_path.to_string_lossy(), "rev-parse", "HEAD"])
        .output()
        .await?;
    if !output.status.success() {
        eyre::bail!(
            "git rev-parse HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

/// Try to reuse a cached `Dependency` from the existing lockfile.
/// Returns `None` if there is no cache, the entry is missing, or the entry is stale.
async fn try_cached(cache: &Option<Lockfile>, artifact: &Artifact) -> Option<Dependency> {
    let Lockfile::V2 { dependencies } = cache.as_ref()?;
    let install_path = artifact.install_path.to_string_lossy();
    let cached = dependencies.get(install_path.as_ref())?;

    if cached.manifest != artifact.manifest {
        log::info!("Lockfile entry for {install_path} has a stale manifest, re-resolving");
        return None;
    }

    // For git packages also verify the current checkout rev matches what's locked.
    if let Src::Universal(UniversalSrc::Git(git)) = &cached.src {
        match get_current_git_rev(&artifact.full_path).await {
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

    log::debug!("Using cached entry for {install_path}");
    Some(cached.clone())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let args = Args::parse();
    let client = RegistryClient::default();

    let existing = load_existing_lockfile(&args.lockfile_path());

    let global = extract_artifacts(&args.core_dir()?)?;
    let workspace = if let Some(workspace_dir) = args.workspace_dir()? {
        extract_artifacts(&workspace_dir)?
    } else {
        vec![]
    };

    let mut lockfile = Lockfile::default();

    for artifact in global.into_iter().chain(workspace.into_iter()) {
        let install_path = artifact.install_path.to_string_lossy().into_owned();
        let name = artifact.manifest.spec.name().to_string();
        let dependency = if let Some(dep) = try_cached(&existing, &artifact).await {
            dep
        } else {
            client
                .resolve(artifact)
                .await
                .with_context(|| format!("resolving {name}"))?
        };
        lockfile.add_dependency(install_path, dependency);
    }

    println!("{}", serde_json::to_string_pretty(&lockfile)?);

    Ok(())
}
