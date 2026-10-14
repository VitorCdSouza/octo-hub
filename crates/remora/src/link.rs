//! A background thread that owns the receiver: it reconnects on its own,
//! forwards status reports and sends feature reports one at a time, waiting
//! for each acknowledgement. The radio round trip takes tens of
//! milliseconds, so the window never blocks on it.

use crate::hid::{self, HidDevice};
use crate::proto::{self, Status};
use anyhow::{Result, bail};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

pub const ACK_TIMEOUT: Duration = Duration::from_millis(800);

pub enum Command {
    Send(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected { path: String, phys: String },
    /// no receiver plugged in, or it went away; carries the reason
    Disconnected(String),
    Status(Status),
    /// a feature report reached the mouse
    Applied(u8),
    Failed(u8, String),
}

/// What the worker talks to: the real receiver or the demo mouse.
pub trait Backend: Send {
    fn send_feature(&mut self, report: &[u8]) -> Result<()>;
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>>;
    fn drain(&mut self);
}

impl Backend for HidDevice {
    fn send_feature(&mut self, report: &[u8]) -> Result<()> {
        HidDevice::send_feature(self, report)
    }
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        HidDevice::read(self, timeout)
    }
    fn drain(&mut self) {
        HidDevice::drain(self)
    }
}

pub struct Link {
    tx: Sender<Command>,
    rx: Receiver<Event>,
}

impl Link {
    /// Starts the worker; `wake` runs after every event so the window can repaint.
    pub fn spawn(demo: bool, wake: impl Fn() + Send + 'static) -> Link {
        let (tx, cmd_rx) = mpsc::channel();
        let (ev_tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("remora-link".into())
            .spawn(move || worker(demo, cmd_rx, ev_tx, wake))
            .expect("could not start the device thread");
        Link { tx, rx }
    }

    pub fn send(&self, report: Vec<u8>) {
        let _ = self.tx.send(Command::Send(report));
    }

    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

fn open(demo: bool) -> Result<(Box<dyn Backend>, String, String)> {
    if demo {
        return Ok((Box::new(crate::mock::DemoMouse::new()), "demo".into(), "in-memory mouse".into()));
    }
    let Some(info) = hid::enumerate().into_iter().next() else {
        bail!("no Attack Shark X6 receiver found (1d57:fa60)");
    };
    let dev = HidDevice::open(&info)?;
    Ok((Box::new(dev), info.path.display().to_string(), info.phys))
}

fn worker(demo: bool, cmds: Receiver<Command>, events: Sender<Event>, wake: impl Fn()) {
    let emit = |e: Event| {
        let ok = events.send(e).is_ok();
        wake();
        ok
    };
    let mut last_error = String::new();
    loop {
        let (mut dev, path, phys) = match open(demo) {
            Ok(d) => d,
            Err(e) => {
                let msg = format!("{e:#}");
                if msg != last_error && !emit(Event::Disconnected(msg.clone())) {
                    return;
                }
                last_error = msg;
                // keep the queue from piling up writes that can no longer go anywhere
                loop {
                    match cmds.try_recv() {
                        Ok(Command::Send(r)) => {
                            if !emit(Event::Failed(r[0], "mouse receiver not connected".into())) {
                                return;
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return,
                    }
                }
                std::thread::sleep(Duration::from_millis(800));
                continue;
            }
        };
        last_error.clear();
        dev.drain();
        if !emit(Event::Connected { path, phys }) {
            return;
        }
        match serve(dev.as_mut(), &cmds, &emit) {
            Ok(()) => return,
            Err(e) => {
                last_error = format!("{e:#}");
                if !emit(Event::Disconnected(last_error.clone())) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

/// Runs until the window closes (Ok) or the device fails (Err).
fn serve(dev: &mut dyn Backend, cmds: &Receiver<Command>, emit: &impl Fn(Event) -> bool) -> Result<()> {
    loop {
        // only the newest copy of each report matters: a dragged slider queues dozens
        let mut queue: Vec<Vec<u8>> = Vec::new();
        loop {
            match cmds.try_recv() {
                Ok(Command::Send(r)) => {
                    queue.retain(|q| q[0] != r[0]);
                    queue.push(r);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        for report in queue {
            let id = report[0];
            match send_and_wait(dev, &report, emit) {
                Ok(true) => {
                    if !emit(Event::Applied(id)) {
                        return Ok(());
                    }
                }
                Ok(false) => {
                    if !emit(Event::Failed(id, "no reply from the mouse (asleep or out of range? move it and try again)".into())) {
                        return Ok(());
                    }
                }
                Err(e) => {
                    emit(Event::Failed(id, format!("{e:#}")));
                    return Err(e);
                }
            }
        }
        if let Some(r) = dev.read(Duration::from_millis(60))?
            && let Some(s) = proto::decode_status(&r)
                && !emit(Event::Status(s)) {
                    return Ok(());
                }
    }
}

/// Sends one report and waits for its ACK, forwarding anything else that
/// arrives. The receiver sometimes answers `03 10 03 00 02` instead and drops
/// the write (seen right after a pause in traffic), so it tries again.
pub fn send_and_wait(dev: &mut dyn Backend, report: &[u8], emit: &impl Fn(Event) -> bool) -> Result<bool> {
    for attempt in 0..ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(40));
        }
        if send_once(dev, report, emit)? {
            return Ok(true);
        }
    }
    Ok(false)
}

const ATTEMPTS: usize = 3;

fn send_once(dev: &mut dyn Backend, report: &[u8], emit: &impl Fn(Event) -> bool) -> Result<bool> {
    let id = report[0];
    dev.send_feature(report)?;
    let deadline = Instant::now() + ACK_TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(false);
        }
        let Some(r) = dev.read(left)? else { return Ok(false) };
        match proto::decode_status(&r) {
            Some(Status::Ack(got)) if got == id => return Ok(true),
            Some(Status::Other(o)) if o.get(2) == Some(&0x03) => {
                emit(Event::Status(Status::Other(o)));
                return Ok(false);
            }
            Some(s) => {
                emit(Event::Status(s));
            }
            None => {}
        }
    }
}
