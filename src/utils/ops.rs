use crate::capabilities::{NewPath, OriginalPath};
use std::io::Write;
use std::path::Path;
use std::{fs, io};

pub trait FileOps {
    /// Rename files and directories. Works only within the same file system.
    ///
    /// Can also be used to move files and directories, when the target path is not the same.
    fn rename_move_consuming(&mut self);

    /// Copy files to a new location. Works between file systems.
    fn copy_consuming(&mut self);

    /// Move files to a new location by copying and removing the original. Works between file systems.
    fn cross_move_consuming(&mut self);
}

impl<M: OriginalPath + NewPath> FileOps for Vec<M> {
    fn rename_move_consuming(&mut self) {
        files_op(self, silent, |p, q| fs::rename(p, q))
    }

    fn copy_consuming(&mut self) {
        files_op(self, verbose, |p, q| copy_path(p, q, false, 0))
    }

    fn cross_move_consuming(&mut self) {
        files_op(self, verbose, |p, q| copy_path(p, q, true, 0))
    }
}

fn files_op(
    paths: &mut Vec<impl OriginalPath + NewPath>,
    notify: fn(&[u8]),
    op: fn(&Path, &Path) -> io::Result<()>,
) {
    paths.retain(|m| {
        let target = m.new_path();
        if target.exists() {
            notify(b"-\n");
            eprintln!("file already exists: {:?} -> {target:?}", m.path());
            notify(b"\n");
            return true;
        }
        match op(m.path(), &target) {
            Ok(()) => false,
            Err(err) => {
                notify(b"x\n");
                eprintln!("error: {err}: {:?} -> {target:?}", m.path());
                notify(b"\n");
                true
            }
        }
    });
    notify(b"\n");
}

// `n` is just a counter for verbose output.
fn copy_path(p: &Path, q: &Path, remove_dir: bool, n: usize) -> io::Result<()> {
    if p.is_dir() {
        fs::create_dir(q).and_then(|()| {
            verbose(b"d[");
            let files = fs::read_dir(p)?
                .flatten()
                .try_fold(Vec::new(), |mut acc, de| {
                    let is_dir = de.path().is_dir(); // need to cache because is_dir goes to the fs again, and copy_path may have removed it.
                    copy_path(&de.path(), &q.join(de.file_name()), remove_dir, n + 1).map(|()| {
                        if !is_dir {
                            verbose(b".");
                            if remove_dir {
                                acc.push(de.path())
                            }
                        }
                        acc
                    })
                });
            verbose(b"]");
            if remove_dir {
                files
                    .and_then(|files| files.iter().try_for_each(fs::remove_file))
                    .and_then(|()| fs::remove_dir(p))
            } else {
                files.map(|_| ())
            }
        })
    } else if n == 0 {
        fs::copy(p, q).and_then(|_| {
            verbose(b".");
            if remove_dir {
                fs::remove_file(p)?
            }
            Ok(())
        })
    } else {
        fs::copy(p, q).map(|_| ()) // this is called recursively by the is_dir case above.
    }
}

fn silent(_: &[u8]) {}
fn verbose(c: &[u8]) {
    io::stdout().write_all(c).unwrap();
    io::stdout().flush().unwrap();
}
