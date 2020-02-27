use std::{
    ffi::{CStr, CString},
    path::{Component, Path, PathBuf},
};

#[cfg(unix)]
fn get_home_dir(usr: Option<&str>) -> Option<String> {
    // FIXME: what if something in buf gets truncated?
    // TODO: document usages of unsafe
    let mut buf = [0; 1 << 8];
    let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
    let mut ptr = std::ptr::null_mut();

    let ret = match usr {
        None => {
            // getuid is thread safe and always succeeds
            let uid = unsafe { libc::getuid() };

            unsafe { libc::getpwuid_r(uid, &mut pwd, buf.as_mut_ptr(), buf.len(), &mut ptr) }
        }
        Some(usr) => {
            let c_usr = CString::new(usr).ok()?;
            unsafe {
                libc::getpwnam_r(
                    c_usr.as_ptr(),
                    &mut pwd,
                    buf.as_mut_ptr(),
                    buf.len(),
                    &mut ptr,
                )
            }
        }
    };

    if ret != 0 || ptr.is_null() {
        return None;
    }

    let ret = unsafe { CStr::from_ptr(pwd.pw_dir) };
    ret.to_str().ok().map(|s| s.to_owned())
}

#[cfg(windows)]
fn get_home_dir(usr: Option<&str>) -> Option<String> {
    match usr {
        Some(_) => None,
        None => std::env::var("USERPROFILE").ok(),
    }
}

pub fn tilde_expand<P>(path: P) -> PathBuf
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(prefix)) => {
            // can't do anything with OsStr
            match prefix.to_str() {
                Some(prefix) if prefix.starts_with('~') => {
                    let name = &prefix[1..];
                    match get_home_dir(if name.is_empty() { None } else { Some(name) }) {
                        Some(home) => Path::new(&home).join(components),
                        None => path.to_owned(),
                    }
                }
                _ => path.to_owned(),
            }
        }
        Some(_) | None => path.to_owned(),
    }
}

// FIXME: TODO: tests
