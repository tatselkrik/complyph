use std::{io, path::Path};

#[cfg(not(windows))]
pub fn secure_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

#[cfg(windows)]
pub fn secure_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::CreateDirectoryW,
        System::Threading::*,
    };
    struct Allocation(*mut std::ffi::c_void);
    impl Drop for Allocation {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let mut token = std::ptr::null_mut();
    // SAFETY: valid out pointer; this process's query-only token is held by Token.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Token(token);
    let mut needed = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut needed);
    }
    // u64 alignment is sufficient for TOKEN_USER and its embedded SID.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut sid_text = std::ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let sid_allocation = Allocation(sid_text.cast());
    let mut length = 0;
    while unsafe { *sid_text.add(length) } != 0 {
        length += 1;
    }
    let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(sid_text, length) })
        .map_err(io::Error::other)?;
    drop(sid_allocation);
    let descriptor: Vec<u16> =
        format!("O:{sid}D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{sid})")
            .encode_utf16()
            .chain(Some(0))
            .collect();
    let mut security = std::ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            1,
            &mut security,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let security = Allocation(security);
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // Atomic restrictive creation: there is no public-readable staging interval.
    if unsafe { CreateDirectoryW(wide.as_ptr(), &attributes) } != 0 {
        return Ok(());
    }
    let error = unsafe { GetLastError() };
    if error != ERROR_ALREADY_EXISTS {
        return Err(io::Error::from_raw_os_error(error as i32));
    }
    if !path.is_dir() {
        return Err(io::Error::other("The workspace path is not a directory."));
    }
    let mut existing_security = std::ptr::null_mut();
    let mut owner = std::ptr::null_mut();
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut existing_security,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _existing_security = Allocation(existing_security);
    if unsafe { EqualSid(owner, user.User.Sid) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "This machine workspace belongs to another Windows account. Use its owning account or have an administrator configure access; ComplyPH will not take ownership.",
        ));
    }
    let mut dacl = std::ptr::null_mut();
    let mut present = 0;
    let mut defaulted = 0;
    if unsafe { GetSecurityDescriptorDacl(security.0, &mut present, &mut dacl, &mut defaulted) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    let status = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            dacl,
            std::ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}
