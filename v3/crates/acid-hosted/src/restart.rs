//! Restarting the hosted OS: replace this process with a fresh copy of the
//! program (on Unix; elsewhere, start a copy and exit), which opens at the
//! screen-size picker. The new copy gets the same
//! command line minus any `--screen`, so the picker shows again; every
//! other argument (`--app tetris`, say) carries over.

/// The arguments for the fresh copy: `args` (without the program name) with
/// `--screen <size>` and `--screen=<size>` removed, everything else in order.
pub fn restart_args(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--screen" {
            it.next();
        } else if !a.starts_with("--screen=") {
            out.push(a);
        }
    }
    out
}

/// Starts the fresh copy. On Unix this replaces the running process (exec),
/// so the copy keeps this one's process id, process group and terminal:
/// Ctrl+C and the launching shell's wait carry over, and every thread and
/// the old window go with the old image. Elsewhere it spawns the copy and
/// exits. Returns false (after saying why on stderr) only if the copy
/// couldn't be started.
pub fn restart_process() -> bool {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("Acid OS v3: restart: can't find this program: {e}");
            return false;
        }
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.args(restart_args(std::env::args().skip(1)));
    start_copy(&mut cmd)
}

/// exec only returns on failure.
#[cfg(unix)]
fn start_copy(cmd: &mut std::process::Command) -> bool {
    use std::os::unix::process::CommandExt;
    let e = cmd.exec();
    eprintln!("Acid OS v3: restart: can't start a new copy: {e}");
    false
}

#[cfg(not(unix))]
fn start_copy(cmd: &mut std::process::Command) -> bool {
    match cmd.spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            eprintln!("Acid OS v3: restart: can't start a new copy: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::restart_args;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn screen_is_dropped_and_everything_else_kept_in_order() {
        assert_eq!(restart_args(args(&[])), args(&[]));
        assert_eq!(restart_args(args(&["--screen", "800x600"])), args(&[]));
        assert_eq!(restart_args(args(&["--screen=640x360"])), args(&[]));
        assert_eq!(restart_args(args(&["--app", "tetris", "--screen", "800x600"])), args(&["--app", "tetris"]));
        assert_eq!(restart_args(args(&["--app", "tetris", "--screen"])), args(&["--app", "tetris"]));
        assert_eq!(restart_args(args(&["a", "--screen=800x600", "b", "c"])), args(&["a", "b", "c"]));
    }
}
