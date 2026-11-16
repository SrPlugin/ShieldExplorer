//! Ejecutar un comando de shell en una carpeta, con salida en vivo y cancelable.
//!
//! El comando corre en su propio grupo de procesos para poder cancelar también a sus hijos.

use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub enum ShellEvent {
    /// Una línea de salida (stdout o stderr).
    Line(String),
    /// Terminó. `None` si lo mató una señal.
    Done(Option<i32>),
}

pub struct Running {
    rx: Receiver<ShellEvent>,
    cancel: Arc<AtomicBool>,
}

impl Running {
    pub fn try_recv(&self) -> Result<ShellEvent, TryRecvError> {
        self.rx.try_recv()
    }

    /// Pide terminar el comando y todo su grupo de procesos.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub fn spawn(command: &str, cwd: &Path) -> io::Result<Running> {
    let mut child = Command::new("sh")
        .args(["-c", command])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    // Con la cola llena los lectores esperan: la salida de un `yes` no llena la memoria.
    let (tx, rx) = sync_channel(2048);
    let cancel = Arc::new(AtomicBool::new(false));
    let readers = Arc::new(AtomicUsize::new(0));
    if let Some(out) = child.stdout.take() {
        spawn_reader(out, tx.clone(), &readers);
    }
    if let Some(err) = child.stderr.take() {
        spawn_reader(err, tx.clone(), &readers);
    }
    let flag = Arc::clone(&cancel);
    thread::spawn(move || wait_for(child, &flag, &readers, &tx));
    Ok(Running { rx, cancel })
}

fn spawn_reader(
    stream: impl Read + Send + 'static,
    tx: SyncSender<ShellEvent>,
    alive: &Arc<AtomicUsize>,
) {
    alive.fetch_add(1, Ordering::SeqCst);
    let alive = Arc::clone(alive);
    thread::spawn(move || {
        for line in BufReader::new(stream).split(b'\n').map_while(Result::ok) {
            let text = String::from_utf8_lossy(&line)
                .trim_end_matches('\r')
                .to_owned();
            if tx.send(ShellEvent::Line(text)).is_err() {
                break;
            }
        }
        alive.fetch_sub(1, Ordering::SeqCst);
    });
}

fn signal_group(pid: u32, signal: &str) {
    // `kill` externo: así no hace falta `unsafe`. El `-` delante del pid apunta al grupo.
    let _ = Command::new("kill")
        .args([&format!("-{signal}"), "--", &format!("-{pid}")])
        .stderr(Stdio::null())
        .status();
}

fn wait_for(
    mut child: Child,
    cancel: &AtomicBool,
    readers: &AtomicUsize,
    tx: &SyncSender<ShellEvent>,
) {
    let pid = child.id();
    let mut asked = None::<Instant>;
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            match asked {
                None => {
                    signal_group(pid, "TERM");
                    asked = Some(Instant::now());
                }
                Some(at) if at.elapsed() > Duration::from_secs(2) => {
                    signal_group(pid, "KILL");
                    let _ = child.kill();
                }
                Some(_) => {}
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(_) => break None,
        }
    };
    // Deja un momento a los lectores para vaciar la salida; un hijo que siga con las tuberías
    // abiertas no debe bloquear el aviso de fin.
    let deadline = Instant::now() + Duration::from_millis(300);
    while readers.load(Ordering::SeqCst) > 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    let _ = tx.send(ShellEvent::Done(status.and_then(|s| s.code())));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(run: &Running, limit: Duration) -> (Vec<String>, Option<Option<i32>>) {
        let start = Instant::now();
        let mut lines = Vec::new();
        while start.elapsed() < limit {
            match run.try_recv() {
                Ok(ShellEvent::Line(l)) => lines.push(l),
                Ok(ShellEvent::Done(code)) => return (lines, Some(code)),
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        }
        (lines, None)
    }

    #[test]
    fn captures_output_and_exit_code() {
        let tmp = tempfile::tempdir().unwrap();
        let run = spawn("echo hola; echo error 1>&2; pwd; exit 3", tmp.path()).unwrap();
        let (mut lines, done) = collect(&run, Duration::from_secs(5));
        assert_eq!(done, Some(Some(3)));
        lines.sort();
        assert!(lines.contains(&"hola".to_owned()) && lines.contains(&"error".to_owned()));
        let dir = tmp.path().canonicalize().unwrap().display().to_string();
        assert!(lines.contains(&dir));
    }

    #[test]
    fn cancel_stops_the_whole_group() {
        let tmp = tempfile::tempdir().unwrap();
        let run = spawn("sleep 30 & sleep 30", tmp.path()).unwrap();
        thread::sleep(Duration::from_millis(150));
        run.cancel();
        let (_, done) = collect(&run, Duration::from_secs(6));
        assert!(done.is_some(), "el comando debía terminar al cancelarlo");
    }
}
