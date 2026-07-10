use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use redb::{Database, ReadableTable, TableDefinition};

/// name -> canonical vault root path
const VAULTS: TableDefinition<&str, &str> = TableDefinition::new("vaults");

/// Central registry of every vault on this machine, so notes can link across
/// vaults with `[[vault-name/note-title]]`.
pub struct Registry {
    db: Database,
}

/// Registry location: `~/.config/banyan/`. `BANYAN_CONFIG_DIR` overrides it
/// (used by tests to avoid touching the real registry).
fn config_dir() -> Result<PathBuf> {
    if let Ok(dir) = env::var("BANYAN_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    #[allow(deprecated)] // un-deprecated in Rust 1.85; kept for older lints
    let home = env::home_dir().context("cannot determine home directory")?;
    Ok(home.join(".config").join("banyan"))
}

impl Registry {
    pub fn open() -> Result<Self> {
        Self::open_in(&config_dir()?)
    }

    fn open_in(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)
            .with_context(|| format!("creating config dir {}", dir.display()))?;
        let db_path = dir.join("registry.redb");
        let db = Database::create(&db_path)
            .with_context(|| format!("opening registry {}", db_path.display()))?;
        Ok(Self { db })
    }

    /// Register a vault. The name becomes the `[[name/...]]` link prefix, so
    /// path separators are not allowed in it.
    pub fn add(&self, name: &str, path: &Path) -> Result<PathBuf> {
        if name.is_empty() || name.contains(['/', '\\']) {
            bail!("vault name must not be empty or contain path separators");
        }
        let canonical = fs::canonicalize(path)
            .with_context(|| format!("vault path {} does not exist", path.display()))?;
        if !canonical.is_dir() {
            bail!("vault path {} is not a directory", canonical.display());
        }
        if self.get(name)?.is_some() {
            bail!("vault \"{name}\" is already registered (remove it first to re-register)");
        }
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(VAULTS)?;
            table.insert(name, canonical.to_string_lossy().as_ref())?;
        }
        txn.commit()?;
        Ok(canonical)
    }

    /// Returns true if the vault existed.
    pub fn remove(&self, name: &str) -> Result<bool> {
        let txn = self.db.begin_write()?;
        let existed = {
            let mut table = txn.open_table(VAULTS)?;
            let removed = table.remove(name)?.is_some();
            removed
        };
        txn.commit()?;
        Ok(existed)
    }

    pub fn get(&self, name: &str) -> Result<Option<PathBuf>> {
        let txn = self.db.begin_read()?;
        let table = match txn.open_table(VAULTS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(table.get(name)?.map(|v| PathBuf::from(v.value())))
    }

    /// All registered vaults, sorted by name.
    pub fn list(&self) -> Result<Vec<(String, PathBuf)>> {
        let txn = self.db.begin_read()?;
        let table = match txn.open_table(VAULTS) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (name, path) = entry?;
            out.push((name.value().to_string(), PathBuf::from(path.value())));
        }
        out.sort();
        Ok(out)
    }

    /// The registered name of the vault rooted at `path`, if any.
    pub fn name_of(&self, path: &Path) -> Result<Option<String>> {
        let Ok(canonical) = fs::canonicalize(path) else {
            return Ok(None);
        };
        Ok(self
            .list()?
            .into_iter()
            .find(|(_, root)| *root == canonical)
            .map(|(name, _)| name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(dir: &Path) -> Registry {
        Registry::open_in(&dir.join("config")).unwrap()
    }

    #[test]
    fn add_list_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let vault_a = dir.path().join("a");
        let vault_b = dir.path().join("b");
        fs::create_dir_all(&vault_a).unwrap();
        fs::create_dir_all(&vault_b).unwrap();

        let reg = registry(dir.path());
        reg.add("alpha", &vault_a).unwrap();
        reg.add("beta", &vault_b).unwrap();

        let listed = reg.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].0, "alpha");
        assert_eq!(listed[1].0, "beta");

        assert!(reg.remove("alpha").unwrap());
        assert!(!reg.remove("alpha").unwrap());
        assert_eq!(reg.list().unwrap().len(), 1);
        assert!(reg.get("alpha").unwrap().is_none());
    }

    #[test]
    fn add_rejects_bad_names_and_missing_paths() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v");
        fs::create_dir_all(&vault).unwrap();
        let reg = registry(dir.path());

        assert!(reg.add("has/slash", &vault).is_err());
        assert!(reg.add("has\\backslash", &vault).is_err());
        assert!(reg.add("", &vault).is_err());
        assert!(reg.add("ok", &dir.path().join("missing")).is_err());

        reg.add("ok", &vault).unwrap();
        assert!(reg.add("ok", &vault).is_err(), "duplicate name rejected");
    }

    #[test]
    fn name_of_matches_canonical_root() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path().join("v");
        fs::create_dir_all(&vault).unwrap();
        let reg = registry(dir.path());
        reg.add("mine", &vault).unwrap();

        assert_eq!(reg.name_of(&vault).unwrap().as_deref(), Some("mine"));
        assert!(reg.name_of(dir.path()).unwrap().is_none());
        assert!(reg.name_of(&dir.path().join("nope")).unwrap().is_none());
    }
}
