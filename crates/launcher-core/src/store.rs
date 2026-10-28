use std::fs;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;

use crate::{Error, Host, Instance, InstanceConfig, InstanceId, NewInstance};

const INSTANCES_DIR: &str = "instances";
const TEMP_SUFFIX: &str = ".tmp";

#[derive(Debug)]
pub struct ListFailure {
    pub dir: PathBuf,
    pub error: Error,
}

#[derive(Debug, Default)]
pub struct Listing {
    pub instances: Vec<Instance>,
    pub failures: Vec<ListFailure>,
}

#[derive(Debug, Clone)]
pub struct InstanceStore {
    instances_dir: PathBuf,
}

impl InstanceStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            instances_dir: root.as_ref().join(INSTANCES_DIR),
        }
    }

    pub fn create(&self, new: NewInstance, host: Host) -> Result<Instance, Error> {
        if !new.kind.supported_on(host) {
            return Err(Error::UnsupportedKind {
                kind: new.kind,
                host,
            });
        }
        let config = InstanceConfig {
            id: InstanceId::generate(),
            name: new.name,
            kind: new.kind,
            edition: new.edition,
            app_version: new.app_version,
            group: new.group,
            created_at: OffsetDateTime::now_utc(),
            plugins: Vec::new(),
        };
        config.validate()?;
        let instance = Instance {
            dir: self.dir_of(config.id),
            config,
        };
        let app_dir = instance.app_dir();
        fs::create_dir_all(&app_dir).map_err(|e| Error::io(&app_dir, e))?;
        if let Err(error) = self.save(&instance) {
            let _ = fs::remove_dir_all(&instance.dir);
            return Err(error);
        }
        Ok(instance)
    }

    pub fn load(&self, id: InstanceId) -> Result<Instance, Error> {
        let dir = self.dir_of(id);
        if !dir.is_dir() {
            return Err(Error::NotFound(id));
        }
        Self::load_dir(dir, id)
    }

    pub fn save(&self, instance: &Instance) -> Result<(), Error> {
        instance.config.validate()?;
        let text = toml::to_string_pretty(&instance.config)?;
        let path = instance.config_path();
        let temp = path.with_extension(format!("toml{TEMP_SUFFIX}"));
        fs::write(&temp, text).map_err(|e| Error::io(&temp, e))?;
        fs::rename(&temp, &path).map_err(|e| Error::io(&path, e))
    }

    pub fn delete(&self, id: InstanceId) -> Result<(), Error> {
        let dir = self.dir_of(id);
        if !dir.is_dir() {
            return Err(Error::NotFound(id));
        }
        fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))
    }

    pub fn list(&self) -> Result<Listing, Error> {
        let mut listing = Listing::default();
        let entries = match fs::read_dir(&self.instances_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(listing),
            Err(e) => return Err(Error::io(&self.instances_dir, e)),
        };
        for entry in entries {
            let entry = entry.map_err(|e| Error::io(&self.instances_dir, e))?;
            let dir = entry.path();
            let Some(id) = dir
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.parse::<InstanceId>().ok())
            else {
                continue;
            };
            match Self::load_dir(dir.clone(), id) {
                Ok(instance) => listing.instances.push(instance),
                Err(error) => listing.failures.push(ListFailure { dir, error }),
            }
        }
        listing.instances.sort_by(|a, b| {
            (a.config.created_at, a.config.id.to_string())
                .cmp(&(b.config.created_at, b.config.id.to_string()))
        });
        Ok(listing)
    }

    fn dir_of(&self, id: InstanceId) -> PathBuf {
        self.instances_dir.join(id.to_string())
    }

    fn load_dir(dir: PathBuf, id: InstanceId) -> Result<Instance, Error> {
        let path = dir.join(Instance::CONFIG_FILE);
        let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let config: InstanceConfig = toml::from_str(&text).map_err(|source| Error::Parse {
            path: path.clone(),
            source,
        })?;
        if config.id != id {
            return Err(Error::IdMismatch {
                dir: id,
                file: config.id,
            });
        }
        config.validate()?;
        Ok(Instance { dir, config })
    }
}
