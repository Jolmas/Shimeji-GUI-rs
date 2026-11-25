use nix::errno::Errno;
use nix::pty::openpty;
use nix::sys::signal::{kill, Signal};
use nix::sys::termios::{cfmakeraw, tcgetattr, tcsetattr, SetArg};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{read, write, Pid};
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CtlOutput {
    pub code: i32,
    pub output: String,
}

#[derive(Debug, Clone)]
pub enum CtlEvent {
    Output(String),
    Finished(CtlOutput),
}

fn normalize_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n").replace('\r', "\n")
}

fn set_nonblocking(fd: BorrowedFd<'_>) -> nix::Result<()> {
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    let flags = fcntl(fd.as_raw_fd(), FcntlArg::F_GETFL)?;
    let mut flags = OFlag::from_bits_truncate(flags);
    flags.insert(OFlag::O_NONBLOCK);
    fcntl(fd.as_raw_fd(), FcntlArg::F_SETFL(flags))?;
    Ok(())
}

fn drain_available(master: BorrowedFd<'_>, chunks: &mut Vec<u8>, out_tx: &Option<mpsc::Sender<CtlEvent>>) {
    loop {
        let mut buf = [0u8; 4096];
        match read(master.as_raw_fd(), &mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let data = &buf[..n];
                chunks.extend_from_slice(data);
                if let Some(tx) = out_tx {
                    let _ = tx.send(CtlEvent::Output(normalize_output(data)));
                }
            }
            Err(Errno::EAGAIN) => break,
            Err(Errno::EIO) | Err(Errno::EBADF) => break,
            Err(_) => break,
        }
    }
}

fn wait_readable(fd: BorrowedFd<'_>, timeout: Duration) -> bool {
    use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
    let mut fds = [PollFd::new(fd, PollFlags::POLLIN | PollFlags::POLLHUP | PollFlags::POLLERR)];
    let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
    let to = PollTimeout::try_from(ms).unwrap_or(PollTimeout::ZERO);
    matches!(poll(&mut fds, to), Ok(n) if n > 0)
}

fn child_exited(pid: Pid) -> Option<i32> {
    match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
        Ok(WaitStatus::Exited(_, code)) => Some(code),
        Ok(WaitStatus::Signaled(_, sig, _)) => Some(128 + sig as i32),
        Ok(WaitStatus::StillAlive) => None,
        Ok(_) => None,
        Err(Errno::ECHILD) => Some(0),
        Err(_) => None,
    }
}

pub fn run_ctl_sync(args: &[String], input: Option<&str>, timeout: Duration) -> io::Result<CtlOutput> {
    run_cmd_sync("shimejictl", args, input, timeout, None)
}

pub fn run_ctl_sync_with_cancel(
    args: &[String],
    input: Option<&str>,
    timeout: Duration,
    cancel: Option<Arc<AtomicBool>>,
) -> io::Result<CtlOutput> {
    run_cmd_sync("shimejictl", args, input, timeout, cancel)
}

pub fn run_cmd_sync(program: &str, args: &[String], input: Option<&str>, timeout: Duration, cancel: Option<Arc<AtomicBool>>) -> io::Result<CtlOutput> {
    let (tx, rx) = mpsc::channel::<CtlEvent>();
    let handle = spawn_cmd(program.to_string(), args.to_vec(), input.map(|s| s.to_string()), timeout, Some(tx), cancel);
    let mut last = CtlOutput { code: 1, output: String::new() };
    for ev in rx {
        if let CtlEvent::Finished(out) = ev {
            last = out;
        }
    }
    let _ = handle.join();
    Ok(last)
}

pub struct CtlHandle {
    cancel: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
}

impl CtlHandle {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
    pub fn join(mut self) {
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub fn spawn_ctl(args: Vec<String>, input: Option<String>, timeout: Duration, events: Option<mpsc::Sender<CtlEvent>>, cancel: Option<Arc<AtomicBool>>) -> CtlHandle {
    spawn_cmd("shimejictl".to_string(), args, input, timeout, events, cancel)
}

pub fn spawn_cmd(program: String, args: Vec<String>, input: Option<String>, timeout: Duration, events: Option<mpsc::Sender<CtlEvent>>, cancel: Option<Arc<AtomicBool>>) -> CtlHandle {
    let cancel = cancel.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    let cancel_thread = cancel.clone();
    let join = thread::spawn(move || {
        let out = run_with_pty(program, args, input, timeout, events.clone(), cancel_thread);
        if let Some(tx) = events {
            let _ = tx.send(CtlEvent::Finished(out));
        }
    });
    CtlHandle { cancel, join: Some(join) }
}

fn run_with_pty(program: String, args: Vec<String>, input: Option<String>, timeout: Duration, events: Option<mpsc::Sender<CtlEvent>>, cancel: Arc<AtomicBool>) -> CtlOutput {
    let pty = match openpty(None, None) {
        Ok(p) => p,
        Err(e) => return CtlOutput { code: 1, output: format!("[Error de ejecución: {e}]\n") },
    };
    let master: OwnedFd = pty.master;
    let slave: OwnedFd = pty.slave;

    // Raw mode avoids PTY line-discipline mangling (ONLCR) while keeping output faithful.
    if let Ok(mut termios) = tcgetattr(&slave) {
        cfmakeraw(&mut termios);
        let _ = tcsetattr(&slave, SetArg::TCSANOW, &termios);
    }

    let master_b = unsafe { BorrowedFd::borrow_raw(master.as_raw_fd()) };
    let _ = set_nonblocking(master_b);

    let mut cmd = Command::new(&program);
    cmd.args(&args);
    cmd.stdin(Stdio::from(slave.try_clone().expect("clone slave")));
    cmd.stdout(Stdio::from(slave.try_clone().expect("clone slave")));
    cmd.stderr(Stdio::from(slave));
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return CtlOutput { code: 1, output: format!("[Error de ejecución: {e}]\n") },
    };
    let pid = Pid::from_raw(child.id() as i32);

    if let Some(input) = input {
        let _ = write(&master, input.as_bytes());
    }

    let mut chunks: Vec<u8> = Vec::new();
    let start = Instant::now();
    let mut timed_out = false;
    let mut exit_code: Option<i32> = None;
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = kill(pid, Signal::SIGTERM);
            let _ = child.kill();
            if let Some(tx) = &events {
                let _ = tx.send(CtlEvent::Output("\n[Operación cancelada por el usuario]\n".to_string()));
            }
            chunks.extend_from_slice("\n[Operación cancelada por el usuario]\n".as_bytes());
            break;
        }
        if !timeout.is_zero() && start.elapsed() > timeout {
            timed_out = true;
            let _ = kill(pid, Signal::SIGTERM);
            let _ = child.kill();
            break;
        }
        if wait_readable(master_b, Duration::from_millis(100)) {
            drain_available(master_b, &mut chunks, &events);
        }
        if let Some(code) = child_exited(pid) {
            exit_code = Some(code);
            for _ in 0..20 {
                if wait_readable(master_b, Duration::from_millis(20)) {
                    drain_available(master_b, &mut chunks, &events);
                } else {
                    break;
                }
            }
            drain_available(master_b, &mut chunks, &events);
            break;
        }
    }

    let code = if cancel.load(Ordering::SeqCst) {
        -1
    } else if timed_out {
        let _ = child.wait();
        124
    } else if let Some(code) = exit_code {
        let _ = child.wait();
        code
    } else {
        match child.wait() {
            Ok(s) => s.code().unwrap_or(-1),
            Err(_) => -1,
        }
    };
    CtlOutput { code, output: normalize_output(&chunks) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_cmd_sync_captures_output_and_normalizes_crlf() {
        let args = vec!["-c".to_string(), "printf 'a\r\nb\r'".to_string()];
        let out = run_cmd_sync("/bin/sh", &args, None, Duration::from_secs(5), None).unwrap();
        assert_eq!(out.code, 0);
        assert_eq!(out.output, "a\nb\n");
    }

    #[test]
    fn run_cmd_sync_accepts_input() {
        let args = vec!["-c".to_string(), "IFS= read -r line; printf '%s' \"$line\"".to_string()];
        let out = run_cmd_sync("/bin/sh", &args, Some("hello\n"), Duration::from_secs(5), None).unwrap();
        assert_eq!(out.code, 0);
        assert!(out.output.contains("hello"));
    }

    #[test]
    fn spawn_cmd_can_cancel() {
        let args = vec!["-c".to_string(), "sleep 5".to_string()];
        let (tx, rx) = mpsc::channel();
        let handle = spawn_cmd("/bin/sh".to_string(), args, None, Duration::from_secs(10), Some(tx), None);
        thread::sleep(Duration::from_millis(100));
        handle.cancel();
        handle.join();
        let mut saw_cancel = false;
        let mut code = 0;
        for ev in rx {
            match ev {
                CtlEvent::Output(s) if s.contains("cancelada por el usuario") => saw_cancel = true,
                CtlEvent::Finished(out) => code = out.code,
                _ => {}
            }
        }
        assert!(saw_cancel);
        assert_eq!(code, -1);
    }
}
