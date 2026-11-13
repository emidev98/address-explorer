//! Process-level secret-leak mitigations.

/// Disable core dumps and, on Linux, mark the process non-dumpable so a crash
/// cannot write decrypted database pages or derived keys to disk. Swap
/// encryption and ptrace restrictions remain host responsibilities.
pub fn apply() {
    #[cfg(unix)]
    disable_core_dumps();
    #[cfg(target_os = "linux")]
    disable_dumpable();
}

#[cfg(unix)]
fn disable_core_dumps() {
    // SAFETY: `setrlimit` only reads the two initialized `rlimit` values and
    // applies a process-wide limit. No pointers escape the call.
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
            eprintln!("warning: could not disable core dumps");
        }
    }
}

#[cfg(target_os = "linux")]
fn disable_dumpable() {
    // SAFETY: `prctl` is called with the documented PR_SET_DUMPABLE option and
    // scalar arguments; it has no pointer or lifetime requirements.
    unsafe {
        if libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) != 0 {
            eprintln!("warning: could not mark the process non-dumpable");
        }
    }
}
