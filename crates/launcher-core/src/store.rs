use std::fs;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use tracing::info;

use crate::{Error, Host, Instance, InstanceConfig, InstanceId, NewInstance, PluginRecord};

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

    /// # Errors
    ///
    /// Returns `Error::UnsupportedKind` when `host` cannot run `new.kind`, the errors of
    /// `InstanceConfig::validate`, and `Error::Io` or `Error::Serialize` when the instance
    /// directory or `instance.toml` cannot be written.
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
            addon_version: None,
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
        info!(
            id = %instance.config.id,
            kind = ?instance.config.kind,
            dir = %instance.dir.display(),
            "instance created"
        );
        Ok(instance)
    }

    /// # Errors
    ///
    /// Returns `Error::NotFound` when the instance directory does not exist,
    /// `Error::Io` or `Error::Parse` when `instance.toml` cannot be read,
    /// `Error::IdMismatch` when its id differs from the directory name,
    /// and the errors of `InstanceConfig::validate`.
    pub fn load(&self, id: InstanceId) -> Result<Instance, Error> {
        let dir = self.dir_of(id);
        if !dir.is_dir() {
            return Err(Error::NotFound(id));
        }
        Self::load_dir(dir, id)
    }

    /// # Errors
    ///
    /// Returns the errors of `InstanceConfig::validate`, `Error::Serialize`,
    /// and `Error::Io` when `instance.toml` cannot be written.
    pub fn save(&self, instance: &Instance) -> Result<(), Error> {
        instance.config.validate()?;
        let text = toml::to_string_pretty(&instance.config)?;
        let path = instance.config_path();
        let temp = path.with_extension(format!("toml{TEMP_SUFFIX}"));
        fs::write(&temp, text).map_err(|e| Error::io(&temp, e))?;
        fs::rename(&temp, &path).map_err(|e| Error::io(&path, e))
    }

    /// Changes the name and the group. The group is removed when `group` is `None`.
    ///
    /// # Errors
    ///
    /// Returns the errors of `load` and `save`.
    pub fn update(
        &self,
        id: InstanceId,
        name: String,
        group: Option<String>,
    ) -> Result<Instance, Error> {
        let mut instance = self.load(id)?;
        instance.config.name = name;
        instance.config.group = group;
        self.save(&instance)?;
        Ok(instance)
    }

    /// Writes the versions of the app and of the addon.
    ///
    /// # Errors
    ///
    /// Returns the errors of `load` and `save`.
    pub fn set_versions(
        &self,
        id: InstanceId,
        app_version: String,
        addon_version: Option<String>,
    ) -> Result<Instance, Error> {
        let mut instance = self.load(id)?;
        instance.config.app_version = app_version;
        instance.config.addon_version = addon_version;
        self.save(&instance)?;
        Ok(instance)
    }

    /// Sets the paused state of a plugin.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` when the plugin is not installed, and the errors of `load` and `save`.
    pub fn set_plugin_paused(
        &self,
        id: InstanceId,
        plugin_id: &str,
        paused: bool,
    ) -> Result<Instance, Error> {
        let mut instance = self.load(id)?;
        let Some(record) = instance
            .config
            .plugins
            .iter_mut()
            .find(|record| record.id == plugin_id)
        else {
            return Err(Error::NotFound(id));
        };
        record.paused = paused;
        self.save(&instance)?;
        Ok(instance)
    }

    /// Adds or updates a plugin record for the instance.
    pub fn add_or_update_plugin(
        &self,
        id: InstanceId,
        record: PluginRecord,
    ) -> Result<Instance, Error> {
        let mut instance = self.load(id)?;
        if let Some(existing) = instance
            .config
            .plugins
            .iter_mut()
            .find(|p| p.id == record.id)
        {
            *existing = record;
        } else {
            instance.config.plugins.push(record);
        }
        self.save(&instance)?;
        Ok(instance)
    }

    /// Copies the instance directory, except the launch log, under a new id and name.
    ///
    /// # Errors
    ///
    /// Returns the errors of `load`, the errors of `InstanceConfig::validate`,
    /// and `Error::Io` or `Error::Serialize` when the copy or `instance.toml` fails.
    /// The new directory is removed on failure.
    pub fn duplicate(&self, id: InstanceId, name: String) -> Result<Instance, Error> {
        let source = self.load(id)?;
        let mut config = source.config.clone();
        config.id = InstanceId::generate();
        config.name = name;
        config.created_at = OffsetDateTime::now_utc();
        config.validate()?;
        let instance = Instance {
            dir: self.dir_of(config.id),
            config,
        };
        let copied = copy_dir(&source.dir, &instance.dir).and_then(|()| self.save(&instance));
        if let Err(error) = copied {
            let _ = fs::remove_dir_all(&instance.dir);
            return Err(error);
        }
        info!(
            from = %id,
            id = %instance.config.id,
            dir = %instance.dir.display(),
            "instance duplicated"
        );
        Ok(instance)
    }

    /// # Errors
    ///
    /// Returns `Error::NotFound` when the instance directory does not exist,
    /// and `Error::Io` when it cannot be removed.
    pub fn delete(&self, id: InstanceId) -> Result<(), Error> {
        let dir = self.dir_of(id);
        if !dir.is_dir() {
            return Err(Error::NotFound(id));
        }
        fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        info!(%id, dir = %dir.display(), "instance deleted");
        Ok(())
    }

    /// Instances that fail to load go to `Listing::failures`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Io` when the instances directory exists and cannot be read.
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
        let mut config: InstanceConfig = toml::from_str(&text).map_err(|source| Error::Parse {
            path: path.clone(),
            source,
        })?;
        config.normalize();
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

const SKIPPED_ON_COPY: [&str; 2] = ["launch.log", "instance.toml.tmp"];

/// Copies files and directories. Symbolic links are left out.
fn copy_dir(from: &Path, to: &Path) -> Result<(), Error> {
    fs::create_dir_all(to).map_err(|e| Error::io(to, e))?;
    for entry in fs::read_dir(from).map_err(|e| Error::io(from, e))? {
        let entry = entry.map_err(|e| Error::io(from, e))?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| SKIPPED_ON_COPY.contains(&name))
        {
            continue;
        }
        let source = entry.path();
        let target = to.join(&name);
        let file_type = entry.file_type().map_err(|e| Error::io(&source, e))?;
        if file_type.is_dir() {
            copy_dir(&source, &target)?;
        } else if file_type.is_file() {
            fs::copy(&source, &target).map_err(|e| Error::io(&target, e))?;
        }
    }
    Ok(())
}
