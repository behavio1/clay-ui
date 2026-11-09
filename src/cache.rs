//! Retained data only: never search results, credentials or executable instructions.
use crate::{cli::scrub, js::Object, net};
use ring::digest::{digest, SHA256};
use serde_json::{json, Value};
#[cfg(windows)]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DAY: u64 = 86400;
const RETAIN: u64 = 30 * DAY;
const FRESH: u64 = 7 * DAY;

pub fn hash(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .fold(String::new(), |mut text, b| {
            let _ = write!(text, "{b:02x}");
            text
        })
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Directory capability on Unix: all operations stay on opened, no-follow directory descriptors.
struct Directory {
    path: PathBuf,
    handle: File,
    _lock: File,
    #[cfg(windows)]
    _parents: Vec<File>,
}
impl Directory {
    fn open(root: &Path, bucket: &str) -> Option<Self> {
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, FromRawFd};
            use std::os::unix::ffi::OsStrExt;
            let absolute = if root.is_absolute() {
                root.to_path_buf()
            } else {
                std::env::current_dir().ok()?.join(root)
            }
            .join(bucket);
            let mut handle = File::open("/").ok()?;
            for part in absolute.components() {
                match part {
                    std::path::Component::RootDir => continue,
                    std::path::Component::Normal(part) => {
                        let name = std::ffi::CString::new(part.as_bytes()).ok()?;
                        // mkdirat does not follow the final component. openat refuses links, including ancestors.
                        unsafe {
                            libc::mkdirat(handle.as_raw_fd(), name.as_ptr(), 0o700);
                        }
                        let fd = unsafe {
                            libc::openat(
                                handle.as_raw_fd(),
                                name.as_ptr(),
                                libc::O_RDONLY
                                    | libc::O_DIRECTORY
                                    | libc::O_NOFOLLOW
                                    | libc::O_CLOEXEC,
                            )
                        };
                        if fd < 0 {
                            return None;
                        }
                        handle = unsafe { File::from_raw_fd(fd) };
                    }
                    _ => return None,
                }
            }
            let name = std::ffi::CString::new(".lock").ok()?;
            let mut attempt = 0;
            let fd = loop {
                let fd = unsafe {
                    libc::openat(
                        handle.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                        0o600,
                    )
                };
                if fd >= 0 {
                    break fd;
                }
                let error = std::io::Error::last_os_error();
                if attempt >= 10
                    || !matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound
                            | std::io::ErrorKind::Interrupted
                            | std::io::ErrorKind::AlreadyExists
                    )
                {
                    return None;
                }
                attempt += 1;
                std::thread::sleep(Duration::from_millis(10));
            };
            let lock = unsafe { File::from_raw_fd(fd) };
            if !lock.metadata().ok()?.is_file() {
                return None;
            }
            // Bounded lock wait; a killed process releases the OS lock automatically.
            let start = std::time::Instant::now();
            while unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                if start.elapsed() > Duration::from_secs(60) {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Some(Self {
                path: absolute,
                handle,
                _lock: lock,
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let path = if root.is_absolute() {
                root.to_path_buf()
            } else {
                std::env::current_dir().ok()?.join(root)
            }
            .join(bucket);
            let mut current = PathBuf::new();
            let mut parents = Vec::new();
            for part in path.components() {
                current.push(part);
                if matches!(part, std::path::Component::Prefix(_)) {
                    continue;
                }
                if !current.exists() {
                    let _ = fs::create_dir(&current);
                }
                // Keep every ancestor open without FILE_SHARE_DELETE: it cannot be swapped
                // between our no-follow check and a later child open/rename.
                let handle = OpenOptions::new()
                    .read(true)
                    .share_mode(3)
                    .custom_flags(0x0200_0000 | 0x0020_0000)
                    .open(&current)
                    .ok()?;
                let meta = handle.metadata().ok()?;
                use std::os::windows::fs::MetadataExt;
                if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 {
                    return None;
                }
                parents.push(handle);
            }
            let start = std::time::Instant::now();
            let lock = loop {
                if let Ok(file) = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .share_mode(0)
                    .custom_flags(0x0020_0000)
                    .open(path.join(".lock"))
                {
                    break file;
                }
                if start.elapsed() > Duration::from_secs(60) {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            if lock.metadata().ok()?.file_type().is_symlink() {
                return None;
            }
            Some(Self {
                handle: lock.try_clone().ok()?,
                path,
                _lock: lock,
                _parents: parents,
            })
        }
    }
    fn file(&self, name: &str, write: bool) -> Option<File> {
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, FromRawFd};
            let name = std::ffi::CString::new(name).ok()?;
            let flags = if write {
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL
            } else {
                libc::O_RDONLY | libc::O_NONBLOCK
            };
            let fd = unsafe {
                libc::openat(
                    self.handle.as_raw_fd(),
                    name.as_ptr(),
                    flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return None;
            }
            let file = unsafe { File::from_raw_fd(fd) };
            file.metadata().ok()?.is_file().then_some(file)
        }
        #[cfg(windows)]
        {
            let _ = &self.handle;
            let path = self.path.join(name);
            if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
                return None;
            }
            use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
            let file = OpenOptions::new()
                .read(!write)
                .write(write)
                .create_new(write)
                .custom_flags(0x0020_0000)
                .open(path)
                .ok()?;
            let meta = file.metadata().ok()?;
            (meta.is_file() && meta.file_attributes() & 0x400 == 0).then_some(file)
        }
    }
    fn remove(&self, name: &str) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            if let Ok(name) = std::ffi::CString::new(name) {
                unsafe {
                    libc::unlinkat(self.handle.as_raw_fd(), name.as_ptr(), 0);
                }
            }
        }
        #[cfg(windows)]
        {
            let _ = fs::remove_file(self.path.join(name));
        }
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Option<()> {
        let temp = format!("{name}.tmp");
        self.remove(&temp);
        let mut file = self.file(&temp, true)?;
        file.write_all(bytes).ok()?;
        file.sync_all().ok()?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let (from, to) = (
                std::ffi::CString::new(temp).ok()?,
                std::ffi::CString::new(name).ok()?,
            );
            if unsafe {
                libc::renameat(
                    self.handle.as_raw_fd(),
                    from.as_ptr(),
                    self.handle.as_raw_fd(),
                    to.as_ptr(),
                )
            } != 0
            {
                return None;
            }
        }
        #[cfg(windows)]
        {
            fs::rename(self.path.join(temp), self.path.join(name)).ok()?;
        }
        Some(())
    }
    fn names(&self) -> Vec<String> {
        // Enumeration is advisory; reads/deletes always use the pinned descriptor on Unix.
        fs::read_dir(&self.path)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| {
                n.ends_with(".json")
                    && n.len() == 69
                    && n[..64].bytes().all(|b| b.is_ascii_hexdigit())
            })
            .collect()
    }
}

pub struct Store {
    directory: Directory,
    time: u64,
    limit: usize,
    inclusive: bool,
}
impl Store {
    pub fn open(root: &Path, bucket: &str, limit: usize) -> Option<Self> {
        let directory = Directory::open(root, bucket)?;
        let store = Self {
            directory,
            time: now(),
            limit,
            inclusive: bucket == "components",
        };
        store.prune();
        Some(store)
    }
    fn at(root: &Path, bucket: &str, limit: usize, time: u64) -> Option<Self> {
        let store = Self {
            directory: Directory::open(root, bucket)?,
            time,
            limit,
            inclusive: bucket == "components",
        };
        store.prune();
        Some(store)
    }
    pub fn get(&self, key: &str) -> Option<(Value, u64)> {
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let name = format!("{key}.json");
        let read = || {
            let mut text = String::new();
            self.directory
                .file(&name, false)?
                .take(32 * 1024 * 1024)
                .read_to_string(&mut text)
                .ok()?;
            if scrub(&text) != text {
                return None;
            }
            let v: Value = serde_json::from_str(&text).ok()?;
            if crate::cli::scrub_value(v.clone()) != v {
                return None;
            }
            let created = v["created"].as_u64()?;
            if created > self.time
                || (self.time - created > RETAIN
                    || (!self.inclusive && self.time - created == RETAIN))
                || v["key"].as_str()? != key
            {
                return None;
            }
            let data = v.get("data")?.clone();
            if v["sha256"].as_str()? != hash(json!([key, created, data]).to_string().as_bytes()) {
                return None;
            }
            Some((data, self.time - created))
        };
        let value = read();
        if value.is_none() {
            self.directory.remove(&name);
        }
        value
    }
    pub fn put(&self, key: &str, data: Value) {
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return;
        }
        if crate::cli::scrub_value(data.clone()) != data {
            return;
        }
        let text = json!({"key":key,"created":self.time,"sha256":hash(json!([key,self.time,data]).to_string().as_bytes()),"data":data}).to_string();
        // Do not store an entry if upstream echoed any known credential.
        if scrub(&text) == text {
            let _ = self
                .directory
                .write(&format!("{key}.json"), text.as_bytes());
        }
        self.prune();
    }
    fn prune(&self) {
        let mut entries: Vec<_> = self
            .directory
            .names()
            .into_iter()
            .filter_map(|name| self.get(&name[..64]).map(|(_, age)| (age, name)))
            .collect();
        // Oldest creation time first, deterministic tie-breaker.
        entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, name) in entries
            .iter()
            .take(entries.len().saturating_sub(self.limit))
        {
            self.directory.remove(name);
        }
    }
}

pub fn prune_all(root: &Path) {
    let _ = Store::open(root, "components", usize::MAX);
    let _ = Store::open(root, "queries", 2000);
}

pub fn component(
    root: &Path,
    item: &Object,
    fetch: impl FnOnce() -> net::Res<Object>,
) -> net::Res<Object> {
    component_at(root, item, now(), fetch)
}

fn component_at(
    root: &Path,
    item: &Object,
    time: u64,
    fetch: impl FnOnce() -> net::Res<Object>,
) -> net::Res<Object> {
    // The complete upstream index record includes id, package version, URLs and static content.
    // Changing any upstream identity field bypasses the old entry.
    let key = hash(Value::Object(item.clone()).to_string().as_bytes());
    let store = Store::at(root, "components", usize::MAX, time);
    let cached = store
        .as_ref()
        .and_then(|s| s.get(&key))
        .and_then(|(v, age)| {
            Some((
                {
                    let mut value = v.as_object()?.clone();
                    value
                        .entry("fetchedAt")
                        .or_insert_with(|| json!(crate::js::iso_at((time - age) as i64 * 1000)));
                    value.insert("fromCache".into(), json!(true));
                    value
                },
                age,
            ))
        });
    if let Some((value, age)) = &cached {
        if *age < FRESH {
            return Ok(value.clone());
        }
    }
    match fetch() {
        Ok(mut value) => {
            value.insert(
                "fetchedAt".into(),
                json!(crate::js::iso_at(time as i64 * 1000)),
            );
            value.insert("fromCache".into(), json!(false));
            let downloadable = value
                .get("files")
                .and_then(Value::as_array)
                .is_some_and(|files| {
                    files.iter().any(|f| {
                        f.get("content")
                            .and_then(Value::as_str)
                            .is_some_and(|s| !s.is_empty())
                    })
                });
            if downloadable {
                if let Some(store) = store {
                    store.put(&key, Value::Object(value.clone()));
                }
                return Ok(value);
            }
            // Some adapters report a GitHub block as a successful detail without source files.
            if cached.is_none() {
                return Ok(value);
            }
            stale(cached)
        }
        Err(error) => {
            if cached.is_some() {
                stale(cached)
            } else {
                Err(error)
            }
        }
    }
}
fn stale(cached: Option<(Object, u64)>) -> net::Res<Object> {
    let (mut value, age) = cached.ok_or_else(|| net::Fail::other("No retained component"))?;
    let days = age / DAY;
    value.insert("cacheAgeDays".into(), json!(days));
    value.insert("cacheWarning".into(), json!(format!("Warning: upstream unavailable; using cached component aged {days} days. Retry when the source is available.")));
    Ok(value)
}

/// Existing index/document-vector writes use the same no-follow directory capability.
pub(crate) fn write_state(file: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = file.parent().ok_or("Cache file needs a parent directory")?;
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid cache filename")?;
    let directory = Directory::open(parent, "")
        .ok_or("Refusing to write cache: inaccessible directory or symbolic link")?;
    directory
        .write(name, bytes)
        .ok_or_else(|| "Cannot atomically write cache file".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "clayui-cache-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p.canonicalize().unwrap())
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn item() -> Object {
        json!({"id":"test/button","version":"1"})
            .as_object()
            .unwrap()
            .clone()
    }
    fn code() -> Object {
        json!({"id":"test/button","files":[{"path":"button.tsx","content":"export const button = 1;"}]}).as_object().unwrap().clone()
    }
    #[test]
    fn offline_github_block_freshness_boundaries_and_warning() {
        let t = Temp::new();
        let base = 100 * DAY;
        component_at(&t.0, &item(), base, || Ok(code())).unwrap();
        assert!(component_at(&t.0, &item(), base + 7 * DAY - 1, || panic!(
            "fresh must not fetch"
        ))
        .is_ok());
        let stale = component_at(&t.0, &item(), base + 7 * DAY, || {
            Err(net::Fail::source("GitHub HTTP 403"))
        })
        .unwrap();
        assert_eq!(stale["cacheAgeDays"], 7);
        assert!(stale["cacheWarning"].as_str().unwrap().contains("7 days"));
        let blocked = component_at(&t.0, &item(), base + 8 * DAY, || {
            Ok(json!({"files":[],"sourceWarnings":["GitHub HTTP 403"]})
                .as_object()
                .unwrap()
                .clone())
        })
        .unwrap();
        assert_eq!(blocked["cacheAgeDays"], 8);
        assert_eq!(
            component_at(&t.0, &item(), base + 30 * DAY, || Err(net::Fail::source(
                "offline"
            )))
            .unwrap()["cacheAgeDays"],
            30
        );
        assert!(component_at(&t.0, &item(), base + 30 * DAY + 1, || Err(
            net::Fail::source("offline")
        ))
        .is_err());
        assert_eq!(fs::read_dir(t.0.join("components")).unwrap().count(), 1); // lock only
    }
    #[test]
    fn cached_timestamp_is_the_content_fetch_even_for_legacy_entries() {
        let t = Temp::new();
        let base = 100 * DAY;
        let first = component_at(&t.0, &item(), base, || Ok(code())).unwrap();
        let warm = component_at(&t.0, &item(), base + DAY, || panic!("fresh")).unwrap();
        let stale = component_at(&t.0, &item(), base + 8 * DAY, || {
            Err(net::Fail::other("offline"))
        })
        .unwrap();
        assert_eq!(first["fromCache"], false);
        for value in [&warm, &stale] {
            assert_eq!(value["fromCache"], true);
            assert_eq!(value["fetchedAt"], first["fetchedAt"]);
        }
        // Entries produced before fetchedAt existed use their original creation timestamp.
        let key = hash(Value::Object(item()).to_string().as_bytes());
        Store::at(&t.0, "components", usize::MAX, base)
            .unwrap()
            .put(&key, Value::Object(code()));
        let legacy = component_at(&t.0, &item(), base + DAY, || panic!("fresh")).unwrap();
        assert_eq!(legacy["fetchedAt"], first["fetchedAt"]);
    }
    #[test]
    fn corrupted_tampered_and_new_upstream_version_refetch() {
        let t = Temp::new();
        let i = item();
        let key = hash(Value::Object(i.clone()).to_string().as_bytes());
        let base = 100 * DAY;
        for bad in ["not json", "tampered"] {
            component_at(&t.0, &i, base, || Ok(code())).unwrap();
            let file = t.0.join("components").join(format!("{key}.json"));
            if bad == "not json" {
                fs::write(&file, bad).unwrap();
            } else {
                let mut v: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
                v["data"]["files"][0]["content"] = json!("changed");
                fs::write(&file, v.to_string()).unwrap();
            }
            let mut fetched = false;
            component_at(&t.0, &i, base + 1, || {
                fetched = true;
                Ok(code())
            })
            .unwrap();
            assert!(fetched);
        }
        let mut i = item();
        i.insert("version".into(), json!("2"));
        let mut fetched = false;
        component_at(&t.0, &i, base + 1, || {
            fetched = true;
            Ok(code())
        })
        .unwrap();
        assert!(fetched);
    }
    #[test]
    fn queries_expire_at_30_days_and_oldest_is_evicted_at_2000() {
        let t = Temp::new();
        let base = 100 * DAY;
        let key = hash(b"oldest");
        {
            let s = Store::at(&t.0, "queries", 2000, base).unwrap();
            s.put(&key, json!("vector"));
        }
        assert!(Store::at(&t.0, "queries", 2000, base + 30 * DAY - 1)
            .unwrap()
            .get(&key)
            .is_some());
        assert!(Store::at(&t.0, "queries", 2000, base + 30 * DAY)
            .unwrap()
            .get(&key)
            .is_none());
        {
            let s = Store::at(&t.0, "queries", 2000, base).unwrap();
            s.put(&key, json!("old"));
        }
        // Stage valid entries in one lock; exercise real pruning at the exact size boundary.
        let s = Store::at(&t.0, "queries", 2000, base + 1).unwrap();
        for n in 0..2000 {
            let key = hash(n.to_string().as_bytes());
            let data = json!(n);
            let text=json!({"key":key,"created":base+1,"sha256":hash(json!([key,base+1,data]).to_string().as_bytes()),"data":data}).to_string();
            s.directory
                .write(&format!("{key}.json"), text.as_bytes())
                .unwrap();
        }
        s.prune();
        assert_eq!(s.directory.names().len(), 2000);
        assert!(s.get(&key).is_none());
        assert_ne!(
            hash(json!(["model-a", "Query"]).to_string().as_bytes()),
            hash(json!(["model-b", "Query"]).to_string().as_bytes())
        );
    }
    #[test]
    fn concurrent_cache_writes_are_atomic_and_preserve_entries() {
        let t = Temp::new();
        let exe = std::env::current_exe().unwrap();
        let mut children: Vec<_> = (0..6)
            .map(|n| {
                std::process::Command::new(&exe)
                    .args(["--exact", "cache::tests::process_cache_worker"])
                    .env("CACHE_TEST_ROOT", &t.0)
                    .env("CACHE_TEST_N", n.to_string())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let statuses: Vec<_> = children.iter_mut().map(|c| c.wait().unwrap()).collect();
        assert!(statuses.iter().all(|s| s.success()), "{statuses:?}");
        let s = Store::open(&t.0, "queries", 2000).unwrap();
        assert_eq!(s.directory.names().len(), 6);
        for n in 0..6 {
            assert_eq!(s.get(&hash(n.to_string().as_bytes())).unwrap().0, json!(n));
        }
    }
    #[test]
    fn process_cache_worker() {
        let Some(root) = std::env::var_os("CACHE_TEST_ROOT") else {
            return;
        };
        let n: usize = std::env::var("CACHE_TEST_N").unwrap().parse().unwrap();
        let s = Store::open(Path::new(&root), "queries", 2000).unwrap();
        s.put(&hash(n.to_string().as_bytes()), json!(n));
    }
    #[cfg(unix)]
    #[test]
    fn symlink_bucket_parent_entry_and_temp_never_write_through() {
        use std::os::unix::fs::symlink;
        let t = Temp::new();
        let victim = Temp::new();
        symlink(&victim.0, t.0.join("components")).unwrap();
        assert!(Store::open(&t.0, "components", usize::MAX).is_none());
        assert_eq!(fs::read_dir(&victim.0).unwrap().count(), 0);
        let key = hash(b"test");
        let s = Store::open(&t.0, "queries", 2000).unwrap();
        let file = victim.0.join("victim");
        fs::write(&file, "unchanged").unwrap();
        symlink(&file, t.0.join("queries").join(format!("{key}.json"))).unwrap();
        assert!(s.get(&key).is_none());
        symlink(&file, t.0.join("queries").join(format!("{key}.json.tmp"))).unwrap();
        s.put(&key, json!("safe"));
        assert_eq!(fs::read_to_string(file).unwrap(), "unchanged");
        let linked = t.0.join("parent");
        symlink(&victim.0, &linked).unwrap();
        assert!(Store::open(&linked.join("child"), "queries", 2000).is_none());
    }
}
