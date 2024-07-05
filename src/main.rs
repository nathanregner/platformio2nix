mod output;
mod registry;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use clap::Parser;
use color_eyre::eyre::{self, Context};
use output::{Dependency, DependencyType, Lockfile};
use registry::RegistryClient;
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

/// https://docs.platformio.org/en/latest/platforms/creating_platform.html#platform-creating-manifest-file
#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub name: String,
    pub version: semver::Version,
    pub repository: Option<Repository>,
    #[serde(default)]
    pub packages: HashMap<String, Package>,
}

#[derive(Deserialize, Debug)]
pub struct Package {
    #[serde(rename = "type")]
    pub ty: Option<String>,
    pub owner: String,
    pub version: VersionReq,
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

    let platforms = extract_manifests(&args.core_dir.join("platforms"), "platform.json")?;
    // let platform_packages = platforms
    //     .iter()
    //     .flat_map(|platform| {
    //         platform
    //             .packages
    //             .iter()
    //             .map(|(name, package)| (name, package))
    //     })
    //     .collect::<HashMap<_, _>>();
    let packages = extract_manifests(&args.core_dir.join("packages"), "package.json")?;

    let mut deps = vec![];

    for platform in &platforms {
        let package_spec = client
            .get(
                "platformio",
                "platform",
                &platform.name,
                Some(platform.version.to_string()),
            )
            .await?;
        deps.push(Dependency::new(
            package_spec.name.clone(),
            DependencyType::Platform,
            &package_spec.version,
        ));
    }

    for package in &packages {
        // TODO: resolve from platform.packages if available?
        let results = client
            .search(registry::SearchParams {
                names: &[&package.name],
            })
            .await?;
        if results.items.len() > 1 {
            eyre::bail!(
                "Multiple mathches for package {}:\n{:?}",
                package.name,
                results.items
            );
        }
        let Some(package_meta) = results.items.first() else {
            return Err(eyre::eyre!("package not found: {}", package.name));
        };
        let package_spec = client
            .get(
                &package_meta.owner.username,
                &package_meta.ty,
                &package.name,
                Some(package.version.to_string()),
            )
            .await?;
        deps.push(Dependency::new(
            package_spec.name.clone(),
            DependencyType::Package,
            &package_spec.version,
        ));
    }

    let lockfile = Lockfile::new(deps);
    println!("{}", serde_json::to_string_pretty(&lockfile)?);
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

        let path = entry.path().join(manifest_file);
        if !path.exists() {
            continue;
        }

        let json = std::fs::read_to_string(&path)?;
        let de = &mut serde_json::Deserializer::from_str(&json);
        let manifest = serde_path_to_error::deserialize::<_, Manifest>(de).wrap_err_with(|| {
            format!("failed to parse manifest file: {}", path.to_string_lossy())
        })?;
        manifests.push(manifest);
    }

    Ok(manifests)
}
