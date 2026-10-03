//! Host system samples behind Platform's local_time / mem_used_kb /
//! network_info / thread_samples. Linux reads /proc and libc; other targets
//! report the trait defaults.

use alloc::{string::String, string::ToString, vec::Vec};

use crate::{LocalTime, NetworkInfo, ThreadSample};

/// VmRSS from /proc/self/status text.
pub fn parse_vmrss_kb(status: &str) -> Option<i64> {
    let rest = status.lines().find_map(|l| l.strip_prefix("VmRSS:"))?;
    rest.split_whitespace().next()?.parse().ok()
}

/// (state, utime + stime in clock ticks) from /proc/<pid>/task/<tid>/stat.
/// The name field is in parens and may itself contain ')', so parsing starts
/// after the LAST ')'; from there index 0 is the state, 11/12 utime/stime.
pub fn parse_task_stat(stat: &str) -> Option<(char, u64)> {
    let after = &stat[stat.rfind(')')? + 1..];
    let f: Vec<&str> = after.split_whitespace().collect();
    let state = f.first()?.chars().next()?;
    let utime: u64 = f.get(11)?.parse().ok()?;
    let stime: u64 = f.get(12)?.parse().ok()?;
    Some((state, utime + stime))
}

#[cfg(target_os = "linux")]
pub fn local_time() -> LocalTime {
    // SAFETY: time(NULL) only returns the clock; localtime_r fills our
    // zeroed tm and, unlike localtime, touches no shared static buffer.
    let tm = unsafe {
        let now = libc::time(core::ptr::null_mut());
        let mut tm: libc::tm = core::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    LocalTime {
        year: tm.tm_year + 1900,
        month: tm.tm_mon + 1,
        day: tm.tm_mday,
        hour: tm.tm_hour,
        min: tm.tm_min,
        sec: tm.tm_sec,
    }
}

#[cfg(target_os = "linux")]
pub fn mem_used_kb() -> i64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| parse_vmrss_kb(&s))
        .unwrap_or(-1)
}

/// Host name plus the first non-loopback IPv4 address.
#[cfg(target_os = "linux")]
pub fn network_info() -> NetworkInfo {
    let mut buf = [0u8; 256];
    // SAFETY: buf is 256 bytes and we pass that length; gethostname
    // NUL-terminates unless truncated, and we stop at the first NUL anyway.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    let host = if rc == 0 {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..end]).into_owned()
    } else {
        "unknown".to_string()
    };

    let mut ip = None;
    let mut head: *mut libc::ifaddrs = core::ptr::null_mut();
    // SAFETY: getifaddrs hands us a linked list we only read while it is
    // alive, and free with freeifaddrs on every path after a successful call.
    unsafe {
        if libc::getifaddrs(&mut head) == 0 {
            let mut cur = head;
            while !cur.is_null() {
                let ifa = &*cur;
                cur = ifa.ifa_next;
                if ifa.ifa_addr.is_null()
                    || i32::from((*ifa.ifa_addr).sa_family) != libc::AF_INET
                    || ifa.ifa_flags & (libc::IFF_LOOPBACK as u32) != 0
                {
                    continue;
                }
                let sin = &*(ifa.ifa_addr as *const libc::sockaddr_in);
                ip = Some(std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr)));
                break;
            }
            libc::freeifaddrs(head);
        }
    }
    match ip {
        Some(a) => NetworkInfo { host, ip: a.to_string(), connected: true },
        None => NetworkInfo { host, ip: "none".to_string(), connected: false },
    }
}

/// One row per OS thread of this process, from /proc/self/task.
#[cfg(target_os = "linux")]
pub fn thread_samples() -> Vec<ThreadSample> {
    let Ok(dir) = std::fs::read_dir("/proc/self/task") else {
        return Vec::new();
    };
    let mut tids: Vec<u64> = dir
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    tids.sort_unstable();
    // SAFETY: sysconf only reads a system constant.
    let hz = match unsafe { libc::sysconf(libc::_SC_CLK_TCK) } {
        n if n > 0 => n as u64,
        _ => 100,
    };
    tids.into_iter()
        .filter_map(|id| {
            // A thread that exited mid-scan has no files left: skip it.
            let base = std::format!("/proc/self/task/{id}");
            let name = std::fs::read_to_string(std::format!("{base}/comm")).ok()?;
            let stat = std::fs::read_to_string(std::format!("{base}/stat")).ok()?;
            let (state, ticks) = parse_task_stat(&stat)?;
            Some(ThreadSample { id, name: name.trim_end_matches('\n').to_string(), state, cpu_ms: ticks * 1000 / hz })
        })
        .collect()
}

#[cfg(not(target_os = "linux"))]
pub fn local_time() -> LocalTime {
    LocalTime { year: 1970, month: 1, day: 1, hour: 0, min: 0, sec: 0 }
}

#[cfg(not(target_os = "linux"))]
pub fn mem_used_kb() -> i64 {
    -1
}

#[cfg(not(target_os = "linux"))]
pub fn network_info() -> NetworkInfo {
    NetworkInfo::unknown()
}

#[cfg(not(target_os = "linux"))]
pub fn thread_samples() -> Vec<ThreadSample> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmrss_is_read_from_proc_status() {
        let status = "Name:\tacid-os\nVmPeak:\t  99999 kB\nVmRSS:\t   12345 kB\nThreads:\t9\n";
        assert_eq!(parse_vmrss_kb(status), Some(12345));
        assert_eq!(parse_vmrss_kb("Name:\tx\n"), None);
    }

    #[test]
    fn task_stat_gives_state_and_cpu_ticks() {
        // Field 2 is the name in parens and may itself contain ") ";
        // everything is read after the LAST ')'. utime/stime are fields 14/15.
        let stat = "4242 (v3/apps/a) b) S 1 2 3 4 5 6 7 8 9 10 250 50 0 0 20 0 1 0 100";
        assert_eq!(parse_task_stat(stat), Some(('S', 300)));
        assert_eq!(parse_task_stat("4242 (x) R"), None, "too few fields");
        assert_eq!(parse_task_stat("garbage"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_values_on_linux() {
        let t = local_time();
        assert!(t.year >= 2026 && (1..=12).contains(&t.month) && (1..=31).contains(&t.day));
        assert!(mem_used_kb() > 0);
        let n = network_info();
        assert!(!n.host.is_empty());
        let named = std::thread::Builder::new()
            .name("sysinfo-probe".into())
            .spawn(thread_samples)
            .unwrap()
            .join()
            .unwrap();
        assert!(named.iter().any(|s| s.name == "sysinfo-probe"), "{named:?}");
    }
}
