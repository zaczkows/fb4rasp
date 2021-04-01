use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(rename(serialize = "remote", deserialize = "remote"))]
    pub remotes: BTreeMap<String, Remote>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            remotes: BTreeMap::new(),
        }
    }
}

const fn truer() -> bool {
    true
}

#[derive(Deserialize, Debug)]
pub struct Remote {
    pub ip: String,
    #[serde(default = "truer")]
    pub enable: bool,
}

pub fn read_toml_config<P: AsRef<Path>>(path: P) -> Option<Config> {
    fn inner(path: &Path) -> Option<Config> {
        let config = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to read config {:?} file: {}", &path, &e);
                return None;
            }
        };

        let t: Result<Config, _> = toml::from_str(config.as_str());
        match t {
            Ok(content) => Some(content),
            Err(e) => {
                log::error!("Failed to parse config {:?} file: {}", &path, &e);
                None
            }
        }
    }

    inner(path.as_ref())
}
