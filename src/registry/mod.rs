use color_eyre::eyre;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest::{Client, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use semver::VersionReq;
use serde::Deserialize;

pub struct RegistryClient {
    client: ClientWithMiddleware,
    registry_url: Url,
}

impl Default for RegistryClient {
    fn default() -> Self {
        let client = ClientBuilder::new(Client::new())
            .with(Cache(HttpCache {
                mode: CacheMode::ForceCache,
                manager: CACacheManager::default(),
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
    pub async fn search(
        &self,
        owner: &str,
        ty: &str,
        name: &str,
        version: Option<String>,
    ) -> eyre::Result<PackageSpec> {
        let mut url = self.registry_url.clone();
        url.path_segments_mut()
            .expect("base path")
            .push("v3")
            .push("packages")
            .push(owner)
            .push(ty)
            .push(name);
        if let Some(version) = version {
            url.query_pairs_mut().append_pair("version", &version);
        }
        println!("url: {}", url);
        let response = self.client.get(url).send().await?;
        let response = response.error_for_status()?;
        let json = response.text().await?;
        let de = &mut serde_json::Deserializer::from_str(&json);
        let spec = serde_path_to_error::deserialize::<_, PackageSpec>(de)?;
        Ok(spec)
    }
}

#[derive(Deserialize, Debug)]
pub struct PackageSpec {
    version: Version,
    versions: Vec<Version>,
}

impl PackageSpec {
    pub fn pick_latest_compatible(&self, version: VersionReq, system: System) -> Option<&Version> {
        self.versions
            .iter()
            .filter(|v| {
                version.matches(&v.name) && v.files.iter().any(|f| f.system.contains(&system))
            })
            .max_by(|a, b| a.name.cmp(&b.name))
    }
}

#[derive(Deserialize, Debug)]
pub struct Version {
    name: semver::Version,
    files: Vec<File>,
}

#[derive(Deserialize, Debug)]
pub struct File {
    system: Vec<System>,
    download_url: Url,
    checksum: Checksum,
}

#[derive(Deserialize, Debug)]
pub struct Checksum {
    sha256: String,
}

#[derive(Deserialize, Hash, Eq, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum System {
    DarwinX86_64,
    DarwinArm64,
    LinuxX86_64,
    LinuxAarch64,
    // #[serde(rename = "linux_armv6l")]
    // LinuxArmv6l,
    // #[serde(rename = "linux_armv7l")]
    // LinuxArmv7l,
    // #[serde(rename = "linux_armv8l")]
    // LinuxArmv8l,
    LinuxI686,
    #[serde(untagged)]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn search() {
    //     let client = RegistryClient::default();
    //     let spec = client
    //         .search(
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
    fn deserialize_platform_atmelavr() {
        let json = include_str!("./test/atmelavr.json");
        let de = &mut serde_json::Deserializer::from_str(json);
        let spec = serde_path_to_error::deserialize::<_, PackageSpec>(de);
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
    fn deserialize_toolchain_atmelavr() {
        let json = include_str!("./test/toolchain-atmelavr.json");
        let de = &mut serde_json::Deserializer::from_str(json);
        let spec = serde_path_to_error::deserialize::<_, PackageSpec>(de);
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
    fn pick_latest_compatible() {
        let spec = PackageSpec {
            version: Version {
                name: "1.0.0".parse().unwrap(),
                files: vec![File {
                    system: vec![System::LinuxX86_64],
                    download_url: Url::parse("https://example.com").unwrap(),
                    checksum: Checksum {
                        sha256: "deadbeef".to_string(),
                    },
                }],
            },
            versions: vec![
                Version {
                    name: "1.0.0".parse().unwrap(),
                    files: vec![File {
                        system: vec![System::LinuxX86_64],
                        download_url: Url::parse("https://example.com").unwrap(),
                        checksum: Checksum {
                            sha256: "deadbeef".to_string(),
                        },
                    }],
                },
                Version {
                    name: "1.1.0".parse().unwrap(),
                    files: vec![File {
                        system: vec![System::LinuxX86_64],
                        download_url: Url::parse("https://example.com").unwrap(),
                        checksum: Checksum {
                            sha256: "deadbeef".to_string(),
                        },
                    }],
                },
                Version {
                    name: "2.0.0".parse().unwrap(),
                    files: vec![File {
                        system: vec![System::LinuxX86_64],
                        download_url: Url::parse("https://example.com").unwrap(),
                        checksum: Checksum {
                            sha256: "deadbeef".to_string(),
                        },
                    }],
                },
            ],
        };

        let version = spec.pick_latest_compatible("~1".parse().unwrap(), System::LinuxX86_64);
        assert_eq!(version.map(|v| &v.name), Some(&"1.1.0".parse().unwrap()));
    }
}
