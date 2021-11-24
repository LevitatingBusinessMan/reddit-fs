use fuser::{FileAttr, Filesystem, ReplyAttr, ReplyEntry, Request, FileType, ReplyData};
use std::path::Path;
use anyhow::Result;
use std::ffi::OsStr;
use libc;
use std::time::Duration;
use std::time::{UNIX_EPOCH, SystemTime};
use libc::ENOENT;
use lazy_static::lazy_static;

struct RedditFS;
static README_TEXT: &'static str = "Reddit filesystem\n";

lazy_static! {
    static ref REDDIT_DIR_ATTR: FileAttr = FileAttr {
        ino: 1,
        size: 0,
        blocks: 0,
        atime: SystemTime::now(), // 1970-01-01 00:00:00
        mtime: SystemTime::now(),
        ctime: SystemTime::now(),
        crtime: SystemTime::now(),
        kind: FileType::Directory,
        perm: 0o755,
        nlink: 2,
        uid: 501,
        gid: 20,
        rdev: 0,
        flags: 0,
        blksize: 512,
    };
    
    static ref README_FILE_ATTR: FileAttr = FileAttr {
        ino: 2,
        size: README_TEXT.len() as u64,
        blocks: 1,
        atime: SystemTime::now(), // 1970-01-01 00:00:00
        mtime: SystemTime::now(),
        ctime: SystemTime::now(),
        crtime: SystemTime::now(),
        kind: FileType::RegularFile,
        perm: 0o644,
        nlink: 1,
        uid: 501,
        gid: 20,
        rdev: 0,
        flags: 0,
        blksize: 512,
    };
}

const TTL: Duration = Duration::from_secs(1);

impl Filesystem for RedditFS {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let path = Path::new(name);
        let name = path.file_name().unwrap().to_str().unwrap();
        
        if parent == 1 && name == "README.txt" {
            reply.entry(&TTL, &README_FILE_ATTR, 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        match ino {
            1 => reply.attr(&TTL, &REDDIT_DIR_ATTR),
            2 => reply.attr(&TTL, &README_FILE_ATTR),
            _ => reply.error(ENOENT),
        }
    }
    fn readdir(&mut self, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, mut reply: fuser::ReplyDirectory) {
        if ino != 1 {
            reply.error(ENOENT);
            return;
        }

        let entries = vec![
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
            (2, FileType::RegularFile, "README.txt"),
        ];

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }
        reply.ok();
    }

    fn read(&mut self, _req: &Request<'_>, ino: u64,_fh: u64, _offset: i64, _size: u32, _flags: i32, lock_owner: Option<u64>, reply: ReplyData) {
        if ino == 2 {
            reply.data(README_TEXT.as_bytes());
        } else {
            reply.error(ENOENT);
        }
    }
}

fn main() -> Result<()> {
    fuser::mount2(RedditFS, &Path::new("/home/rein/reddit"), &[])?;
    Ok(())
}