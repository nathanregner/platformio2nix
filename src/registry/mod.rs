use color_eyre::eyre;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest::{Client, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use semver::VersionReq;
use serde::{de::Visitor, Deserialize};

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
                version.matches(&v.name) && v.files.iter().any(|f| f.system.supports(&system))
            })
            .max_by(|a, b| a.name.cmp(&b.name))
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct Version {
    name: semver::Version,
    files: Vec<File>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct File {
    system: SystemSpec,
    download_url: Url,
    checksum: Checksum,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Checksum {
    sha256: String,
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum SystemSpec {
    Wildcard,
    Systems(Vec<System>),
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

impl SystemSpec {
    pub fn supports(&self, system: &System) -> bool {
        match self {
            SystemSpec::Wildcard => true,
            SystemSpec::Systems(systems) => systems.contains(&system),
        }
    }
}

#[derive(Deserialize, Hash, Eq, PartialEq, Clone, Debug)]
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
        let systems = SystemSpec::Systems(vec![System::LinuxX86_64]);
        let latest = Version {
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
            version: latest.clone(),
            versions: vec![
                Version {
                    name: "1.0.0".parse().unwrap(),
                    files: vec![File {
                        system: systems.clone(),
                        download_url: Url::parse("https://example.com").unwrap(),
                        checksum: Checksum {
                            sha256: "deadbeef".to_string(),
                        },
                    }],
                },
                Version {
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

        let version = spec.pick_latest_compatible("~1".parse().unwrap(), System::LinuxX86_64);
        assert_eq!(version.map(|v| &v.name), Some(&"1.1.0".parse().unwrap()));
    }
}
