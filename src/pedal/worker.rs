//! A background thread that owns the pedal, so the window never waits on MIDI.
//!
//! The pedal sends nothing on its own, so while connected the thread polls once
//! a second: the active preset register for footswitch changes and bypass, and
//! the live block for knobs turned on the pedal. Knob frames go out one at a
//! time with the latest values: a drag sends what the pedal can take, never a
//! backlog. Logic follows TabJam's `PedalPanel.axaml.cs`.

use super::port::{self, Port, RawMidi, Simulated};
use super::preset::{self, Preset};
use super::tankg::{self, TankG};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

pub enum Command {
    /// Opens the real pedal, or the simulated one.
    Connect {
        simulated: bool,
    },
    /// Closes the port so other apps (TabJam) can open it.
    Release,
    Load(usize),
    Bypass(bool),
    Switch {
        offset: usize,
        on: bool,
    },
    Knobs(Preset),
    /// Backs up once per session, then stores, commits and loads `slot`.
    Save {
        slot: usize,
        preset: Preset,
    },
    BackupTo(PathBuf),
    Restore {
        table: Vec<u8>,
        slots: Vec<usize>,
    },
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub table: Vec<u8>,
    pub sounding: Option<usize>,
    pub bypassed: bool,
    /// None in bypass, where the pedal reads the live block as zeros
    pub live: Option<Preset>,
}

pub enum Event {
    Connected(Snapshot),
    /// The pedal changed under us (footswitch, knob on the pedal) or after a command.
    State(Snapshot),
    /// Connecting failed; `quiet` when it was an automatic retry.
    NotFound {
        why: String,
        quiet: bool,
    },
    Lost(String),
    Done(String),
    Failed(String),
}

pub struct Worker {
    tx: Sender<Command>,
    rx: Receiver<Event>,
}

impl Worker {
    pub fn spawn(wake: impl Fn() + Send + 'static) -> Worker {
        let (tx, cmd_rx) = mpsc::channel();
        let (ev_tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("octo-pedal".into())
            .spawn(move || run(cmd_rx, ev_tx, wake))
            .expect("could not start the pedal thread");
        Worker { tx, rx }
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }

    pub fn poll(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

/// The first write of a session to the stored presets goes after a copy of all 36.
pub fn backup_dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|x| !x.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
    data.join("octo-hub").join("tank-g")
}

struct State {
    pedal: TankG,
    simulated: bool,
    table: Vec<u8>,
    register: Vec<u8>,
    live: Option<Preset>,
    backed_up: bool,
}

impl State {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            table: self.table.clone(),
            sounding: self.pedal.sounding,
            bypassed: self.pedal.bypassed,
            live: self.live,
        }
    }

    /// Which stored preset is sounding: the register's bank and letter, else the one
    /// stored preset equal to the live block (`previous` breaks a tie).
    fn identify(&self, live: &Preset, previous: Option<usize>) -> Option<usize> {
        if let Some(tankg::Register { slot: Some(s), .. }) = tankg::decode_register(&self.register)
        {
            return Some(s);
        }
        let matches: Vec<usize> = (0..preset::COUNT)
            .filter(|s| preset::from_table(&self.table, *s) == *live)
            .collect();
        match matches.as_slice() {
            [one] => Some(*one),
            _ => previous.filter(|p| matches.contains(p)),
        }
    }

    /// After a load or save the pedal sounds a stored slot: read back what it holds.
    fn refresh(&mut self, table: bool) -> Result<()> {
        if table {
            self.table = self.pedal.read_table()?;
        }
        self.register = self.pedal.read_register()?;
        let reg = tankg::decode_register(&self.register);
        self.pedal.bypassed = reg.is_some_and(|r| r.bypassed);
        self.live = if self.pedal.bypassed {
            None
        } else {
            Some(self.pedal.read_live()?)
        };
        Ok(())
    }

    fn backup_once(&mut self) -> Result<()> {
        if self.backed_up || self.simulated {
            return Ok(());
        }
        let table = self.pedal.read_table()?;
        let dir = backup_dir();
        std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
        let path = dir.join(format!("auto-{}.bin", stamp()));
        std::fs::write(&path, &table).with_context(|| path.display().to_string())?;
        self.backed_up = true;
        Ok(())
    }
}

/// Local time as yyyymmdd-hhmmss, for backup names.
pub fn stamp() -> String {
    // SAFETY: localtime_r only writes into the struct it is given
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!(
            "{:04}{:02}{:02}-{:02}{:02}{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}

fn connect(simulated: bool) -> Result<State> {
    let port: Box<dyn Port> = if simulated {
        Box::new(Simulated::new())
    } else {
        let path = port::find().context("Tank-G not found")?;
        Box::new(RawMidi::open(path)?)
    };
    let pedal = TankG::connect(port)?;
    let mut s = State {
        pedal,
        simulated,
        table: vec![],
        register: vec![],
        live: None,
        backed_up: false,
    };
    s.refresh(true)?;
    // in bypass the letter is gone and the live block reads zeros: the preset is
    // unknown until one is loaded
    s.pedal.sounding = s.live.and_then(|l| s.identify(&l, None));
    Ok(s)
}

fn run(rx: Receiver<Command>, tx: Sender<Event>, wake: impl Fn()) {
    let emit = |e: Event| {
        let ok = tx.send(e).is_ok();
        wake();
        ok
    };
    let mut state: Option<State> = None;
    // reconnects quietly while the panel wants the pedal and it is away
    let mut wanted = false;
    let mut last_try = Instant::now();
    let mut last_edit = Instant::now() - Duration::from_secs(10);
    loop {
        let cmd = match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(c) => Some(c),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        // only the newest knob frame matters
        let mut cmds: Vec<Command> = cmd.into_iter().chain(rx.try_iter()).collect();
        if let Some(last) = cmds.iter().rposition(|c| matches!(c, Command::Knobs(_))) {
            let mut i = 0;
            cmds.retain(|c| {
                let keep = !matches!(c, Command::Knobs(_)) || i == last;
                i += 1;
                keep
            });
        }
        if cmds.is_empty() {
            match &mut state {
                None if wanted && last_try.elapsed() > Duration::from_secs(3) => {
                    last_try = Instant::now();
                    match connect(false) {
                        Ok(s) => {
                            emit(Event::Connected(s.snapshot()));
                            state = Some(s);
                        }
                        Err(e) => {
                            emit(Event::NotFound {
                                why: format!("{e:#}"),
                                quiet: true,
                            });
                        }
                    }
                }
                Some(s) if last_edit.elapsed() > Duration::from_millis(1500) => match poll(s) {
                    Ok(true) => {
                        emit(Event::State(s.snapshot()));
                    }
                    Ok(false) => {}
                    Err(e) => {
                        state = None;
                        emit(Event::Lost(format!("{e:#}")));
                    }
                },
                _ => {}
            }
            continue;
        }
        for c in cmds {
            match c {
                Command::Connect { simulated } => {
                    wanted = !simulated;
                    last_try = Instant::now();
                    state = None;
                    match connect(simulated) {
                        Ok(s) => {
                            emit(Event::Connected(s.snapshot()));
                            state = Some(s);
                        }
                        Err(e) => {
                            emit(Event::NotFound {
                                why: format!("{e:#}"),
                                quiet: false,
                            });
                        }
                    }
                }
                Command::Release => {
                    wanted = false;
                    state = None;
                }
                other => {
                    let Some(s) = state.as_mut() else { continue };
                    let edit = matches!(other, Command::Knobs(_) | Command::Switch { .. });
                    if edit {
                        last_edit = Instant::now();
                    }
                    match apply(s, other) {
                        Ok(Some(msg)) => {
                            emit(Event::State(s.snapshot()));
                            emit(Event::Done(msg));
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let lost = e.chain().any(|c| {
                                let t = c.to_string();
                                t.contains("did not answer")
                                    || t.contains("No such device")
                                    || t.contains("MIDI port")
                            });
                            if lost {
                                state = None;
                                emit(Event::Lost(format!("{e:#}")));
                            } else {
                                emit(Event::Failed(format!("{e:#}")));
                            }
                        }
                    }
                    if edit {
                        last_edit = Instant::now();
                    }
                }
            }
        }
    }
}

/// Reads the register and live block; true when something changed.
fn poll(s: &mut State) -> Result<bool> {
    let register = s.pedal.read_register()?;
    let decoded = tankg::decode_register(&register);
    let bypassed = decoded.is_some_and(|r| r.bypassed);
    let live = if bypassed {
        None
    } else {
        Some(s.pedal.read_live()?)
    };
    if register != s.register {
        // a footswitch changed the preset or went to bypass
        s.register = register;
        s.pedal.bypassed = bypassed;
        if let Some(l) = &live {
            s.pedal.sounding = s.identify(l, s.pedal.sounding);
        } else if let (Some(b), Some(d)) = (s.pedal.sounding, decoded)
            && b / 4 != d.bank
        {
            s.pedal.sounding = None;
        }
        if live.is_some() {
            s.live = live;
        }
        return Ok(true);
    }
    match live {
        Some(l) if s.live != Some(l) => {
            s.live = Some(l);
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Runs one command; Some(message) when it finished something worth a toast.
fn apply(s: &mut State, c: Command) -> Result<Option<String>> {
    match c {
        Command::Connect { .. } | Command::Release => Ok(None),
        Command::Load(slot) => {
            s.pedal.load(slot)?;
            s.refresh(false)?;
            Ok(Some(format!("Playing {}", preset::name(slot))))
        }
        Command::Bypass(on) => {
            if on {
                s.pedal.bypass()?;
            } else {
                // the vendor editor turns bypass off by loading the preset again, as stored
                let slot = s
                    .pedal
                    .sounding
                    .context("the playing preset is not known")?;
                s.pedal.load(slot)?;
            }
            s.refresh(false)?;
            Ok(Some(if on {
                "Bypassed".into()
            } else {
                "Bypass off".into()
            }))
        }
        Command::Switch { offset, on } => {
            s.pedal.set_switch(offset, on)?;
            if let Some(l) = &mut s.live {
                l[offset] = on as u8;
            }
            Ok(None)
        }
        Command::Knobs(p) => {
            s.pedal.write_knobs(&p)?;
            if let Some(l) = &mut s.live {
                l[preset::KNOBS_OFFSET..preset::KNOBS_OFFSET + preset::KNOBS_LEN].copy_from_slice(
                    &p[preset::KNOBS_OFFSET..preset::KNOBS_OFFSET + preset::KNOBS_LEN],
                );
            }
            Ok(None)
        }
        Command::Save { slot, preset: p } => {
            s.backup_once()?;
            s.pedal.save(slot, &p)?;
            s.refresh(true)?;
            Ok(Some(format!("Saved to {}", preset::name(slot))))
        }
        Command::BackupTo(path) => {
            let table = s.pedal.read_table()?;
            std::fs::write(&path, &table).with_context(|| path.display().to_string())?;
            s.table = table;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(Some(format!("Saved {name}")))
        }
        Command::Restore { table, slots } => {
            s.backup_once()?;
            for slot in &slots {
                s.pedal.store(*slot, &preset::from_table(&table, *slot))?;
            }
            s.pedal.commit()?;
            // the sounding preset may have changed under it
            if let Some(slot) = s.pedal.sounding {
                s.pedal.load(slot)?;
            }
            s.refresh(true)?;
            Ok(Some(format!("Restored {} presets", slots.len())))
        }
    }
}
