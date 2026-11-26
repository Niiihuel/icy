use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

pub type PtyWriterSlot = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

pub struct PtyHandle {
    master: Box<dyn MasterPty + Send>,
    writer: PtyWriterSlot,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    _reader: JoinHandle<()>,
}

impl PtyHandle {
    pub fn spawn(
        mut cmd: CommandBuilder,
        cols: u16,
        rows: u16,
        cwd: Option<&Path>,
        writer_slot: PtyWriterSlot,
        mut on_output: impl FnMut(&[u8]) + Send + 'static,
        on_exit: impl FnOnce() + Send + 'static,
    ) -> io::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(1),
                cols: cols.max(2),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::other(e.to_string()))?;

        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        if let Some(dir) = cwd {
            cmd.cwd(dir);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| io::Error::other(e.to_string()))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| io::Error::other(e.to_string()))?;
        *writer_slot.lock().unwrap() = Some(writer);

        let on_exit = Arc::new(Mutex::new(Some(on_exit)));
        let reader_thread = std::thread::Builder::new()
            .name("icy-pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => on_output(&buf[..n]),
                        Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                if let Some(cb) = on_exit.lock().unwrap().take() {
                    cb();
                }
            })?;

        Ok(Self {
            master: pair.master,
            writer: writer_slot,
            child: Mutex::new(child),
            _reader: reader_thread,
        })
    }

    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        let mut guard = self.writer.lock().unwrap();
        let writer = guard
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "pty writer closed"))?;
        writer.write_all(bytes)?;
        writer.flush()
    }

    pub fn foreground_cmdline(&self) -> Option<String> {
        crate::platform::foreground_cmdline(self.master.as_ref())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> io::Result<()> {
        self.master
            .resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(2),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| io::Error::other(e.to_string()))
    }

    pub fn kill(&self) {
        let _ = self.writer.lock().unwrap().take();
        if let Ok(mut child) = self.child.lock()
            && child.try_wait().is_ok_and(|status| status.is_none())
            && child.kill().is_ok()
        {
            let _ = child.wait();
        }
    }

    pub(crate) fn reap_if_exited(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.try_wait();
        }
    }
}

impl Drop for PtyHandle {
    fn drop(&mut self) {
        self.kill();
    }
}
