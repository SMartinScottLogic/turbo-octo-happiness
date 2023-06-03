use fuse_mt::{
    DirectoryEntry, FileAttr, FileType, FilesystemMT, RequestInfo, ResultEmpty, ResultEntry,
    ResultOpen, ResultReaddir, ResultXattr, Xattr,
};
use magic::{Cookie, CookieFlags};
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Component::Normal, Path, PathBuf};
use std::time::{Duration, SystemTime};
use walkdir::WalkDir;

use crate::libc_wrapper;

static TTL: Duration = Duration::from_secs(1);

#[derive(Debug)]
struct FSEntry {
    filename: PathBuf,
    filetype: String,
    size: u64,
}

impl FSEntry {
    fn stat(&self) -> io::Result<FileAttr> {
        let stat = libc_wrapper::lstat(&self.filename)?;
        Ok(Self::stat_to_fuse(stat))
    }
    fn stat_to_fuse(stat: libc::stat) -> FileAttr {
        // st_mode encodes both the kind and the permissions
        let kind = OrganiseFS::mode_to_filetype(stat.st_mode);
        let perm = (stat.st_mode & 0o7777) as u16;

        FileAttr {
            size: stat.st_size as u64,
            blocks: stat.st_blocks as u64,
            atime: SystemTime::UNIX_EPOCH
                + Duration::from_secs(stat.st_atime.try_into().unwrap())
                + Duration::from_nanos(stat.st_atime_nsec.try_into().unwrap()),
            mtime: SystemTime::UNIX_EPOCH
                + Duration::from_secs(stat.st_mtime.try_into().unwrap())
                + Duration::from_nanos(stat.st_mtime_nsec.try_into().unwrap()),
            ctime: SystemTime::UNIX_EPOCH
                + Duration::from_secs(stat.st_ctime.try_into().unwrap())
                + Duration::from_nanos(stat.st_ctime_nsec.try_into().unwrap()),
            crtime: SystemTime::UNIX_EPOCH,
            kind,
            perm,
            nlink: stat.st_nlink as u32,
            uid: stat.st_uid,
            gid: stat.st_gid,
            rdev: stat.st_rdev as u32,
            flags: 0,
        }
    }
}


impl TryFrom<(&walkdir::DirEntry, &magic::Cookie)> for FSEntry {
    type Error = std::io::Error;
    fn try_from(
        (entry, cookie): (&walkdir::DirEntry, &magic::Cookie),
    ) -> Result<Self, Self::Error> {
        let filename = entry.path().canonicalize()?;
        let metadata = entry.metadata()?;
        let size = metadata.len();
        let filetype = cookie
            .file(&filename)
            .unwrap()
            .split(';')
            .next()
            .unwrap_or("unknown")
            .replace("/", "_");
        Ok(Self {
            filename,
            size,
            filetype,
        })
    }
}

pub struct OrganiseFS {
    root: String,
    tags: HashSet<OsString>,
    entries: Vec<FSEntry>,
    attrs: HashMap<&'static str, &'static str>,
}

impl OrganiseFS {
    pub fn new(root: &str) -> Self {
        let entries = scan(root);
        debug!("{:#?}", entries);
        Self {
            root: root.to_string(),
            tags: HashSet::new(),
            /*
            tags: entries
                .iter()
                .flat_map(|tag_entry| tag_entry.tags.clone())
                .collect(),
            */
            entries,
            attrs: vec![("user.tagfs.strategy", "0"), ("user.tagfs.depth", "1")]
                .into_iter()
                .collect(),
        }
    }

    fn mode_to_filetype(mode: libc::mode_t) -> FileType {
        match mode & libc::S_IFMT {
            libc::S_IFDIR => FileType::Directory,
            libc::S_IFREG => FileType::RegularFile,
            libc::S_IFLNK => FileType::Symlink,
            libc::S_IFBLK => FileType::BlockDevice,
            libc::S_IFCHR => FileType::CharDevice,
            libc::S_IFIFO => FileType::NamedPipe,
            libc::S_IFSOCK => FileType::Socket,
            _ => {
                panic!("unknown file type");
            }
        }
    }

    fn stat_to_fuse() -> FileAttr {
        FileAttr {
            size: 0,
            blocks: 0,
            atime: SystemTime::UNIX_EPOCH,
        mtime: SystemTime::UNIX_EPOCH,
        ctime: SystemTime::UNIX_EPOCH,
        crtime: SystemTime::UNIX_EPOCH,
            kind: FileType::Directory,
            perm: 0o0755,
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
        }
    }

    fn tags(path: &Path) -> Option<Vec<OsString>> {
        Some(
            path.parent()?
                .components()
                .map(|comp| comp.as_os_str().to_owned())
                .filter(|comp| comp != "/")
                .collect(),
        )
    }
}

fn info(entry: &walkdir::DirEntry, meta: &std::fs::Metadata) {
    let dev_id = meta.dev();
    let inode = meta.ino();
    println!(
        "{} {} {} {:o} {:?} {} {} (@ {})",
        dev_id,
        inode,
        entry.path().display(),
        meta.mode(),
        meta.is_dir(),
        meta.is_file(),
        meta.size(),
        std::env::current_dir()
            .unwrap()
            .as_path()
            .join(entry.path())
            .canonicalize()
            .unwrap()
            .display()
    );
}

fn process(_root: &str, entry: &walkdir::DirEntry, cookie: &Cookie) -> Option<FSEntry> {
    let meta = match fs::metadata(entry.path()) {
        Ok(meta) => meta,
        _ => return None,
    };
    //info(&entry, &meta);
    if meta.is_file() {
        if let Some(_p) = entry.path().parent() {
            return FSEntry::try_from((entry, cookie)).ok()
        }
    };
    None
}

fn scan(root: &str) -> Vec<FSEntry> {
    let mut flags = CookieFlags::empty();
    flags.insert(magic::CookieFlags::MIME);

    let cookie = Cookie::open(flags).unwrap();
    let databases = vec!["/usr/share/misc/magic"];
    cookie
        .load(&databases)
        .expect("Loading databases for mime types");

    WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok().and_then(|entry| process(root, &entry, &cookie)))
        .collect()
}

impl FilesystemMT for OrganiseFS {
    fn init(&self, _req: RequestInfo) -> ResultEmpty {
        debug!("init");
        Ok(())
    }

    fn destroy(&self) {
        debug!("destroy");
    }

    fn getattr(&self, _req: RequestInfo, path: &Path, fh: Option<u64>) -> ResultEntry {
        debug!("getattr: {:?} {:?}", path, fh);

        debug!("TODO: lookup {:?} {:?}", path, Self::tags(path));
        Ok((TTL, Self::stat_to_fuse()))
        /*

        if let Some(fh) = fh {
            match libc_wrappers::fstat(fh) {
                Ok(stat) => Ok((TTL, stat_to_fuse(stat))),
                Err(e) => Err(e)
            }
        } else {
            match self.stat_real(path) {
                Ok(attr) => Ok((TTL, attr)),
                Err(e) => Err(e.raw_os_error().unwrap())
            }
        }
        */
    }

    fn opendir(&self, _req: RequestInfo, path: &Path, _flags: u32) -> ResultOpen {
        debug!("opendir: {:?} (flags = {:#o})", path, _flags);
        //let real = self.real_path(path);
        Ok((0, 0))
    }

    fn readdir(&self, _req: RequestInfo, path: &Path, _fh: u64) -> ResultReaddir {
        debug!("readdir: {:?}", path);
        let cur_tags: HashSet<OsString> = path
            .components()
            .filter_map(|c| match c {
                Normal(t) => Some(t.to_os_string()),
                _ => None,
            })
            .collect();
        debug!("components: {:?}", cur_tags);

        let mut entries: Vec<DirectoryEntry> = vec![];
        for tag in &self.tags {
            if !cur_tags.contains(tag) {
                entries.push(DirectoryEntry {
                    name: tag.to_os_string(),
                    kind: FileType::Directory,
                });
            }
        }
        /*
        if cur_tags.len() > 0 {
            for entry in &self.entries {
                if entry.tags.is_superset(&cur_tags) {
                    debug!("match {:?}", entry);
                    entries.push(DirectoryEntry {
                        name: OsString::from(
                            format!("{:?} {:?}", entry.name, entry.absolute).replace("/", ":"),
                        ),
                        //name: entry.name.to_os_string(),
                        kind: FileType::RegularFile,
                    });
                }
            }
        }
        */
        /*
        let real = self.real_path(path);
        // Consider using libc::readdir to prevent need for always stat-ing entries
        let iter = match fs::read_dir(&real) {
            Ok(iter) => iter,
            Err(e) => return Err(e.raw_os_error().unwrap_or(ENOENT))
        };
        for entry in iter {
            match entry {
                Ok(entry) => {
                    let real_path = entry.path();
                    debug!("readdir: {:?} {:?}", real, real_path);
                    let stat = match libc_wrapper::lstat(real_path.clone()) {
                        Ok(stat) => stat,
                        Err(e) => return Err(e.raw_os_error().unwrap_or(ENOENT))
                    };
                    let filetype = DecoFS::stat_to_filetype(&stat);

                    entries.push(DirectoryEntry {
                        name: real_path.file_name().unwrap().to_os_string(),
                        kind: filetype,
                    });
                },
                Err(e) => {
                    error!("readdir: {:?}: {}", path, e);
                    return Err(e.raw_os_error().unwrap_or(ENOENT));
                }
            }
        }
        */
        info!("entries: {:?}", entries);
        Ok(entries)
    }

    fn listxattr(&self, _req: RequestInfo, path: &Path, size: u32) -> ResultXattr {
        debug!("listxattr({:?}, {})", path, size);

        if size == 0 {
			let size: usize = self.attrs.keys().map(|name| name.len()).sum();
            return Ok(Xattr::Size(size as u32));
        }
        print!(
            "{:?}",
            self.attrs.keys().map(|name| name.as_bytes())
                .collect::<Vec<_>>()
                .join(&0_u8)
        );
        //print!("{:?}", attrs.iter().flat_map(|attr| attr.as_bytes().to_vec().push(0_u8)).collect::<Vec<_>>());
        let mut data = self
            .attrs.keys().map(|name| name.as_bytes())
            .collect::<Vec<_>>()
            .join(&0_u8);
        data.push(0_u8);
        Ok(Xattr::Data(data))
    }

    fn getxattr(&self, _req: RequestInfo, path: &Path, name: &OsStr, size: u32) -> ResultXattr {
        debug!("getxattr: {:?} {:?} {}", path, name, size);

        if size == 0 {
            return Ok(Xattr::Size(
                self.attrs
                    .get(&name.to_str().unwrap_or(""))
                    .map_or(0, |a| a.len()) as u32,
            ));
        }

        let data = match self.attrs.get(&name.to_str().unwrap_or("")) {
            Some(&v) => v.as_bytes().to_vec(),
            _ => Vec::new(),
        };
        Ok(Xattr::Data(data))
    }

    fn setxattr(
        &self,
        _req: RequestInfo,
        path: &Path,
        name: &OsStr,
        value: &[u8],
        flags: u32,
        position: u32,
    ) -> ResultEmpty {
        debug!(
            "setxattr: {:?} {:?} {} bytes, flags = {:#x}, pos = {}",
            path,
            name,
            value.len(),
            flags,
            position
        );
        Err(libc::ENODATA)
    }
}
