mod registry;

use std::path::{Path, PathBuf};

use clap::Parser;
use color_eyre::eyre;
use semver::VersionReq;
use serde::Deserialize;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// PlatformIO [core_dir](https://docs.platformio.org/en/latest/projectconf/sections/platformio/options/directory/core_dir.html)
    /// containing toolchains and global libraries.
    #[arg(short, long)]
    core_dir: PathBuf,
}

// https://docs.platformio.org/en/latest/platforms/creating_platform.html#platform-creating-manifest-file

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub repository: Option<Repository>,
    #[serde(default)]
    pub packages: Vec<Package>,
}

#[derive(Deserialize, Debug)]
pub struct Package {
    pub owner: String,
    pub version: VersionReq,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Repository {
    Git { url: String },
}

fn main() -> eyre::Result<()> {
    let args = Args::parse();
    let platforms = extract_manifests(&args.core_dir.join("platforms"), "platform.json")?;
    let packages = extract_manifests(&args.core_dir.join("packages"), "package.json")?;
    dbg!(platforms);
    dbg!(packages);
    Ok(())
}

fn extract_manifests(dir: &Path, manifest_file: &str) -> Result<Vec<Manifest>, eyre::Error> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut manifests = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let manifest_path = entry.path().join(manifest_file);
        if !manifest_path.exists() {
            continue;
        }

        let manifest = std::fs::read_to_string(manifest_path)?;
        manifests.push(serde_json::from_str::<Manifest>(&manifest)?)
    }

    Ok(manifests)
}
