/*
fuse: failed to access mountpoint /home/rein/reddit: Transport endpoint is not connected
fusermount -u ~/reddit
*/

use fuser::{FileAttr, Filesystem, ReplyAttr, ReplyEntry, Request, FileType, ReplyData};
use std::path::Path;
use anyhow::Result;
use std::ffi::OsStr;
use libc;
use std::time::Duration;
use std::time::SystemTime;
use libc::{ENOENT,EIO};
use lazy_static::lazy_static;
use orca::App as RedditCLient;
use orca::Sort as RedditSort;
use std::collections::HashMap;
use orca::LimitMethod;

struct RedditFS {
    reddit: orca::App,
    files: HashMap<String, File>,
    last_inode: u64
}

impl RedditFS {
    pub fn new() -> RedditFS {
        RedditFS {
            reddit: RedditCLient::new(
                "reddit-fs",
                env!("CARGO_PKG_VERSION"),
                "LevitatingBusinessMan",
            ).unwrap(),
            files: HashMap::new(),
            last_inode: 3
        }
    }
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
}

#[derive(Debug)]
struct File {
    content: Option<String>,
    attr: FileAttr,
}

const TTL: Duration = Duration::from_secs(1);

/// https://libfuse.github.io/doxygen/structfuse__lowlevel__ops.html
impl Filesystem for RedditFS {
    /// Look up a directory entry by name and get its attributes.
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_str().unwrap();
        debug!("lookup: parent {:?} {:?}", parent, name);

        match parent {
            // Fetch a subreddit
            1 => {
                if name == "README.txt" {
                    reply.entry(&TTL, &README_FILE_ATTR, 0);
                    return;
                } else if !name.contains(".") {
                    let attr = self.create_subreddit_directory(name);
                    reply.entry(&TTL, &attr, 0);
                } else {
                    reply.error(ENOENT);
                }
            },

            // Fetch a post
            3 => {
                let file = self.files.get(name);
                if let Some(file) = file {
                    reply.entry(&TTL, &file.attr, 0);
                } else {
                    reply.error(ENOENT);
                }
            },
            _ => reply.error(ENOENT),
        }

    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr: ino {:?}", ino);
        match ino {
            1 => reply.attr(&TTL, &REDDIT_DIR_ATTR),
            2 => reply.attr(&TTL, &README_FILE_ATTR),
            3 => reply.attr(&TTL, &SUBREDDIT_DIR_ATTR),
            _ => reply.error(ENOENT),
        }
    }

    fn readdir(&mut self, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, mut reply: fuser::ReplyDirectory) {
        debug!("readdir: ino {:?} offset {:?}", ino, offset);
        
        // So the offsets can be added later
        let mut entries: Vec<(u64, FileType, String)> = vec![
            (1, FileType::Directory, ".".to_owned()),
            (1, FileType::Directory, "..".to_owned()),
        ];

        match ino {
            1 => entries.push((2, FileType::RegularFile, "README.txt".to_owned())),
            _ => {
                // 25 results and . ..
                if offset >= 27 {
                    reply.ok();
                    return;
                }

                if let Some((sub, _file)) = self.files.iter().find(|(_k,file)| file.attr.ino == ino) {
                    debug!("ino {} is {}", ino, sub);
                    
                    let fetch_result = self.reddit.get_posts(sub, RedditSort::Hot);
                    match fetch_result {
                        Ok(res) => {
                            let posts = res.get("data").unwrap().get("children").unwrap();
                            for post in posts.as_array().unwrap() {
                                let (id, attr) = self.create_post_file(post);
                                entries.push((attr.ino, FileType::RegularFile, id));
                            }
                        },
                        Err(_err) => {
                            reply.error(EIO);
                            eprint!("Reddit request failed");
                            return;
                        }
                    }
                } else {
                    reply.error(ENOENT);
                    return;
                }
            }
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            // i + 1 means the index of the next entry
            if reply.add(entry.0, (i + 1) as i64, entry.1, entry.2) {
                break
            };
        }

        reply.ok();
    }

    fn read(&mut self, _req: &Request<'_>, ino: u64,_fh: u64, _offset: i64, _size: u32, _flags: i32, _lock_owner: Option<u64>, reply: ReplyData) {
        debug!("read: ino {:?} offset {:?}", ino, _offset);
        if ino == 2 {
            reply.data(README_TEXT.as_bytes());
        } else {
            if let Some((_key, file)) = self.files.iter().find(|(_k,file)| file.attr.ino == ino) {
                // FIXME: A newline is added to these bytes but it's never displayed
                reply.data(file.content.as_ref().unwrap().to_owned().as_bytes());
            } else {
                reply.error(ENOENT);
            }
        }
    }
}

impl RedditFS {
    // TODO: error handling
    fn create_post_file(&mut self, post: &serde_json::Value) -> (String, FileAttr) {
        let kind = post.get("kind").unwrap().as_str().unwrap();
        let data = post.get("data").unwrap();
        let _id = data.get("id").unwrap().as_str().unwrap().to_owned();
        let title = data.get("title").unwrap().as_str().unwrap().to_owned();

        if let Some(file) = self.files.get(&title) {
            return (title, file.attr)
        }

        //TODO: edge cases
        let content = (match kind {
            "t3" => data.get("url").unwrap(),
            _ => data.get("selftext").unwrap()
        }).as_str().unwrap();

        self.last_inode += 1;
        let attr = FileAttr {
            ino: self.last_inode,
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

        self.files.insert(
            title.clone(),
            File {
                content: Some(content.to_string() + "\n"),
                attr: attr,
            },
        );

        debug!("saved post {}", &title);
        (title, attr)
    }

    // TOOD: Make sure these subs exist
    fn create_subreddit_directory(&mut self, sub: &str) -> FileAttr {
        let sub = sub.to_owned();

        if let Some(file) = self.files.get(&sub) {
            return file.attr
        }

        self.last_inode += 1;
        let attr = FileAttr {
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

        debug!("saved sub {}", &sub);

        self.files.insert(
            sub,
            File {
                content: None,
                attr: attr,
            },
        );

        attr
    }

}

fn main() -> Result<()> {
    let fs = RedditFS::new();
    fs.reddit.set_ratelimiting(LimitMethod::Steady);
    fuser::mount2(fs, &Path::new("/home/rein/reddit"), &[])?;
    Ok(())
}
