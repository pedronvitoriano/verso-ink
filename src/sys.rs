//! Chamadas de sistema mínimas (sem crates): ioctl e poll da libc (musl).

extern "C" {
    pub fn ioctl(fd: i32, request: u32, ...) -> i32;
    fn poll(fds: *mut PollFd, nfds: u32, timeout: i32) -> i32;
    fn prctl(option: i32, ...) -> i32;
}

/// Faz o filho morrer junto com o Verso Ink (PR_SET_PDEATHSIG com SIGTERM). Sem isso, cada
/// saída do Verso Ink deixava um `lipc-wait-event` órfão vivo (105 acumulados em 13/11).
pub fn die_with_parent(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            prctl(1, 15 as std::ffi::c_ulong);
            Ok(())
        });
    }
}

#[repr(C)]
pub struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

pub const POLLIN: i16 = 0x1;

/// Espera até `timeout_ms` por dados em `fd`. Devolve true se há o que ler.
pub fn wait_readable(fd: i32, timeout_ms: i32) -> bool {
    let mut p = PollFd { fd, events: POLLIN, revents: 0 };
    let rc = unsafe { poll(&mut p, 1, timeout_ms) };
    rc > 0 && (p.revents & POLLIN) != 0
}

/// Lê um arquivo de texto curto do sysfs/proc, sem espaços nas pontas.
pub fn read_trim(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

/// Roda um comando e devolve a saída padrão (sem espaços nas pontas).
pub fn sh(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn log(msg: &str) {
    use std::io::Write;
    let line = format!("[verso-ink] {msg}\n");
    let _ = std::io::stderr().write_all(line.as_bytes());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/verso-ink.log")
    {
        let _ = f.write_all(line.as_bytes());
    }
}
