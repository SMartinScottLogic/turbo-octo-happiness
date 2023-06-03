use std::ffi::CString;
use std::io;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

pub fn fstat(fh: u64) -> io::Result<libc::stat> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();

    let result = unsafe { libc::fstat(fh as libc::c_int, stat.as_mut_ptr()) };
    if -1 == result {
        let e = io::Error::last_os_error();
        error!("fstat({:?}): {}", fh, e);
        Err(e)
    } else {
        let stat = unsafe { stat.assume_init() };
        Ok(stat)
    }
}

pub fn lstat(path: &PathBuf) -> io::Result<libc::stat> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();

    let cstr = CString::new(path.clone().into_os_string().as_bytes())?;
    let result = unsafe { libc::lstat(cstr.as_ptr(), stat.as_mut_ptr()) };
    if -1 == result {
        let e = io::Error::last_os_error();
        error!("lstat({:?}): {}", path, e);
        Err(e)
    } else {
        let stat = unsafe { stat.assume_init() };
        Ok(stat)
    }
}

pub fn statfs(path: PathBuf) -> io::Result<libc::statfs> {
    let mut stat = MaybeUninit::<libc::statfs>::zeroed();

    let cstr = CString::new(path.clone().into_os_string().as_bytes())?;
    let result = unsafe { libc::statfs(cstr.as_ptr(), stat.as_mut_ptr()) };

    if -1 == result {
        let e = io::Error::last_os_error();
        error!("statfs({:?}): {}", path, e);
        Err(e)
    } else {
        let stat = unsafe { stat.assume_init() };
        Ok(stat)
    }
}
