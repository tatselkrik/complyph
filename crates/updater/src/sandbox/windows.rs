//! The network child receives pipe handles and Internet-client capability only.
//! Its executable is copied into a read/execute-only AppContainer-accessible directory.
use crate::protocol::{self, Reply, Request};
use std::{
    fs,
    io::{Read, Write},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{Authorization::*, Isolation::*, *},
    System::{JobObjects::*, Pipes::*, Threading::*},
};
const SE_GROUP_ENABLED: u32 = 0x00000004;

fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}
fn error(stage: &str) -> String {
    format!(
        "The isolated downloader could not {stage} (Windows error {}).",
        unsafe { GetLastError() }
    )
}
struct Sid(PSID);
impl Drop for Sid {
    fn drop(&mut self) {
        unsafe {
            FreeSid(self.0);
        }
    }
}
struct Local(*mut std::ffi::c_void);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}
fn text_sid(sid: PSID) -> Result<String, String> {
    let mut text = std::ptr::null_mut();
    // SAFETY: sid is a valid OS-provided SID. The returned allocation is released by Local.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(error("format its security identity"));
    }
    let _guard = Local(text.cast());
    let mut length = 0;
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) })
        .map_err(|_| "The downloader security identity was invalid UTF-16.".into())
}
fn token() -> Result<OwnedHandle, String> {
    let mut raw = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(error("open its security token"));
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}
fn owner_sid() -> Result<String, String> {
    let token = token()?;
    let mut needed = 0;
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(error("read its owner identity"));
    }
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    text_sid(user.User.Sid)
}
fn grant_runtime(path: &Path, container: PSID) -> Result<(), String> {
    let user = owner_sid()?;
    let container = text_sid(container)?;
    let sddl = wide(format!(
        "O:{user}D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;{user})(A;OICI;GRGX;;;{container})"
    ));
    let mut descriptor = std::ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(error("prepare its access policy"));
    }
    let guard = Local(descriptor);
    let (mut present, mut defaulted) = (0, 0);
    let mut dacl = std::ptr::null_mut();
    if unsafe { GetSecurityDescriptorDacl(guard.0, &mut present, &mut dacl, &mut defaulted) } == 0 {
        return Err(error("read its access policy"));
    }
    let result = unsafe {
        SetNamedSecurityInfoW(
            wide(path).as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            dacl,
            std::ptr::null_mut(),
        )
    };
    if result != 0 {
        return Err(format!(
            "The downloader directory could not be protected (Windows error {result})."
        ));
    }
    Ok(())
}

pub struct Launcher {
    directory: PathBuf,
    executable: PathBuf,
    container: Sid,
    capability: Vec<u64>,
}
impl Drop for Launcher {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
impl Launcher {
    pub fn new() -> Result<Self, String> {
        let profile = wide("ComplyPH.SourceUpdates");
        let mut sid = std::ptr::null_mut();
        let result = unsafe {
            CreateAppContainerProfile(
                profile.as_ptr(),
                profile.as_ptr(),
                profile.as_ptr(),
                std::ptr::null(),
                0,
                &mut sid,
            )
        };
        if result < 0 {
            if result as u32 != 0x800700B7 {
                return Err(format!(
                    "Windows could not create the source-check AppContainer (HRESULT 0x{:08X}). No online check was made.",
                    result as u32
                ));
            }
            if unsafe { DeriveAppContainerSidFromAppContainerName(profile.as_ptr(), &mut sid) } < 0
            {
                return Err(
                    "Windows could not resolve the existing source-check AppContainer.".into(),
                );
            }
        }
        if sid.is_null() {
            return Err("Windows returned an empty AppContainer identity.".into());
        }
        let container = Sid(sid);
        let mut capability = vec![0u64; 9];
        let mut size = (capability.len() * 8) as u32;
        if unsafe {
            CreateWellKnownSid(
                WinCapabilityInternetClientSid,
                std::ptr::null_mut(),
                capability.as_mut_ptr().cast(),
                &mut size,
            )
        } == 0
        {
            return Err(error("create its InternetClient identity"));
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "ComplyPH-downloader-{}-{unique}",
            std::process::id()
        ));
        complyph_paths::secure_directory(&directory)
            .map_err(|_| "The downloader directory could not be protected.".to_string())?;
        let executable = directory.join("ComplyPH-downloader.exe");
        let result = (|| {
            grant_runtime(&directory, container.0)?;
            let original_path = std::env::current_exe()
                .map_err(|e| format!("The updater executable could not be located: {e}"))?;
            fs::copy(&original_path, &executable)
                .map_err(|e| format!("The downloader executable could not be prepared: {e}"))?;
            grant_runtime(&executable, container.0)?;
            // Verify the runtime copy before granting it execution in the lowbox.
            use sha2::{Digest, Sha256};
            let original = fs::read(&original_path)
                .map_err(|e| format!("The updater executable could not be verified: {e}"))?;
            let copy = fs::read(&executable)
                .map_err(|e| format!("The downloader executable could not be verified: {e}"))?;
            if Sha256::digest(original) != Sha256::digest(copy) {
                return Err("The downloader executable failed its integrity check.".into());
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&directory);
            return Err(error);
        }
        let launcher = Self {
            directory,
            executable,
            container,
            capability,
        };
        let protected = launcher.directory.join("denial-probe");
        complyph_paths::secure_directory(&protected)
            .map_err(|_| "The isolation canary could not be protected.")?;
        let canary = protected.join("synthetic-canary.txt");
        fs::write(&canary, b"ComplyPH synthetic isolation canary")
            .map_err(|_| "The isolation canary could not be created.")?;
        match launcher.run(&Request::Probe {
            files: vec![canary],
        })? {
            (
                Reply::Probe {
                    app_container: true,
                    all_files_denied: true,
                    ..
                },
                _,
            ) => Ok(launcher),
            (Reply::Failed { message }, _) => {
                Err(format!("The downloader isolation check failed: {message}"))
            }
            _ => Err(
                "The downloader failed its runtime file-denial check. No source was contacted."
                    .into(),
            ),
        }
    }

    pub fn run(&self, request: &Request) -> Result<(Reply, Vec<u8>), String> {
        let input = serde_json::to_vec(request)
            .map_err(|_| "The public URL request could not be encoded.")?;
        if input.len() > 8192 {
            return Err("The public URL request exceeded its size limit.".into());
        }
        let (read_in, write_in) = pipe()?;
        let (read_out, write_out) = pipe()?;
        let stderr = fs::OpenOptions::new()
            .write(true)
            .open("NUL")
            .map_err(|e| format!("The downloader error stream could not be opened: {e}"))?;
        let inheritable = |h: HANDLE, value| {
            if unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT, value) } == 0 {
                Err(error("limit pipe inheritance"))
            } else {
                Ok(())
            }
        };
        inheritable(write_in.as_raw_handle(), 0)?;
        inheritable(read_out.as_raw_handle(), 0)?;
        inheritable(stderr.as_raw_handle(), HANDLE_FLAG_INHERIT)?;
        let mut capability = SID_AND_ATTRIBUTES {
            Sid: self.capability.as_ptr().cast_mut().cast(),
            Attributes: SE_GROUP_ENABLED,
        };
        let mut security = SECURITY_CAPABILITIES {
            AppContainerSid: self.container.0,
            Capabilities: &mut capability,
            CapabilityCount: 1,
            Reserved: 0,
        };
        let mut size = 0;
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut size);
        }
        let mut storage = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
        let list = storage.as_mut_ptr().cast();
        if unsafe { InitializeProcThreadAttributeList(list, 2, 0, &mut size) } == 0 {
            return Err(error("initialize its process attributes"));
        }
        struct Attributes(LPPROC_THREAD_ATTRIBUTE_LIST);
        impl Drop for Attributes {
            fn drop(&mut self) {
                unsafe {
                    DeleteProcThreadAttributeList(self.0);
                }
            }
        }
        let _attributes = Attributes(list);
        let mut handles = [
            read_in.as_raw_handle(),
            write_out.as_raw_handle(),
            stderr.as_raw_handle(),
        ];
        // The explicit handle list prevents inherited private/database handles.
        if unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                (&mut security as *mut SECURITY_CAPABILITIES).cast(),
                std::mem::size_of_val(&security),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        } == 0
            || unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    handles.as_mut_ptr().cast(),
                    std::mem::size_of_val(&handles),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                )
            } == 0
        {
            return Err(error("set its process attributes"));
        }
        let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        startup.StartupInfo.cb = std::mem::size_of_val(&startup) as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = read_in.as_raw_handle();
        startup.StartupInfo.hStdOutput = write_out.as_raw_handle();
        startup.StartupInfo.hStdError = stderr.as_raw_handle();
        startup.lpAttributeList = list;
        let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let exe = wide(&self.executable);
        let mut command = wide(format!(
            "\"{}\" --network-worker",
            self.executable.display()
        ));
        let mut environment = Vec::new();
        for name in ["SystemRoot", "WINDIR"] {
            if let Some(value) = std::env::var_os(name) {
                environment.extend(wide(format!("{name}={}", value.to_string_lossy())));
            }
        }
        environment.push(0u16);
        let job = job()?;
        let created = unsafe {
            CreateProcessW(
                exe.as_ptr(),
                command.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT,
                environment.as_ptr().cast(),
                wide(&self.directory).as_ptr(),
                &startup.StartupInfo,
                &mut process,
            )
        };
        if created == 0 {
            return Err(error("create its AppContainer process"));
        }
        let child = unsafe { OwnedHandle::from_raw_handle(process.hProcess) };
        let thread = unsafe { OwnedHandle::from_raw_handle(process.hThread) };
        if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } == 0 {
            let error = error("attach its process limits");
            unsafe {
                TerminateProcess(child.as_raw_handle(), 1);
            }
            return Err(error);
        }
        if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
            return Err(error("start its AppContainer process"));
        }
        drop(read_in);
        drop(write_out);
        drop(stderr);
        drop(thread);
        let writer = std::thread::spawn(move || {
            let mut file = write_in;
            file.write_all(&input)
        });
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            read_out
                .take(protocol::MAX_WIRE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        let start = Instant::now();
        let timed_out = loop {
            if unsafe { WaitForSingleObject(child.as_raw_handle(), 100) } == WAIT_OBJECT_0 {
                break false;
            }
            if start.elapsed() > Duration::from_secs(30) {
                unsafe {
                    TerminateJobObject(job.as_raw_handle(), 1);
                    TerminateProcess(child.as_raw_handle(), 1);
                    WaitForSingleObject(child.as_raw_handle(), 5000);
                }
                break true;
            }
        };
        let _ = writer.join();
        let bytes = reader
            .join()
            .map_err(|_| "The downloader output could not be collected.")?
            .map_err(|_| "The downloader output could not be read.")?;
        if timed_out {
            return Err("The source check exceeded its 30-second worker deadline.".into());
        }
        let mut code = 1;
        unsafe {
            GetExitCodeProcess(child.as_raw_handle(), &mut code);
        }
        if code != 0 {
            return Err(
                "The isolated downloader stopped safely. The source could not be checked.".into(),
            );
        }
        protocol::decode(&bytes)
    }
}

fn pipe() -> Result<(fs::File, fs::File), String> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write) = (std::ptr::null_mut(), std::ptr::null_mut());
    if unsafe { CreatePipe(&mut read, &mut write, &sa, 0) } == 0 {
        return Err(error("create its pipes"));
    }
    Ok(unsafe {
        (
            fs::File::from_raw_handle(read),
            fs::File::from_raw_handle(write),
        )
    })
}
fn job() -> Result<OwnedHandle, String> {
    let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw.is_null() {
        return Err(error("create its process limits"));
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_PROCESS_TIME;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    limits.BasicLimitInformation.PerProcessUserTimeLimit = 15 * 10_000_000;
    limits.ProcessMemoryLimit = 512 * 1024 * 1024;
    if unsafe {
        SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } == 0
    {
        return Err(error("configure its process limits"));
    }
    Ok(handle)
}

pub fn require_isolation() -> Result<(), String> {
    let token = token()?;
    let mut value = 0u32;
    let mut returned = 0;
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenIsAppContainer,
            (&mut value as *mut u32).cast(),
            4,
            &mut returned,
        )
    } == 0
        || value != 1
    {
        return Err("The downloader refused networking outside an AppContainer.".into());
    }
    let mut needed = 0;
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenCapabilities,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    if (needed as usize) < std::mem::size_of::<TOKEN_GROUPS>()
        || unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenCapabilities,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        } == 0
    {
        return Err("The downloader could not verify its capabilities.".into());
    }
    let groups = unsafe { &*buffer.as_ptr().cast::<TOKEN_GROUPS>() };
    if groups.GroupCount != 1 {
        return Err(format!(
            "The downloader requires InternetClient capability only (observed {}).",
            groups.GroupCount
        ));
    }
    let mut expected = vec![0u64; 9];
    let mut expected_size = (expected.len() * 8) as u32;
    if unsafe {
        CreateWellKnownSid(
            WinCapabilityInternetClientSid,
            std::ptr::null_mut(),
            expected.as_mut_ptr().cast(),
            &mut expected_size,
        )
    } == 0
        || unsafe { EqualSid(groups.Groups[0].Sid, expected.as_mut_ptr().cast()) } == 0
        || groups.Groups[0].Attributes & SE_GROUP_ENABLED == 0
    {
        return Err("The downloader refused an unexpected AppContainer capability.".into());
    }
    Ok(())
}
