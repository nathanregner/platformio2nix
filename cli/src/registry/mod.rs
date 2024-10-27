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
    manifest::{ExternalSpec, Manifest, PackageType, PlatformIOSpec},
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
    pub async fn resolve(&self, manifest: &Manifest) -> eyre::Result<Dependency> {
        match &manifest.spec {
            crate::manifest::PackageSpec::PlatformIO(PlatformIOSpec { owner, name, .. }) => {
                let package_spec = self
                    .get_package_spec(owner, manifest.ty, name, Some(manifest.version.to_string()))
                    .await?;
                Ok(Dependency::from_registry(&manifest, &package_spec))
            }
            crate::manifest::PackageSpec::External(package_spec) => {
                self.get_external(&manifest, package_spec).await
            }
        }
    }

    pub async fn get_external(
        &self,
        manifest: &Manifest,
        package_spec: &ExternalSpec,
    ) -> eyre::Result<Dependency> {
        let response = self.client.get(package_spec.uri.clone()).send().await?;
        let response = response.error_for_status()?;
        let bytes = response.bytes().await?;
        let mut hash = Sha256::new();
        hash.update(bytes);
        let hash = hash.finalize();
        Ok(Dependency::from_url(manifest, package_spec, &hash))
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
        // TODO: remove
        eprintln!("url: {}", url);
        let response = self.client.get(url).send().await?;
        extract_json(response).await
    }

    pub async fn search(&self, params: SearchParams<'_>) -> eyre::Result<SearchResults> {
        let mut url = self.registry_url.clone();
        url.path_segments_mut()
            .expect("base path")
            .push("v3")
            .push("search");
        url.query_pairs_mut()
            .append_pair("query", &params.to_query());
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

pub struct SearchParams<'s> {
    pub names: &'s [&'s str],
}

impl<'s> SearchParams<'s> {
    pub fn to_query(&self) -> String {
        let mut query = String::new();
        for name in self.names {
            if !query.is_empty() {
                query.push(' ');
            }
            query.push_str("name:");
            query.push_str(&serde_json::to_string(name).expect("name is serializable"));
        }
        query
    }
}

#[derive(Deserialize, Debug)]
pub struct SearchResults {
    pub items: Vec<PackageMeta>,
}

#[derive(Deserialize, Debug)]
pub struct PackageMeta {
    pub owner: PackageMetaOwner,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(rename = "version")]
    pub latest_version: VersionSpec,
}

#[derive(Deserialize, Debug)]
pub struct PackageMetaOwner {
    pub username: String,
}

#[derive(Deserialize, Debug)]
pub struct PackageSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub version: VersionSpec,
    pub versions: Vec<VersionSpec>,
}

// impl PackageSpec {
//     pub fn pick_latest_compatible(
//         &self,
//         version: VersionReq,
//         system: &System,
//     ) -> Option<&VersionSpec> {
//         self.versions
//             .iter()
//             .filter(|v| version.matches(&v.name) && v.supports(system).is_some())
//             .max_by(|a, b| a.name.cmp(&b.name))
//     }
// }

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

    // #[tokio::test]
    // async fn search() {
    //     let client = RegistryClient::default();
    //     let spec = client
    //         .search(SearchParams {
    //             names: &["tool-scons"],
    //         })
    //         .await
    //         .unwrap();
    //     assert!(!spec.items.is_empty());
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
    fn deserialize_search_result() {
        let json = include_str!("./test/search.json");
        let de = &mut serde_json::Deserializer::from_str(json);
        let spec = serde_path_to_error::deserialize::<_, SearchResults>(de);
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

    // TODO
    /* #[test]
    fn pick_latest_compatible() {
        let systems = SystemSpec::Systems(vec![System::LinuxX86_64]);
        let latest = VersionSpec {
            name: "2.0.0".parse().unwrap(),
            files: vec![File {
                system: systems.clone(),
                download_url: Url::parse("https://example.com").unwrap(),
                checksum: Checksum {
                    sha256: "deadbeef".to_string(),
                },
            }],
        };
        let spec = PackageSpec {
            name: "test".to_string(),
            ty: "platform".to_string(),
            version: latest.clone(),
            versions: vec![
                VersionSpec {
                    name: "1.0.0".parse().unwrap(),
                    files: vec![File {
                        system: systems.clone(),
                        download_url: Url::parse("https://example.com").unwrap(),
                        checksum: Checksum {
                            sha256: "deadbeef".to_string(),
                        },
                    }],
                },
                VersionSpec {
                    name: "1.1.0".parse().unwrap(),
                    files: vec![File {
                        system: systems.clone(),
                        download_url: Url::parse("https://example.com").unwrap(),
                        checksum: Checksum {
                            sha256: "deadbeef".to_string(),
                        },
                    }],
                },
                latest,
            ],
        };

        let version = spec.pick_latest_compatible("~1".parse().unwrap(), &System::LinuxX86_64);
        assert_eq!(version.map(|v| &v.name), Some(&"1.1.0".parse().unwrap()));
    } */

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
