mod lockfile;
mod manifest;
mod registry;
mod resolver;

use std::collections::HashSet;

use clap::Parser;
use color_eyre::eyre::{self, Context};
use lockfile::Lockfile;
use serde::Deserialize;
use std::{
    env::{self},
    path::{Path, PathBuf},
};

use crate::{manifest::extract_artifacts, resolver::Resolver};

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
    /// Lockfile to read from/write to.
    ///
    /// Default: `platformio2nix.lock`
    #[arg(short, long)]
    lockfile: Option<PathBuf>,
    /// Force resolution of hashes, even if a dependency already exists in the lockfile
    #[arg(short, long, default_value = "false")]
    disable_cache: bool,
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

#[tokio::main]
async fn main() -> eyre::Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    let args = Args::parse();

    let lockfile_path = args.lockfile_path();
    let existing_lockfile = if args.disable_cache {
        None
    } else {
        load_existing_lockfile(&lockfile_path)
    };

    let resolver = Resolver::new(existing_lockfile);

    let workspace = if let Some(workspace_dir) = args.workspace_dir()? {
        extract_artifacts(&workspace_dir)?
    } else {
        HashSet::default()
    };
    let global = {
        let mut global = extract_artifacts(&args.core_dir()?)?;
        global.retain(|package| !workspace.contains(package));
        global
    };

    let mut lockfile = Lockfile::default();

    log::info!(
        "Locking {} workspace dependencies and {} global dependencies...",
        workspace.len(),
        global.len(),
    );
    for artifact in global.into_iter().chain(workspace.into_iter()) {
        let install_path = artifact.install_path.to_string_lossy().into_owned();
        let name = artifact.manifest.spec.name().to_string();
        let dependency = resolver
            .resolve(artifact)
            .await
            .with_context(|| format!("resolving {name}"))?;
        lockfile.add_dependency(install_path, dependency);
    }

    lockfile
        .write_to(&lockfile_path)
        .with_context(|| format!("writing {lockfile_path:?}"))?;

    Ok(())
}
