mod lockfile;
mod manifest;
mod registry;

use std::{
    env::{self},
    path::PathBuf,
};

use clap::Parser;
use color_eyre::eyre::{self};
use lockfile::{Dependency, Lockfile};
use manifest::extract_manifests;
use registry::RegistryClient;
use serde::Deserialize;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// PlatformIO [core_dir](https://docs.platformio.org/en/latest/projectconf/sections/platformio/options/directory/core_dir.html)
    /// containing toolchains and global libraries.
    #[arg(short, long)]
    core_dir: Option<PathBuf>,
    #[arg(short, long)]
    workspace_dir: Option<PathBuf>,
}

impl Args {
    // https://docs.platformio.org/en/latest/projectconf/sections/platformio/options/directory/core_dir.html
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

    // https://docs.platformio.org/en/latest/projectconf/sections/platformio/options/directory/workspace_dir.html
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
    let args = Args::parse();
    let client = RegistryClient::default();

    let global = extract_manifests(&args.core_dir()?)?;
    let workspace = if let Some(workspace_dir) = args.workspace_dir()? {
        extract_manifests(&workspace_dir)?
    } else {
        vec![]
    };

    let mut lockfile = Lockfile::default();

    for manifest in global.into_iter().chain(workspace.into_iter()) {
        let dependency = client.resolve(&manifest).await?;
        lockfile.insert(dependency);
    }

    println!("{}", serde_json::to_string_pretty(&lockfile)?);

    Ok(())
}
