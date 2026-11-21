//! Linux worker: deny-by-default Landlock filesystem + seccomp, before input/HTTP.
//! Only exact root-owned DNS/NSS configuration files are readable. No catalogs,
//! user directories, /proc, writable paths, Unix sockets or child executables.
use crate::protocol::{self, Reply, Request};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::MetadataExt, process::CommandExt},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
static ISOLATED: AtomicBool = AtomicBool::new(false);

fn failure(stage: &str) -> String {
    format!(
        "Linux source isolation could not {stage}: {}. No online request was made.",
        std::io::Error::last_os_error()
    )
}
pub fn require_isolation() -> Result<(), String> {
    if ISOLATED.load(Ordering::Acquire) {
        Ok(())
    } else {
        Err("The Linux worker must establish Landlock and seccomp before accepting a network request.".into())
    }
}
#[repr(C)]
struct Ruleset {
    handled_access_fs: u64,
}
#[repr(C, packed)]
struct PathBeneath {
    allowed_access: u64,
    parent_fd: i32,
}

pub fn enter_isolation() -> Result<(), String> {
    #[cfg(not(any(target_arch="x86_64",target_arch="aarch64")))]
    return Err("This Linux architecture has no reviewed downloader seccomp policy. Networking remains disabled.".into());
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        // SAFETY: the ABI query has a null attribute and zero length as required.
        let abi = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                std::ptr::null::<u8>(),
                0,
                1,
            )
        };
        if abi < 3 {
            return Err("Source checks require Linux Landlock ABI 3 or newer and seccomp. This kernel keeps networking disabled.".into());
        }
        let attr = Ruleset {
            handled_access_fs: (1 << 15) - 1,
        };
        // SAFETY: attr is the exact version-1 eight-byte ruleset structure.
        let ruleset = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &attr,
                std::mem::size_of::<Ruleset>(),
                0,
            )
        } as i32;
        if ruleset < 0 {
            return Err(failure("create its Landlock policy"));
        }
        let result = (|| {
            for path in [
                "/etc/resolv.conf",
                "/etc/hosts",
                "/etc/nsswitch.conf",
                "/etc/gai.conf",
                "/etc/host.conf",
                "/lib/x86_64-linux-gnu/libnss_dns.so.2",
                "/lib/x86_64-linux-gnu/libnss_files.so.2",
                "/lib/aarch64-linux-gnu/libnss_dns.so.2",
                "/lib/aarch64-linux-gnu/libnss_files.so.2",
            ] {
                let Ok(metadata) = fs::metadata(path) else {
                    continue;
                };
                if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
                    return Err(format!(
                        "Linux source isolation refused an unprotected DNS/NSS runtime file ({path}). Networking remains disabled."
                    ));
                }
                let name = std::ffi::CString::new(path).expect("fixed system path");
                // SAFETY: valid NUL-terminated fixed path; fd is closed below.
                let fd = unsafe { libc::open(name.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
                if fd < 0 {
                    return Err(failure("open its DNS runtime file"));
                }
                let rule = PathBeneath {
                    allowed_access: 1 << 2,
                    parent_fd: fd,
                };
                // SAFETY: ruleset/fd are live; rule is the packed kernel layout.
                let added =
                    unsafe { libc::syscall(libc::SYS_landlock_add_rule, ruleset, 1, &rule, 0) };
                unsafe {
                    libc::close(fd);
                }
                if added < 0 {
                    return Err(failure("allow its exact DNS runtime file"));
                }
            }
            if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
                return Err(failure("drop privilege escalation"));
            }
            if unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset, 0) } < 0 {
                return Err(failure("apply its filesystem boundary"));
            }
            install_seccomp()?;
            ISOLATED.store(true, Ordering::Release);
            Ok(())
        })();
        unsafe {
            libc::close(ruleset);
        }
        result
    }
}
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn install_seccomp() -> Result<(), String> {
    #[cfg(target_arch = "x86_64")]
    const ARCH: u32 = 0xc000003e;
    #[cfg(target_arch = "aarch64")]
    const ARCH: u32 = 0xc00000b7;
    fn instruction(code: u16, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
        libc::sock_filter { code, jt, jf, k }
    }
    let deny = 0x00050000 | libc::EPERM as u32;
    let mut filter = vec![
        instruction(0x20, 0, 0, 4),
        instruction(0x15, 1, 0, ARCH),
        instruction(0x06, 0, 0, 0x80000000),
        instruction(0x20, 0, 0, 0),
        // Reject alternate x32 syscall numbers before applying the native policy.
        instruction(0x35, 0, 1, 0x40000000),
        instruction(0x06, 0, 0, deny),
    ];
    let mut blocked = vec![
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_pidfd_getfd,
        libc::SYS_open_by_handle_at,
        libc::SYS_name_to_handle_at,
        libc::SYS_io_uring_setup,
        libc::SYS_io_uring_enter,
        libc::SYS_io_uring_register,
        libc::SYS_bpf,
        libc::SYS_mount,
        libc::SYS_umount2,
        libc::SYS_unshare,
        libc::SYS_setns,
        libc::SYS_execve,
        libc::SYS_execveat,
        libc::SYS_memfd_create,
        libc::SYS_userfaultfd,
        libc::SYS_perf_event_open,
        libc::SYS_keyctl,
        libc::SYS_add_key,
        libc::SYS_request_key,
        libc::SYS_socketpair,
    ];
    #[cfg(target_arch = "x86_64")]
    blocked.extend([libc::SYS_fork, libc::SYS_vfork]);
    for syscall in blocked {
        filter.push(instruction(0x15, 0, 1, syscall as u32));
        filter.push(instruction(0x06, 0, 0, deny));
    }
    // Network code may create Internet sockets only; no local daemon/Unix socket access.
    filter.extend([
        instruction(0x15, 0, 5, libc::SYS_socket as u32),
        instruction(0x20, 0, 0, 16),
        instruction(0x15, 2, 0, libc::AF_INET as u32),
        instruction(0x15, 1, 0, libc::AF_INET6 as u32),
        instruction(0x06, 0, 0, deny),
        instruction(0x20, 0, 0, 0),
        instruction(0x06, 0, 0, 0x7fff0000),
    ]);
    let program = libc::sock_fprog {
        len: filter.len() as u16,
        filter: filter.as_mut_ptr(),
    };
    // SAFETY: filter remains live for the call; the kernel copies it on installation.
    if unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            &program,
            0,
            0,
        )
    } != 0
    {
        return Err(failure("apply its seccomp boundary"));
    }
    Ok(())
}

pub struct Launcher;
impl Launcher {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }
    pub fn run(&self, request: &Request) -> Result<(Reply, Vec<u8>), String> {
        let bytes =
            serde_json::to_vec(request).map_err(|_| "The public request could not be encoded.")?;
        if bytes.len() > 8192 {
            return Err("The public request exceeded its size limit.".into());
        }
        let mut command = Command::new(
            std::env::current_exe().map_err(|_| "The downloader executable could not be found.")?,
        );
        command
            .arg("--linux-network-worker")
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let parent = std::process::id() as i32;
        // SAFETY: pre-exec uses only async-signal-safe Linux syscalls. Every
        // nonstandard inherited fd becomes close-on-exec, including unknown fds.
        unsafe {
            command.pre_exec(move || {
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::other(
                        "The downloader coordinator closed before isolation.",
                    ));
                }
                for (resource, limit) in [
                    (libc::RLIMIT_CORE, 0),
                    (libc::RLIMIT_FSIZE, 0),
                    (libc::RLIMIT_AS, 512 * 1024 * 1024),
                    (libc::RLIMIT_CPU, 25),
                ] {
                    let value = libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    };
                    if libc::setrlimit(resource, &value) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|_| "The isolated Linux downloader could not start.")?;
        let mut input = child
            .stdin
            .take()
            .ok_or("The downloader input pipe is missing.")?;
        if input.write_all(&bytes).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The downloader did not accept its public request.".into());
        }
        drop(input);
        let stdout = child
            .stdout
            .take()
            .ok_or("The downloader output pipe is missing.")?;
        let reader = std::thread::spawn(move || {
            let mut result = Vec::new();
            stdout
                .take(protocol::MAX_WIRE_BYTES as u64 + 1)
                .read_to_end(&mut result)
                .map(|_| result)
        });
        let start = Instant::now();
        loop {
            if child
                .try_wait()
                .map_err(|_| "The downloader state could not be read.")?
                .is_some()
            {
                break;
            }
            if start.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("The isolated downloader exceeded its time limit.".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let output = reader
            .join()
            .map_err(|_| "The downloader output reader stopped.")?
            .map_err(|_| "The downloader output could not be read.")?;
        protocol::decode(&output)
    }
}
