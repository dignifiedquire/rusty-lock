extern crate libc;

use std::ffi::CString;
use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;

// For some reason these are not defined in libc
// Taken from fcntl.h
const	F_RDLCK: libc::c_short = 1;	// shared or read lock
const	F_UNLCK: libc::c_short = 2;	// unlock
const	F_WRLCK: libc::c_short = 3;	// exclusive or write lock

pub fn duplicate(file: &File) -> Result<File> {
    unsafe {
        let fd = libc::dup(file.as_raw_fd());

        if fd < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(File::from_raw_fd(fd))
        }
    }
}

pub fn lock_shared(file: &File) -> Result<()> {
    fcntl(file, libc::F_SETLKW, F_RDLCK)
}

pub fn lock_exclusive(file: &File) -> Result<()> {
    fcntl(file, libc::F_SETLKW, F_WRLCK)
}

pub fn try_lock_shared(file: &File) -> Result<()> {
    fcntl(file, libc::F_SETLK, F_RDLCK)
}

pub fn try_lock_exclusive(file: &File) -> Result<()> {
    fcntl(file, libc::F_SETLK, F_WRLCK)
}

pub fn unlock(file: &File) -> Result<()> {
    fcntl(file, libc::F_SETLK, F_UNLCK)
}

pub fn lock_error() -> Error {
    Error::from_raw_os_error(libc::EWOULDBLOCK)
}

fn fcntl(file: &File, flag: libc::c_int, l_type: libc::c_short) -> Result<()> {
    println!("fnctl");
     let mut fl: libc::flock = unsafe { mem::zeroed() };

    fl.l_type = l_type;
    fl.l_whence = libc::SEEK_SET as libc::c_short;

    let ret = unsafe {
        libc::fcntl(file.as_raw_fd(), flag, &fl as *const libc::flock)
    };

    println!("return {}", ret);
    if ret < 0 { Err(Error::last_os_error()) } else { Ok(()) }
}

pub fn allocated_size(file: &File) -> Result<u64> {
    file.metadata().map(|m| m.blocks() as u64 * 512)
}

#[cfg(any(target_os = "linux",
          target_os = "freebsd",
          target_os = "android",
          target_os = "nacl"))]
pub fn allocate(file: &File, len: u64) -> Result<()> {
    let ret = unsafe { libc::posix_fallocate(file.as_raw_fd(), 0, len as libc::off_t) };
    if ret == 0 { Ok(()) } else { Err(Error::last_os_error()) }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn allocate(file: &File, len: u64) -> Result<()> {
    let stat = try!(file.metadata());

    if len > stat.blocks() as u64 * 512 {
        let mut fstore = libc::fstore_t {
            fst_flags: libc::F_ALLOCATECONTIG,
            fst_posmode: libc::F_PEOFPOSMODE,
            fst_offset: 0,
            fst_length: len as libc::off_t,
            fst_bytesalloc: 0,
        };

        let ret = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_PREALLOCATE, &fstore) };
        if ret == -1 {
            // Unable to allocate contiguous disk space; attempt to allocate non-contiguously.
            fstore.fst_flags = libc::F_ALLOCATEALL;
            let ret = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_PREALLOCATE, &fstore) };
            if ret == -1 {
                return Err(Error::last_os_error());
            }
        }
    }

    if len > stat.size() as u64 {
        file.set_len(len)
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "openbsd",
          target_os = "netbsd",
          target_os = "dragonfly",
          target_os = "solaris"))]
pub fn allocate(file: &File, len: u64) -> Result<()> {
    // No file allocation API available, just set the length if necessary.
    if len > try!(file.metadata()).len() as u64 {
        file.set_len(len)
    } else {
        Ok(())
    }
}

fn statvfs<P>(path: P) -> Result<libc::statvfs> where P: AsRef<Path> {
    let cstr = match CString::new(path.as_ref().as_os_str().as_bytes()) {
        Ok(cstr) => cstr,
        Err(..) => return Err(Error::new(ErrorKind::InvalidInput, "path contained a null")),
    };

    unsafe {
        let mut stat: libc::statvfs = mem::zeroed();
        // danburkert/fs2-rs#1: cast is necessary for platforms where c_char != u8.
        if libc::statvfs(cstr.as_ptr() as *const _, &mut stat) == -1 {
            Err(Error::last_os_error())
        } else {
            Ok(stat)
        }
    }
}

pub fn free_space<P>(path: P) -> Result<u64> where P: AsRef<Path> {
    statvfs(path).map(|statvfs| statvfs.f_frsize as u64 * statvfs.f_bfree as u64)
}

pub fn available_space<P>(path: P) -> Result<u64> where P: AsRef<Path> {
    statvfs(path).map(|statvfs| statvfs.f_frsize as u64 * statvfs.f_bavail as u64)
}

pub fn total_space<P>(path: P) -> Result<u64> where P: AsRef<Path> {
    statvfs(path).map(|statvfs| statvfs.f_frsize as u64 * statvfs.f_blocks as u64)
}

pub fn allocation_granularity<P>(path: P) -> Result<u64> where P: AsRef<Path> {
    statvfs(path).map(|statvfs| statvfs.f_frsize as u64)
}

#[cfg(test)]
mod test {
    extern crate tempdir;
    extern crate libc;

    use std::fs::{self, File};
    use std::os::unix::io::AsRawFd;

    use {FileExt, lock_contended_error};

    /// The duplicate method returns a file with a new file descriptor.
    #[test]
    fn duplicate_new_fd() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();
        assert!(file1.as_raw_fd() != file2.as_raw_fd());
    }

    /// The duplicate method should preservesthe close on exec flag.
    #[test]
    fn duplicate_cloexec() {

        fn flags(file: &File) -> libc::c_int {
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL, 0) }
        }

        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();

        assert_eq!(flags(&file1), flags(&file2));
    }

    /// Tests that locking a file descriptor will replace any existing locks
    /// held on the file descriptor.
    #[test]
    fn lock_replace() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();
        let file2 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();

        // Creating a shared lock will drop an exclusive lock.
        file1.lock_exclusive().unwrap();
        file1.lock_shared().unwrap();
        file2.lock_shared().unwrap();

        // Attempting to replace a shared lock with an exclusive lock will fail
        // with multiple lock holders, and remove the original shared lock.
        assert_eq!(file2.try_lock_exclusive().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());
        file1.lock_shared().unwrap();
    }

    /// Tests that locks are shared among duplicated file descriptors.
    #[test]
    fn lock_duplicate() {
        let tempdir = tempdir::TempDir::new("fs2").unwrap();
        let path = tempdir.path().join("fs2");
        let file1 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();
        let file2 = file1.duplicate().unwrap();
        let file3 = fs::OpenOptions::new().write(true).create(true).open(&path).unwrap();

        // Create a lock through fd1, then replace it through fd2.
        file1.lock_shared().unwrap();
        file2.lock_exclusive().unwrap();
        assert_eq!(file3.try_lock_shared().unwrap_err().raw_os_error(),
                   lock_contended_error().raw_os_error());

        // Either of the file descriptors should be able to unlock.
        file1.unlock().unwrap();
        file3.lock_shared().unwrap();
    }
}
