//! Resource limits, not a filesystem or network sandbox. The parser has no network
//! implementation; OS-enforced offline confinement remains a separate milestone.

#[cfg(unix)]
pub fn apply() -> std::io::Result<()> {
    fn limit(resource: libc::__rlimit_resource_t, value: libc::rlim_t) -> std::io::Result<()> {
        let limits = libc::rlimit {
            rlim_cur: value,
            rlim_max: value,
        };
        // SAFETY: valid stack-allocated rlimit; affects only this short-lived worker.
        if unsafe { libc::setrlimit(resource, &limits) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    limit(libc::RLIMIT_AS, 1024 * 1024 * 1024)?;
    limit(libc::RLIMIT_CPU, 12)?;
    limit(libc::RLIMIT_CORE, 0)?;
    Ok(())
}

#[cfg(windows)]
pub struct Job(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: handle was created by CreateJobObjectW and is owned by this guard.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
pub fn apply() -> std::io::Result<Job> {
    use windows_sys::Win32::System::{JobObjects::*, Threading::GetCurrentProcess};
    // SAFETY: no name/attributes; returned handle is checked, owned, and closed by Job.
    let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let job = Job(handle);
    // SAFETY: POD Windows ABI struct, zero initialization is valid before setting flags.
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_PROCESS_TIME
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
    info.ProcessMemoryLimit = 512 * 1024 * 1024;
    info.BasicLimitInformation.PerProcessUserTimeLimit = 12 * 10_000_000;
    info.BasicLimitInformation.ActiveProcessLimit = 1;
    // SAFETY: correctly sized input structure and valid handles; assigns this worker only.
    let configured = unsafe {
        SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        )
    };
    if configured == 0 || unsafe { AssignProcessToJobObject(handle, GetCurrentProcess()) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Suppress crash/critical-error dialogs. Reader failures return through the pipe.
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode(0x0001 | 0x0002 | 0x8000);
    }
    Ok(job)
}
