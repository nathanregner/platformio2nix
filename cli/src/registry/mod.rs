use reqwest::Url;
use serde::{Deserialize, de::Visitor};

#[derive(Deserialize, Debug)]
pub struct PackageSpec {
    pub name: String,
    pub version: VersionSpec,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VersionSpec {
    pub files: Vec<File>,
}

impl VersionSpec {
    pub fn supports(&self, system: &System) -> Option<&File> {
        self.files.iter().find(|f| f.system.supports(system))
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
            SystemSpec::Systems(systems) => systems.contains(system),
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
