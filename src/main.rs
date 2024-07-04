mod lockfile;
mod manifest;
mod registry;

use std::path::PathBuf;

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
    core_dir: PathBuf,
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

    let platforms = extract_manifests(&args.core_dir.join("platforms"))?;
    // let platform_packages = platforms
    //     .iter()
    //     .flat_map(|platform| {
    //         platform
    //             .packages
    //             .iter()
    //             .map(|(name, package)| (name, package))
    //     })
    //     .collect::<HashMap<_, _>>();
    let packages = extract_manifests(&args.core_dir.join("packages"))?;

    let mut deps = vec![];

    for manifest in &platforms {
        let package_spec = client.get_manifest(&manifest).await?;
        deps.push(Dependency::new(manifest, &package_spec.version));
    }

    for manifest in &packages {
        // TODO: resolve from platform.packages if available?
        // let results = client
        //     .search(registry::SearchParams {
        //         names: &[&package.name],
        //     })
        //     .await?;
        // if results.items.len() > 1 {
        //     eyre::bail!(
        //         "Multiple mathches for package {}:\n{:?}",
        //         package.name,
        //         results.items
        //     );
        // }
        // let Some(package_meta) = results.items.first() else {
        //     return Err(eyre::eyre!("package not found: {}", package.name));
        // };
        let package_spec = client.get_manifest(&manifest).await?;
        deps.push(Dependency::new(manifest, &package_spec.version));
    }

    let lockfile = Lockfile::new(deps);
    println!("{}", serde_json::to_string_pretty(&lockfile)?);
    Ok(())
}
