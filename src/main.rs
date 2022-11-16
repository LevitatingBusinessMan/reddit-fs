use fuser::{FileAttr, Filesystem, ReplyAttr, ReplyEntry, Request, FileType, ReplyData};
use std::path::Path;
use anyhow::Result;
use std::ffi::OsStr;
use libc;
use std::time::Duration;
use std::time::{UNIX_EPOCH, SystemTime};
use libc::ENOENT;
use lazy_static::lazy_static;
use orca::App as RedditCLient;
use orca::Sort as RedditSort;
use std::collections::HashMap;
use std::sync::Mutex;

struct RedditFS {
    reddit: orca::App
}

static README_TEXT: &'static str = "Reddit filesystem\n";

macro_rules! debug {
    ($($arg:tt)*) => ({
        if cfg!(debug_assertions) {
            eprintln!($($arg)*);
        }
    })
}

lazy_static! {
    static ref REDDIT_DIR_ATTR: FileAttr = FileAttr {
        ino: 1,
        size: 0,
        blocks: 0,
        atime: SystemTime::now(),
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
        atime: SystemTime::now(),
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

    static ref SUBREDDIT_DIR_ATTR: FileAttr = FileAttr {
        ino: 3,
        size: 0,
        blocks: 0,
        atime: SystemTime::now(),
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

    static ref files: Mutex<HashMap<String, File>> = {let mut m = HashMap::new();Mutex::new(m)};
}

static mut last_inode: u64 = 3;

struct File {
    content: String,
    attr: FileAttr,
}

const TTL: Duration = Duration::from_secs(1);

//https://libfuse.github.io/doxygen/structfuse__lowlevel__ops.html
impl Filesystem for RedditFS {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_str().unwrap();
        debug!("lookup: parent {:?} {:?}", parent, name);

        if parent == 1 {
            if name == "README.txt" {
                reply.entry(&TTL, &README_FILE_ATTR, 0);
                return;
            } else if !name.contains(".") {
                reply.entry(&TTL, &SUBREDDIT_DIR_ATTR, 0);
                return;
                let fetch_result = self.reddit.get_posts(name, RedditSort::Hot);
                match fetch_result {
                    Ok(res) => {
                        let posts = res.get("data").unwrap().get("children").unwrap();
                        /* for post in posts.as_array().unwrap() {
                            reply.entry(&TTL, &create_post_file(post), 0);
                        } */ // This code should be moved to readdir instead. Lookup should return the subreddit as a directory.
                    },
                    Err(err) => {
                        eprint!("{}", "Request failed");
                    }
                }
                return;
            }
        }

        reply.error(ENOENT);
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr: ino {:?}", ino);
        match ino {
            1 => reply.attr(&TTL, &REDDIT_DIR_ATTR),
            2 => reply.attr(&TTL, &README_FILE_ATTR),
            _ => reply.error(ENOENT),
        }
    }

    fn readdir(&mut self, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, mut reply: fuser::ReplyDirectory) {
        debug!("readdir: ino {:?} offset {:?}", ino, offset);
        
        let mut entries = vec![
            (1, FileType::Directory, "."),
            (1, FileType::Directory, ".."),
        ];

        match ino {
            1 => entries.push((2, FileType::RegularFile, "README.txt")),
            3 => {
                let fetch_result = self.reddit.get_posts("linux", RedditSort::Hot);
                match fetch_result {
                    Ok(res) => {
                        let posts = res.get("data").unwrap().get("children").unwrap();
                        let mut last_inode2 = 3;
                        let mut i: i64 = 0;
                        for post in posts.as_array().unwrap() {
                            let id = post.get("data").unwrap().get("id").unwrap().as_str().unwrap().to_owned();
                            reply.add(last_inode2, i + 1, FileType::RegularFile, &id);
                            last_inode2 += 1;
                            i += 1;
                        }
                        reply.ok();
                        return;
                    },
                    Err(err) => {
                        eprint!("{}", "Reddit request failed");
                    }
                }
            },
            _ => {
                reply.error(ENOENT);
                return;
            }
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break;
            }
        }

        reply.ok();
    }

    fn read(&mut self, _req: &Request<'_>, ino: u64,_fh: u64, _offset: i64, _size: u32, _flags: i32, lock_owner: Option<u64>, reply: ReplyData) {
        debug!("read: ino {:?} offset {:?}", ino, _offset);
        if ino == 2 {
            reply.data(README_TEXT.as_bytes());
        } else {
            reply.error(ENOENT);
        }
    }
}

fn create_post_file(post: &serde_json::Value) -> FileAttr {
    let kind = post.get("kind").unwrap();
    let data = post.get("data").unwrap();
    let id = data.get("id").unwrap();

    //TODO: edge cases
    let content = (if kind == "t3" {data.get("url").unwrap()} else {data.get("selftext").unwrap()}).to_string();

    unsafe {
        last_inode += 1;
        let attr = FileAttr {
            ino: last_inode,
            size: content.len() as u64,
            blocks: 1,
            atime: SystemTime::now(),
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

        let mut files_map = files.lock().unwrap();
        files_map.insert(id.to_string(), File {
            content: content.to_string(),
            attr: attr,
        });

        attr
    }
}

fn main() -> Result<()> {
    let fs = RedditFS {
        reddit: RedditCLient::new("reddit-fs", env!("CARGO_PKG_VERSION"), "LevitatingBusinessMan").unwrap()
    };
    fuser::mount2(fs, &Path::new("/home/rein/reddit"), &[])?;
    Ok(())
}