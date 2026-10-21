//! The pseudo-terminal behind each shell, behind traits so the manager's tests can drive plain
//! channels instead of real processes.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};

use crate::shell::ShellCommand;
use crate::{Error, Result};

/// One read from the PTY.
#[derive(Debug)]
pub enum ReadOutcome {
    Data(Vec<u8>),
    /// Nothing arrived within the timeout; the caller checks whether it should stop.
    Idle,
    /// Every process holding the terminal has gone.
    Eof,
}

/// The PTY's output side. Runs on its own thread.
pub trait PtyReader: Send {
    /// Wait up to `timeout` for output.
    fn read(&mut self, timeout: Duration) -> std::io::Result<ReadOutcome>;
}

/// Resize and signal the shell. Dropping it releases the PTY.
pub trait PtyControl: Send {
    fn resize(&self, cols: u16, rows: u16) -> Result<()>;
    /// Ask the shell and its foreground job to end (`SIGHUP`), or with `force`, kill them.
    fn terminate(&self, force: bool);
    /// The shell's process id, if known.
    fn pid(&self) -> Option<u32>;
}

/// Waits for the shell to exit, reaping it. Runs on its own thread.
pub trait ChildWaiter: Send {
    /// The exit code, or `None` if it could not be read.
    fn wait(self: Box<Self>) -> Option<i32>;
}

pub struct Spawned {
    pub reader: Box<dyn PtyReader>,
    pub writer: Box<dyn Write + Send>,
    pub control: Box<dyn PtyControl>,
    pub waiter: Box<dyn ChildWaiter>,
}

/// Starts a program on a fresh PTY.
pub trait PtyBackend: Send + Sync {
    fn spawn(&self, command: &ShellCommand, cwd: &Path, cols: u16, rows: u16) -> Result<Spawned>;
}

/// The real thing: `portable-pty`'s native PTY (openpty on Unix, ConPTY on Windows).
pub struct NativePty;

impl PtyBackend for NativePty {
    fn spawn(&self, command: &ShellCommand, cwd: &Path, cols: u16, rows: u16) -> Result<Spawned> {
        let spawn_error = |e: &dyn std::fmt::Display| Error::Spawn(e.to_string());
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| spawn_error(&e))?;

        let mut builder = CommandBuilder::new(&command.program);
        for arg in &command.args {
            builder.arg(arg);
        }
        builder.cwd(cwd.as_os_str());
        for (key, value) in &command.env {
            builder.env(key, value);
        }
        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| spawn_error(&e))?;
        // Only the shell keeps the slave side open, so the reader sees EOF once it is gone.
        drop(pair.slave);

        let pid = child.process_id();
        let killer = child.clone_killer();
        let reader = native_reader(&*pair.master).map_err(|e| spawn_error(&e))?;
        let writer = pair.master.take_writer().map_err(|e| spawn_error(&e))?;
        Ok(Spawned {
            reader,
            writer,
            control: Box::new(NativeControl {
                master: pair.master,
                pid,
                killer: std::sync::Mutex::new(killer),
            }),
            waiter: Box::new(NativeWaiter(child)),
        })
    }
}

struct NativeControl {
    master: Box<dyn MasterPty + Send>,
    pid: Option<u32>,
    #[cfg_attr(unix, allow(dead_code))]
    killer: std::sync::Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl PtyControl for NativeControl {
    fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::Io(e.to_string()))
    }

    #[cfg(unix)]
    fn terminate(&self, force: bool) {
        let signal = if force { libc::SIGKILL } else { libc::SIGHUP };
        // The shell is a session leader, so its process group is its pid. The foreground job
        // (what the user is running) has a group of its own under job control.
        let mut groups: Vec<libc::pid_t> = Vec::new();
        if let Some(pid) = self.pid.and_then(|p| libc::pid_t::try_from(p).ok()) {
            groups.push(pid);
        }
        if let Some(foreground) = self.master.process_group_leader() {
            if !groups.contains(&foreground) {
                groups.push(foreground);
            }
        }
        for group in groups.into_iter().filter(|g| *g > 1) {
            // SAFETY: plain syscall; a group that is already gone just returns ESRCH.
            unsafe {
                libc::killpg(group, signal);
                if !force {
                    // A stopped job ignores SIGHUP until continued.
                    libc::killpg(group, libc::SIGCONT);
                }
            }
        }
    }

    #[cfg(windows)]
    fn terminate(&self, _force: bool) {
        // TerminateProcess; closing the pseudo console (dropping `master`) ends the rest.
        if let Ok(mut killer) = self.killer.lock() {
            let _ = killer.kill();
        }
    }

    fn pid(&self) -> Option<u32> {
        self.pid
    }
}

struct NativeWaiter(Box<dyn portable_pty::Child + Send + Sync>);

impl ChildWaiter for NativeWaiter {
    fn wait(mut self: Box<Self>) -> Option<i32> {
        let status = self.0.wait().ok()?;
        i32::try_from(status.exit_code()).ok()
    }
}

/// On Unix: our own close-on-exec duplicate of the master, read with a timeout so the reader
/// thread can stop even if some stray process still holds the terminal open.
#[cfg(unix)]
fn native_reader(master: &dyn MasterPty) -> std::io::Result<Box<dyn PtyReader>> {
    use std::os::fd::{FromRawFd, OwnedFd};
    let fd = master
        .as_raw_fd()
        .ok_or_else(|| std::io::Error::other("the PTY has no file descriptor"))?;
    // SAFETY: `fd` is the open master; F_DUPFD_CLOEXEC returns a new descriptor we own.
    let dup = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if dup < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `dup` was just created and nothing else owns it.
    let fd = unsafe { OwnedFd::from_raw_fd(dup) };
    Ok(Box::new(FdReader {
        fd,
        buffer: vec![0u8; 64 * 1024],
    }))
}

#[cfg(unix)]
struct FdReader {
    fd: std::os::fd::OwnedFd,
    buffer: Vec<u8>,
}

#[cfg(unix)]
impl FdReader {
    /// `select`, not `poll`: macOS `poll` does not support character devices.
    fn wait_readable(&self, timeout: Duration) -> std::io::Result<bool> {
        use std::os::fd::AsRawFd;
        let fd = self.fd.as_raw_fd();
        if fd as usize >= libc::FD_SETSIZE {
            // Too high for an fd_set; fall back to a blocking read.
            return Ok(true);
        }
        // SAFETY: an fd_set is plain data; `fd` is below FD_SETSIZE.
        unsafe {
            let mut set: libc::fd_set = std::mem::zeroed();
            libc::FD_ZERO(&mut set);
            libc::FD_SET(fd, &mut set);
            let mut tv = libc::timeval {
                tv_sec: timeout.as_secs() as libc::time_t,
                tv_usec: timeout.subsec_micros() as libc::suseconds_t,
            };
            let ready = libc::select(
                fd + 1,
                &mut set,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            );
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                return if error.kind() == std::io::ErrorKind::Interrupted {
                    Ok(false)
                } else {
                    Err(error)
                };
            }
            Ok(ready > 0)
        }
    }
}

#[cfg(unix)]
impl PtyReader for FdReader {
    fn read(&mut self, timeout: Duration) -> std::io::Result<ReadOutcome> {
        use std::os::fd::AsRawFd;
        if !self.wait_readable(timeout)? {
            return Ok(ReadOutcome::Idle);
        }
        // SAFETY: reading into our own buffer, bounded by its length.
        let n = unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                self.buffer.as_mut_ptr().cast(),
                self.buffer.len(),
            )
        };
        if n > 0 {
            return Ok(ReadOutcome::Data(self.buffer[..n as usize].to_vec()));
        }
        if n == 0 {
            return Ok(ReadOutcome::Eof);
        }
        let error = std::io::Error::last_os_error();
        match error.raw_os_error() {
            // EIO: the slave side has been closed by everyone.
            Some(libc::EIO) => Ok(ReadOutcome::Eof),
            Some(libc::EINTR) | Some(libc::EAGAIN) => Ok(ReadOutcome::Idle),
            _ => Err(error),
        }
    }
}

/// On Windows: ConPTY's output pipe, which breaks when the pseudo console is closed.
#[cfg(windows)]
fn native_reader(master: &dyn MasterPty) -> std::io::Result<Box<dyn PtyReader>> {
    let reader = master
        .try_clone_reader()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    Ok(Box::new(BlockingReader {
        inner: reader,
        buffer: vec![0u8; 64 * 1024],
    }))
}

#[cfg(windows)]
struct BlockingReader {
    inner: Box<dyn std::io::Read + Send>,
    buffer: Vec<u8>,
}

#[cfg(windows)]
impl PtyReader for BlockingReader {
    fn read(&mut self, _timeout: Duration) -> std::io::Result<ReadOutcome> {
        match std::io::Read::read(&mut self.inner, &mut self.buffer) {
            Ok(0) => Ok(ReadOutcome::Eof),
            Ok(n) => Ok(ReadOutcome::Data(self.buffer[..n].to_vec())),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Ok(ReadOutcome::Idle),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(ReadOutcome::Eof),
            Err(e) => Err(e),
        }
    }
}
