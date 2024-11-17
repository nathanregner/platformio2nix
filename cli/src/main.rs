mod lockfile;
mod manifest;
mod registry;

use clap::Parser;
use color_eyre::eyre::{self};
use lockfile::Lockfile;
use manifest::extract_artifacts;
use registry::RegistryClient;
use serde::Deserialize;
use std::{
    env::{self},
    path::PathBuf,
};

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
}

impl Args {
    fn core_dir(&self) -> eyre::Result<PathBuf> {
        if let Some(core_dir) = &self.core_dir {
            return Ok(core_dir.to_owned());
        }

        if let Some(core_dir) = env::var("PLATFORMIO_CORE_DIR").ok() {
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

        if let Some(workspace_dir) = env::var("PLATFORMIO_WORKSPACE_DIR").ok() {
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
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Repository {
    Git { url: String },
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Warn)
        .parse_default_env()
        .init();

    let args = Args::parse();
    let client = RegistryClient::default();

    let global = extract_artifacts(&args.core_dir()?)?;
    let workspace = if let Some(workspace_dir) = args.workspace_dir()? {
        extract_artifacts(&workspace_dir)?
    } else {
        vec![]
    };

    let mut lockfile = Lockfile::default();

    for artifact in global.into_iter().chain(workspace.into_iter()) {
        let dependency = client.resolve(artifact).await?;
        lockfile.add_dependency(dependency);
    }

    println!("{}", serde_json::to_string_pretty(&lockfile)?);

    Ok(())
}
