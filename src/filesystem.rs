//! Linux descriptor operations. Paths in plans are raw bytes relative to the pinned root.
use crate::{
    error::{Error, Result},
    intent::Location,
};
use rustix::fs::{openat2, Mode, OFlags, ResolveFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::{CStr, OsStr, OsString},
    fs::{File, Metadata},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub device_id: u64,
    pub inode: u64,
    pub mode: u32,
    pub size: u64,
    pub mtime_ns: i128,
    pub ctime_ns: i128,
}
impl Snapshot {
    pub fn from_meta(m: &Metadata) -> Self {
        Self {
            device_id: m.dev(),
            inode: m.ino(),
            mode: m.mode(),
            size: m.size(),
            mtime_ns: i128::from(m.mtime()) * 1_000_000_000 + i128::from(m.mtime_nsec()),
            ctime_ns: i128::from(m.ctime()) * 1_000_000_000 + i128::from(m.ctime_nsec()),
        }
    }
    pub fn regular(&self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFREG
    }
    pub fn symlink(&self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFLNK
    }
    pub fn directory(&self) -> bool {
        self.mode & libc::S_IFMT == libc::S_IFDIR
    }
    pub fn identity(&self) -> Identity {
        Identity {
            device_id: self.device_id,
            inode: self.inode,
            mode: self.mode,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub device_id: u64,
    pub inode: u64,
    pub mode: u32,
}

pub struct Root {
    path: PathBuf,
    fd: File,
    aliases: BTreeMap<String, PathBuf>,
}
impl Root {
    /// Tests inject a temporary directory. Production calls account_home(), never $HOME.
    pub fn new(path: &Path) -> Result<Self> {
        let path = path.canonicalize()?;
        let fd = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)?;
        let aliases = [
            ("home", "."),
            ("desktop", "Desktop"),
            ("documents", "Documents"),
            ("downloads", "Downloads"),
            ("pictures", "Pictures"),
            ("archive", "Archive"),
        ]
        .into_iter()
        .map(|(a, p)| (a.into(), p.into()))
        .collect();
        let root = Self { path, fd, aliases };
        root.directory(Path::new("."))?; // Require openat2; never fall back to weaker lookup.
        Ok(root)
    }
    pub fn production() -> Result<Self> {
        if rustix::process::geteuid().is_root()
            || rustix::process::getuid() != rustix::process::geteuid()
            || rustix::process::getgid() != rustix::process::getegid()
        {
            return Err(Error::new(
                "PERMISSION_DENIED",
                "Run as an ordinary user without elevated credentials",
            ));
        }
        let mut root = Self::new(&account_home()?)?;
        // Parse the narrow XDG assignment format as data, never source it as a script.
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| root.path.join(".config"));
        match std::fs::read_to_string(config.join("user-dirs.dirs")) {
            Ok(text) => {
                for line in text.lines() {
                    for (key, alias) in [
                        ("DESKTOP", "desktop"),
                        ("DOCUMENTS", "documents"),
                        ("DOWNLOAD", "downloads"),
                        ("PICTURES", "pictures"),
                    ] {
                        if let Some(value) = line
                            .trim()
                            .strip_prefix(&format!("XDG_{key}_DIR=\""))
                            .and_then(|v| v.strip_suffix('"'))
                        {
                            let path = if let Some(tail) = value.strip_prefix("$HOME/") {
                                root.path.join(tail)
                            } else if value == "$HOME" {
                                root.path.clone()
                            } else {
                                PathBuf::from(value)
                            };
                            root.aliases.insert(alias.into(), path);
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
        Ok(root)
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn identity(&self) -> Result<Identity> {
        Ok(Snapshot::from_meta(&self.fd.metadata()?).identity())
    }
    pub fn validate_live(&self) -> Result<()> {
        let live = File::from(openat2(
            rustix::fs::CWD,
            &self.path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::NO_SYMLINKS,
        )?);
        if Snapshot::from_meta(&live.metadata()?).identity() != self.identity()? {
            return Err(Error::new(
                "PLAN_INVALIDATED",
                "Approved root pathname changed",
            ));
        }
        Ok(())
    }
    /// Check the live ancestry of a held directory, without reopening its old name.
    /// This is a point-in-time check, not a lock against subsequent directory moves.
    pub(crate) fn contains_directory(&self, directory: &File) -> Result<()> {
        let expected = self.identity()?;
        let mut current = directory.try_clone()?;
        for _ in 0..1024 {
            let identity = Snapshot::from_meta(&current.metadata()?).identity();
            if identity == expected {
                return Ok(());
            }
            // Deliberately walk the fixed parent component, not user input.
            // BENEATH would prohibit this walk; no symlinks or mount crossings occur.
            let parent = File::from(openat2(
                &current,
                "..",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
            )?);
            if Snapshot::from_meta(&parent.metadata()?).identity() == identity {
                break;
            }
            current = parent;
        }
        Err(Error::new(
            "OUTSIDE_ALLOWED_ROOT",
            "Held directory is not provably beneath the approved root",
        ))
    }
    pub fn relative(&self, path: &Path) -> Result<PathBuf> {
        let p = if path.is_absolute() {
            path.strip_prefix(&self.path)
                .map_err(|_| Error::new("OUTSIDE_ALLOWED_ROOT", "Path outside approved root"))?
        } else {
            path
        };
        if p.components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
            || p.as_os_str().as_bytes().contains(&0)
        {
            return Err(Error::new(
                "OUTSIDE_ALLOWED_ROOT",
                "Parent traversal and invalid paths are prohibited",
            ));
        }
        let normalized: PathBuf = p
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .collect();
        Ok(if normalized.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            normalized
        })
    }
    pub fn location(&self, loc: &Location) -> Result<PathBuf> {
        match loc {
            Location::DirectoryAlias(a) => self.relative(
                self.aliases
                    .get(a)
                    .ok_or_else(|| Error::new("UNSUPPORTED_REQUEST", "Unknown alias"))?,
            ),
            Location::PathBytes(b) => self.relative(Path::new(OsStr::from_bytes(b))),
        }
    }
    fn open(&self, path: &Path, flags: OFlags) -> Result<File> {
        let p = self.relative(path)?;
        Ok(File::from(openat2(
            &self.fd,
            &p,
            flags | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )?))
    }
    pub fn directory(&self, path: &Path) -> Result<File> {
        self.open(path, OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW)
    }
    pub fn snapshot(&self, path: &Path) -> Result<Snapshot> {
        Ok(Snapshot::from_meta(
            &self
                .open(path, OFlags::PATH | OFlags::NOFOLLOW)?
                .metadata()?,
        ))
    }
    pub fn absent(&self, path: &Path) -> Result<bool> {
        let p = self.relative(path)?;
        match openat2(
            &self.fd,
            p,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        ) {
            Ok(_) => Ok(false),
            Err(rustix::io::Errno::NOENT) => Ok(true),
            Err(e) => Err(e.into()),
        }
    }
    pub fn entries(&self, path: &Path) -> Result<Vec<(PathBuf, Snapshot)>> {
        let fd = self.directory(path)?;
        let mut entries = Vec::new();
        for item in rustix::fs::Dir::read_from(&fd)? {
            let item = item?;
            let bytes = item.file_name().to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            let p = path.join(OsStr::from_bytes(bytes));
            entries.push((p.clone(), self.snapshot(&p)?));
        }
        entries.sort_by(|a, b| a.0.as_os_str().as_bytes().cmp(b.0.as_os_str().as_bytes()));
        Ok(entries)
    }
}
pub fn bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().as_bytes().to_vec()
}
pub fn path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes.to_vec()))
}

fn basename(name: &OsStr) -> Result<()> {
    if name.as_bytes().is_empty()
        || name.as_bytes().contains(&b'/')
        || name.as_bytes().contains(&0)
        || name == "."
        || name == ".."
    {
        return Err(Error::new(
            "OUTSIDE_ALLOWED_ROOT",
            "Operation requires a single filename",
        ));
    }
    Ok(())
}
/// Pin and inspect the entry itself, including a final symlink, without following it.
/// NO_SYMLINKS also implies NO_MAGICLINKS on Linux. Callers reject nonregular sources.
pub(crate) fn entry_at(parent: &File, name: &OsStr) -> Result<File> {
    basename(name)?;
    Ok(File::from(openat2(
        parent,
        name,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
    )?))
}
pub(crate) fn absent_at(parent: &File, name: &OsStr) -> Result<bool> {
    basename(name)?;
    match rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Err(rustix::io::Errno::NOENT) => Ok(true),
        Ok(_) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn rename_noreplace(
    source_parent: &File,
    source: &OsStr,
    destination_parent: &File,
    destination: &OsStr,
) -> Result<()> {
    // Only single components: callers cannot smuggle path traversal into renameat2.
    for name in [source, destination] {
        basename(name)?;
    }
    rustix::fs::renameat_with(
        source_parent,
        source,
        destination_parent,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    Ok(())
}
pub fn account_home() -> Result<PathBuf> {
    let mut buffer = vec![0u8; 65_536];
    let mut passwd = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = std::ptr::null_mut();
    // SAFETY: writable buffers have their stated sizes and live through the call.
    let code = unsafe {
        libc::getpwuid_r(
            rustix::process::getuid().as_raw(),
            passwd.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if code != 0 || result.is_null() {
        return Err(Error::new("IO_ERROR", "Cannot determine account home"));
    }
    // SAFETY: success with a non-null result initializes passwd; pw_dir points into buffer.
    let passwd = unsafe { passwd.assume_init() };
    if passwd.pw_dir.is_null() {
        return Err(Error::new("IO_ERROR", "Account has no home"));
    }
    // SAFETY: getpwuid_r provides a NUL-terminated string valid while buffer lives.
    let home = unsafe { CStr::from_ptr(passwd.pw_dir) }.to_bytes();
    let p = path(home);
    if !p.is_absolute() || p == Path::new("/") {
        return Err(Error::new("OUTSIDE_ALLOWED_ROOT", "Invalid account home"));
    }
    Ok(p)
}
