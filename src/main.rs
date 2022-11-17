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
use clap::Parser;

type Ino = u64;

/// Time to live for the cache in seconds
const CACHE_TTL: u64 = 120;

struct RedditFS {
    reddit: orca::App,
    files: HashMap<Ino, File>,
    last_inode: Ino
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
        //if cfg!(debug_assertions) {
            eprintln!($($arg)*);
        //}
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
}

#[derive(Clone, Debug)]
struct Post {
    id: String,
    content: Vec<u8>,
}

#[derive(Clone, Debug)]
struct Sub {
    posts: Option<Vec<Ino>>,
}

#[derive(Clone, Debug)]
enum FileKind {
    Sub(Sub),
    Post(Post)
}

#[derive(Clone, Debug)]
struct File {
    name: String,
    attr: FileAttr,
    kind: FileKind
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
                    let file = self.create_subreddit_directory(name);
                    reply.entry(&TTL, &file.attr, 0);
                } else {
                    reply.error(ENOENT);
                }
            },

            // Fetch a post
            _ => {
                if let Some(file) = self.files.get(&parent) {
                    match &file.kind {
                        FileKind::Sub(sub) => {
                            if let Some(posts) = &sub.posts {
                                if let Some(ino) = posts.iter().find(|ino| {
                                    if let Some(file) = self.files.get(ino) {
                                        file.name == name
                                    } else {
                                        false
                                    }
                                }) {
                                    reply.entry(&TTL, &self.files.get(ino).unwrap().attr, 0); 
                                    return;  
                                }
                            }
                        },
                        _ => unreachable!()
                    }
                }
                
                reply.error(ENOENT);
            },
        }

    }

    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        debug!("getattr: ino {:?}", ino);
        match ino {
            1 => reply.attr(&TTL, &REDDIT_DIR_ATTR),
            2 => reply.attr(&TTL, &README_FILE_ATTR),
            _ => {
                if let Some(file) = self.files.get(&ino) {
                    reply.attr(&TTL, &file.attr);
                    return;
                }

                reply.error(ENOENT);
            }
        }
    }

    fn readdir(&mut self, _req: &Request<'_>, ino: u64, _fh: u64, offset: i64, mut reply: fuser::ReplyDirectory) {
        debug!("readdir: ino {:?} offset {:?}", ino, offset);
        
        // So the offsets can be added later
        let mut entries: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".to_owned()),
            (0, FileType::Directory, "..".to_owned()),
        ];

        match ino {
            1 => {
                entries.push((2, FileType::RegularFile, "README.txt".to_owned()));

                for (ino, file) in self.files.iter() {
                    if let FileKind::Sub(sub) = &file.kind {
                        if sub.posts.is_some() {
                            entries.push((*ino, FileType::Directory, file.name.to_owned()))
                        }
                    }
                }

            },
            _ => {
                // 25 results and . ..
                if offset >= 27 {
                    reply.ok();
                    return;
                }

                match self.files.get(&ino) {
                    Some(file) => {
                        debug!("ino {} is {}", ino, file.name);


                        if let FileKind::Sub(sub) = &file.kind {
                            // Refresh cache
                            if sub.posts.is_none() || SystemTime::now().duration_since(file.attr.mtime).unwrap() > Duration::new(CACHE_TTL, 0) {
                                let fetch_result = self.reddit.get_posts(&file.name, RedditSort::Hot);
                                match fetch_result {
                                    Ok(res) => {
                                        let mut sub = (*sub).clone();
                                        let mut file = (*file).clone();

                                        let mut inos = vec![];
                                        let posts = res.get("data").unwrap().get("children").unwrap();
                                        for post in posts.as_array().unwrap() {
                                            let postfile = self.create_post_file(post);
                                            inos.push(postfile.attr.ino);
                                            entries.push((postfile.attr.ino, FileType::RegularFile, postfile.name));
                                        }

                                        file.attr.mtime = SystemTime::now();
                                        sub.posts = Some(inos);
                                        file.kind = FileKind::Sub(sub);
                                        self.files.insert(ino,file);
                                    },
                                    Err(_err) => {
                                        reply.error(EIO);
                                        eprint!("Reddit request failed");
                                        return;
                                    }
                                }
                            } else {
                                // use inos
                                if let Some(inos) = &sub.posts {
                                    for ino in inos {
                                        if let Some(file) = self.files.get(&ino) {
                                            entries.push((*ino, FileType::RegularFile, file.name.clone()));
                                        }
                                    }
                                } else {
                                    unreachable!()
                                }
                            }//
                        } else {
                            unreachable!()
                        }
                    },
                    None => {
                        reply.error(ENOENT);
                        return;
                    }
                }
            }
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset as usize) {
            //debug!("reply ino {} offset {} name {} ", entry.0, i + 1, entry.2);
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
            if let Some(file) = self.files.get(&ino) {
                if let FileKind::Post(post) = &file.kind {
                    reply.data(&post.content)
                }
            } else {
                reply.error(ENOENT);
            }
        }
    }
}

impl RedditFS {
    // TODO: error handling
    fn create_post_file(&mut self, post: &serde_json::Value) -> File {
        let kind = post.get("kind").unwrap().as_str().unwrap();
        let data = post.get("data").unwrap();
        let id = data.get("id").unwrap().as_str().unwrap().to_owned();
        let mut title = data.get("title").unwrap().as_str().unwrap().to_owned();

        //There are probably more broken titles I have to hunt
        if [".",".."].contains(&title.as_str()) {
            title = format!("\"{}\"", title);
        }
        
        //Make slashes disappear
        title = title.replace("/", "");

        //Titles can be duplicate, something to consider (only an issue if duplicate within the same sub)
        if let Some((_ino, file)) = self.files.iter().find(|(_ino, file)| {
            if let FileKind::Post(post) = &file.kind {
                post.id == id
            } else {
                false
            }
        }) {
            return file.clone()
        }

        //TODO: edge cases
        //https://www.reddit.com/dev/api/#listings
        let mut content = (match kind {
            "t3" => {
                let url = data.get("url").unwrap();
                if url.as_str().unwrap().starts_with("https://www.reddit.com/r/") {
                    data.get("selftext").unwrap()
                } else {url}
            },
            _ => data.get("selftext").unwrap()
        }).as_str().unwrap().to_string();

        if !content.is_empty() {
            content += "\n";
        }

        let content = content.into_bytes();

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
            self.last_inode,
            File {
                name: title.clone(),
                attr,
                kind: FileKind::Post(Post {id: id.clone(), content: content.clone()})
            }
        );

        debug!("saved post {}", &title);
        File {
            name: title,
            attr,
            kind: FileKind::Post(Post {id,content})
        }
    }

    // TOOD: Make sure these subs exist
    fn create_subreddit_directory(&mut self, sub: &str) -> File {
        let sub = sub.to_owned();

        if let Some((_ino, file)) = self.files.iter().find(|(_ino, file)| {
            if let FileKind::Sub(_) = file.kind {
                file.name == sub
            } else {false}
        }) {
            return file.clone()
        }

        self.last_inode += 1;
        let attr = FileAttr {
            ino: self.last_inode,
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
            self.last_inode,
            File {
                name: sub.clone(),
                attr,
                kind: FileKind::Sub(Sub {posts: None})
            }
        );

        File {
            name: sub,
            attr,
            kind: FileKind::Sub(Sub {posts: None})
        }
    }

}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(help("Directory to mount on"))]
    directory: String
}

fn main() -> Result<()> {
    let args = Args::parse();
    let fs = RedditFS::new();
    fs.reddit.set_ratelimiting(LimitMethod::Steady);
    fuser::mount2(fs, &Path::new(&args.directory), &[])?;
    Ok(())
}
