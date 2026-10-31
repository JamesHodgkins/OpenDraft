//! Finding a running app. Each run writes a descriptor file naming its
//! endpoint and a fresh token into a per-user directory; the sidecar reads
//! it. The file is readable only by the user (mode 0600 on Linux; the
//! per-user profile folder on Windows) and removed on a clean exit. A stale
//! file is discovered by failing to connect, not by guessing from the pid.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Descriptor {
    pub instance_id: String,
    pub pid: u32,
    pub app_version: String,
    pub api_versions: Vec<u32>,
    /// A named pipe name (Windows) or socket path (Linux).
    pub endpoint: String,
    /// 256 random bits as hex. Never logged.
    pub token: String,
}

/// A descriptor without its token, safe to show a model or write to a log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub pid: u32,
    pub app_version: String,
    pub api_versions: Vec<u32>,
}

impl Descriptor {
    pub fn info(&self) -> InstanceInfo {
        InstanceInfo {
            instance_id: self.instance_id.clone(),
            pid: self.pid,
            app_version: self.app_version.clone(),
            api_versions: self.api_versions.clone(),
        }
    }
}

/// Names a different descriptor folder, for tests and unusual setups. Both
/// the app and the MCP server read it, so they must be started with the same
/// value.
pub const DIRECTORY_ENV: &str = "OD_AUTOMATION_DIR";

/// The per-user folder holding descriptors, or `None` if the environment
/// does not say where it is.
pub fn directory() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(DIRECTORY_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    #[cfg(not(windows))]
    let base = unix_base(|name| std::env::var_os(name))?;
    Some(base.join("OpenDraft").join("automation"))
}

/// Where a Unix session keeps its per-user runtime files: `XDG_RUNTIME_DIR`
/// (`/run/user/<uid>`, which the login system makes and only the user can
/// enter) when there is one. Some sessions have none (`su`, `ssh` without a
/// login manager, a few containers and minimal desktops); there the user's
/// cache folder serves, so the connection still works.
#[cfg_attr(windows, allow(dead_code))]
fn unix_base(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| var(name).map(PathBuf::from).filter(|p| p.is_absolute());
    if let Some(run) = absolute("XDG_RUNTIME_DIR").filter(|p| p.is_dir()) {
        return Some(run);
    }
    absolute("XDG_CACHE_HOME").or_else(|| absolute("HOME").map(|home| home.join(".cache")))
}

/// Make `dir` and any missing parents, readable only by the user on Unix: the
/// socket and the file holding the token live in it.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

fn file_name(instance_id: &str) -> String {
    format!("{instance_id}.json")
}

/// Only ids this module made are used as file names.
pub fn valid_instance_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub fn write(dir: &Path, descriptor: &Descriptor) -> io::Result<PathBuf> {
    if !valid_instance_id(&descriptor.instance_id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bad instance id",
        ));
    }
    create_private_dir(dir)?;
    let path = dir.join(file_name(&descriptor.instance_id));
    let json = serde_json::to_vec(descriptor).map_err(io::Error::other)?;
    write_private(&path, &json)?;
    Ok(path)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)
}

/// Every readable descriptor in `dir`, newest instance ids last. Files that
/// do not parse are skipped.
pub fn list(dir: &Path) -> Vec<Descriptor> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Descriptor> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| fs::read(e.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<Descriptor>(&bytes).ok())
        .filter(|d| valid_instance_id(&d.instance_id))
        .collect();
    found.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
    found
}

/// Remove an instance's descriptor; a missing file is fine.
pub fn remove(dir: &Path, instance_id: &str) {
    if valid_instance_id(instance_id) {
        let _ = fs::remove_file(dir.join(file_name(instance_id)));
    }
}

#[cfg(test)]
mod tests {
    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    fn here() -> String {
        std::env::temp_dir().to_string_lossy().into_owned()
    }

    #[test]
    fn the_runtime_folder_is_used_when_the_session_has_one() {
        let run = here();
        let found = unix_base(env(&[("XDG_RUNTIME_DIR", &run), ("HOME", "/home/someone")]));
        assert_eq!(found, Some(PathBuf::from(run)));
    }

    #[test]
    fn without_a_runtime_folder_the_cache_folder_serves() {
        let home = here();
        let cache = std::env::temp_dir().join("elsewhere");
        let by_home = unix_base(env(&[("HOME", &home)]));
        assert_eq!(by_home, Some(PathBuf::from(&home).join(".cache")));
        let cache_text = cache.to_string_lossy().into_owned();
        let by_cache = unix_base(env(&[("XDG_CACHE_HOME", &cache_text), ("HOME", &home)]));
        assert_eq!(by_cache, Some(cache));
    }

    #[test]
    fn a_runtime_folder_that_is_not_there_or_not_absolute_is_not_trusted() {
        let home = here();
        let gone = std::env::temp_dir().join("od-no-such-run-dir");
        let gone_text = gone.to_string_lossy().into_owned();
        let found = unix_base(env(&[("XDG_RUNTIME_DIR", &gone_text), ("HOME", &home)]));
        assert_eq!(found, Some(PathBuf::from(&home).join(".cache")));
        let relative = unix_base(env(&[("XDG_RUNTIME_DIR", "run"), ("HOME", &home)]));
        assert_eq!(relative, Some(PathBuf::from(&home).join(".cache")));
    }

    #[test]
    fn nothing_in_the_environment_means_no_folder() {
        assert_eq!(unix_base(env(&[])), None);
        assert_eq!(unix_base(env(&[("HOME", "relative/home")])), None);
    }

    #[cfg(unix)]
    #[test]
    fn the_descriptor_folder_is_made_for_the_user_alone() {
        use std::os::unix::fs::PermissionsExt;
        let top = std::env::temp_dir().join(format!("od-private-{}", std::process::id()));
        let _ = fs::remove_dir_all(&top);
        let dir = top.join("a/b");
        create_private_dir(&dir).unwrap();
        for folder in [&top, &top.join("a"), &dir] {
            let mode = fs::metadata(folder).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "{}", folder.display());
        }
        // Making it again is fine.
        create_private_dir(&dir).unwrap();
        let _ = fs::remove_dir_all(&top);
    }

    use super::*;

    fn descriptor(id: &str) -> Descriptor {
        Descriptor {
            instance_id: id.to_owned(),
            pid: 1,
            app_version: "0.1.0".to_owned(),
            api_versions: vec![1],
            endpoint: "od-test".to_owned(),
            token: "secret".to_owned(),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("od-api-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_descriptor_is_written_listed_and_removed() {
        let dir = scratch("roundtrip");
        write(&dir, &descriptor("aaa")).unwrap();
        write(&dir, &descriptor("bbb")).unwrap();
        fs::write(dir.join("junk.json"), b"not json").unwrap();
        let found = list(&dir);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0], descriptor("aaa"));
        remove(&dir, "aaa");
        assert_eq!(list(&dir).len(), 1);
        remove(&dir, "aaa");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_cannot_escape_the_folder() {
        let dir = scratch("escape");
        for bad in ["", "../x", "a/b", "a.b"] {
            assert!(write(&dir, &descriptor(bad)).is_err(), "{bad:?}");
        }
        remove(&dir, "../x");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_public_info_has_no_token() {
        let json = serde_json::to_string(&descriptor("a").info()).unwrap();
        assert!(!json.contains("secret") && !json.contains("endpoint"));
    }
}
