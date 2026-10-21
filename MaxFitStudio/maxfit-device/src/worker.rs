//! The device worker thread.
//!
//! All HID I/O happens here, off the UI thread. That is not just tidiness: an
//! effect engine driven from the GUI's frame callback stops the moment the
//! window is minimised or hidden to tray, which is exactly when a lighting app
//! should keep running. The worker owns its own clock instead.
//!
//! `hidapi`'s handles are thread-affine, so the [`Device`] is constructed inside
//! the worker and never moved across threads.

use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use maxfit_core::effects::{AnimParams, AnimationKind, Animator};
use maxfit_core::{DeviceInfo, LedFrame, LightCapabilities, LightingState};

use crate::device::Device;
use crate::timer::Pacer;
use crate::{DeviceError, find_control_interface};

/// How often to look for the keyboard when it is not connected.
const RECONNECT_EVERY: Duration = Duration::from_millis(1000);
/// Idle poll interval when connected but not animating.
const IDLE_TICK: Duration = Duration::from_millis(120);

#[derive(Clone, Debug)]
pub enum DeviceCmd {
    Connect,
    Disconnect,
    /// Apply a firmware effect (`0x07`).
    SetLighting(LightingState),
    /// Upload one static per-key frame (`0x0F`) without starting an animation.
    ShowFrame(Box<LedFrame>),
    /// Store a per-key frame into an onboard slot (`0x09`).
    StoreFrame { slot: u8, frame: Box<LedFrame> },
    StartAnimation(AnimationKind, AnimParams),
    SetAnimationKind(AnimationKind),
    /// Coalesced: only the most recent one each tick is applied.
    SetAnimationParams(AnimParams),
    StopAnimation,
    SetTargetFps(u16),
    /// Put the lighting back to a known-good lit state.
    Restore,
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum DeviceEvt {
    Connected {
        path: String,
        info: Box<DeviceInfo>,
        caps: Option<LightCapabilities>,
    },
    Disconnected {
        reason: String,
    },
    Error(String),
    Stats(EngineStats),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EngineStats {
    pub target_fps: u16,
    pub actual_fps: f32,
    pub write_ms: f32,
    pub render_us: f32,
    pub dropped: u64,
}

/// Latest-wins preview slot. A channel would either back up at 60 Hz or need
/// manual draining; a 396-byte memcpy under a briefly-held mutex cannot.
#[derive(Default)]
pub struct Preview {
    pub frame: LedFrame,
    pub seq: u64,
    /// Set by the UI while the board widget is actually visible, so the worker
    /// does not pin the UI thread at full frame rate when nobody is looking.
    pub wanted: bool,
}

/// Handle to the worker, held by the UI.
pub struct DeviceLink {
    tx: Sender<DeviceCmd>,
    pub events: Receiver<DeviceEvt>,
    pub preview: Arc<Mutex<Preview>>,
    join: Option<JoinHandle<()>>,
}

impl DeviceLink {
    /// Spawns the worker. `wake` is called after each published frame so the UI
    /// can repaint; pass `egui::Context::request_repaint`.
    pub fn spawn(wake: impl Fn() + Send + 'static) -> Self {
        let (tx, cmd_rx) = channel::<DeviceCmd>();
        let (evt_tx, events) = channel::<DeviceEvt>();
        let preview = Arc::new(Mutex::new(Preview::default()));

        let p = Arc::clone(&preview);
        let join = std::thread::Builder::new()
            .name("maxfit-device".into())
            .spawn(move || Worker::new(evt_tx, p, Box::new(wake)).run(cmd_rx))
            .expect("spawn device worker");

        Self {
            tx,
            events,
            preview,
            join: Some(join),
        }
    }

    pub fn send(&self, cmd: DeviceCmd) {
        // A dead worker is reported through `events`; dropping here is fine.
        let _ = self.tx.send(cmd);
    }

    pub fn set_preview_wanted(&self, wanted: bool) {
        if let Ok(mut p) = self.preview.lock() {
            p.wanted = wanted;
        }
    }

    /// Asks the worker to stop and waits briefly for it.
    ///
    /// Bounded on purpose: blocking app exit on USB I/O is how you get a window
    /// that will not close.
    pub fn shutdown(&mut self) {
        self.send(DeviceCmd::Shutdown);
        if let Some(j) = self.join.take() {
            let deadline = Instant::now() + Duration::from_millis(400);
            while !j.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(10));
            }
            if j.is_finished() {
                let _ = j.join();
            }
        }
    }
}

impl Drop for DeviceLink {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Worker {
    dev: Option<Device>,
    anim: Option<Animator>,
    pacer: Pacer,
    target_fps: u16,
    evt: Sender<DeviceEvt>,
    preview: Arc<Mutex<Preview>>,
    wake: Box<dyn Fn() + Send>,
    /// Re-sent on exit so the board is left on a self-sustaining firmware
    /// effect rather than frozen on the last streamed frame.
    last_lighting: Option<LightingState>,
    want_connected: bool,
    next_reconnect: Instant,
    last_wake: Instant,
    last_stats: Instant,
    frame_times: Vec<f32>,
}

impl Worker {
    fn new(
        evt: Sender<DeviceEvt>,
        preview: Arc<Mutex<Preview>>,
        wake: Box<dyn Fn() + Send>,
    ) -> Self {
        Self {
            dev: None,
            anim: None,
            pacer: Pacer::new(30),
            target_fps: 30,
            evt,
            preview,
            wake,
            last_lighting: None,
            want_connected: true,
            next_reconnect: Instant::now(),
            last_wake: Instant::now(),
            last_stats: Instant::now(),
            frame_times: Vec::with_capacity(120),
        }
    }

    fn run(mut self, rx: Receiver<DeviceCmd>) {
        let start = Instant::now();
        let mut prev = start;
        let mut frame = LedFrame::new();

        loop {
            // Drain every pending command, coalescing the chatty ones so that
            // dragging a slider cannot build a backlog the worker walks through
            // one command per frame.
            let mut pending_params: Option<AnimParams> = None;
            let mut pending_fps: Option<u16> = None;
            loop {
                let cmd = if self.anim.is_some() {
                    match rx.try_recv() {
                        Ok(c) => Some(c),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => return self.finish(),
                    }
                } else {
                    // Idle: block briefly so we are not spinning.
                    match rx.recv_timeout(IDLE_TICK) {
                        Ok(c) => Some(c),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            return self.finish();
                        }
                    }
                };
                let Some(cmd) = cmd else { break };

                match cmd {
                    DeviceCmd::SetAnimationParams(p) => pending_params = Some(p),
                    DeviceCmd::SetTargetFps(f) => pending_fps = Some(f),
                    DeviceCmd::Shutdown => return self.finish(),
                    other => self.handle(other),
                }
            }
            if let Some(p) = pending_params
                && let Some(a) = &mut self.anim
            {
                a.params = p;
            }
            if let Some(f) = pending_fps {
                self.target_fps = f.clamp(1, 120);
                self.pacer.set_fps(self.target_fps);
            }

            if self.dev.is_none() {
                if self.want_connected && Instant::now() >= self.next_reconnect {
                    self.next_reconnect = Instant::now() + RECONNECT_EVERY;
                    self.try_connect();
                }
                continue;
            }

            let Some(anim) = &mut self.anim else { continue };

            // ---- render and stream one frame ----
            let now = Instant::now();
            let t = (now - start).as_secs_f32();
            let dt = (now - prev).as_secs_f32().min(0.25);
            prev = now;

            let r0 = Instant::now();
            anim.render(t, dt, &mut frame);
            let render_us = r0.elapsed().as_secs_f32() * 1e6;

            let w0 = Instant::now();
            let write = self.dev.as_mut().expect("checked above").present_frame(&frame);
            let write_ms = w0.elapsed().as_secs_f32() * 1000.0;

            if let Err(e) = write {
                self.on_io_error(e);
                continue;
            }

            self.publish(&frame, render_us, write_ms);
            self.pacer.wait_next();
        }
    }

    fn handle(&mut self, cmd: DeviceCmd) {
        match cmd {
            DeviceCmd::Connect => {
                self.want_connected = true;
                if self.dev.is_none() {
                    self.try_connect();
                }
            }
            DeviceCmd::Disconnect => {
                self.want_connected = false;
                self.anim = None;
                self.dev = None;
                let _ = self.evt.send(DeviceEvt::Disconnected {
                    reason: "closed by request".into(),
                });
            }
            DeviceCmd::SetLighting(s) => {
                // A firmware effect and a streamed animation fight over the
                // same LEDs, so selecting one stops the other.
                self.anim = None;
                self.last_lighting = Some(s);
                self.io(|d| d.set_lighting(&s));
            }
            DeviceCmd::ShowFrame(f) => {
                self.anim = None;
                self.io(|d| d.present_frame(&f));
                self.publish(&f, 0.0, 0.0);
            }
            DeviceCmd::StoreFrame { slot, frame } => {
                self.io(|d| d.store_frame(slot, &frame));
            }
            DeviceCmd::StartAnimation(kind, params) => {
                self.anim = Some(Animator::new(kind, params));
                self.pacer.set_fps(self.target_fps);
            }
            DeviceCmd::SetAnimationKind(kind) => match &mut self.anim {
                // Rebuild so per-effect state (rain drops) starts clean.
                Some(a) => *a = Animator::new(kind, a.params),
                None => self.anim = Some(Animator::new(kind, AnimParams::default())),
            },
            DeviceCmd::StopAnimation => {
                self.anim = None;
                if let Some(s) = self.last_lighting {
                    self.io(|d| d.set_lighting(&s));
                }
            }
            DeviceCmd::Restore => {
                self.anim = None;
                let s = LightingState::panic();
                self.last_lighting = Some(s);
                self.io(|d| d.set_lighting(&s));
            }
            // Handled by the caller.
            DeviceCmd::SetAnimationParams(_) | DeviceCmd::SetTargetFps(_) | DeviceCmd::Shutdown => {}
        }
    }

    fn try_connect(&mut self) {
        match Device::open() {
            Ok(mut dev) => {
                let path = dev.path().to_string();
                let info = dev.read_device_info().unwrap_or_default();
                let caps = dev.read_capabilities().ok().flatten();
                self.dev = Some(dev);
                let _ = self.evt.send(DeviceEvt::Connected {
                    path,
                    info: Box::new(info),
                    caps,
                });
            }
            Err(DeviceError::NotFound) => { /* keep polling quietly */ }
            Err(e) => {
                let _ = self.evt.send(DeviceEvt::Error(e.to_string()));
            }
        }
    }

    fn io(&mut self, f: impl FnOnce(&mut Device) -> Result<(), DeviceError>) {
        let Some(dev) = self.dev.as_mut() else { return };
        if let Err(e) = f(dev) {
            self.on_io_error(e);
        }
    }

    fn on_io_error(&mut self, e: DeviceError) {
        // hidapi flattens OS errors into a string, so confirm by re-enumerating
        // rather than trusting the message.
        let gone = e.is_disconnect() || matches!(find_control_interface(), Ok(None));
        if gone {
            self.dev = None;
            self.anim = None;
            self.next_reconnect = Instant::now() + RECONNECT_EVERY;
            let _ = self.evt.send(DeviceEvt::Disconnected {
                reason: e.to_string(),
            });
        } else {
            let _ = self.evt.send(DeviceEvt::Error(e.to_string()));
        }
    }

    fn publish(&mut self, frame: &LedFrame, render_us: f32, write_ms: f32) {
        let wanted = match self.preview.lock() {
            Ok(mut p) => {
                p.frame = frame.clone();
                p.seq = p.seq.wrapping_add(1);
                p.wanted
            }
            Err(_) => false,
        };

        self.frame_times.push(write_ms);
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }

        let now = Instant::now();
        // Repaint at most 30 Hz, and only while the board is on screen.
        if wanted && now.duration_since(self.last_wake) >= Duration::from_millis(33) {
            self.last_wake = now;
            (self.wake)();
        }

        if now.duration_since(self.last_stats) >= Duration::from_millis(500) {
            self.last_stats = now;
            let mean = if self.frame_times.is_empty() {
                0.0
            } else {
                self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
            };
            let _ = self.evt.send(DeviceEvt::Stats(EngineStats {
                target_fps: self.target_fps,
                actual_fps: if mean > 0.0 {
                    (1000.0 / mean).min(f32::from(self.target_fps))
                } else {
                    0.0
                },
                write_ms: mean,
                render_us,
                dropped: self.pacer.dropped,
            }));
        }
    }

    /// Leaves the board on something self-sustaining. Never exits with the
    /// lighting disabled: a blanked board is indistinguishable from dead
    /// hardware, which is exactly the trap that byte 17 sets.
    fn finish(mut self) {
        self.anim = None;
        let restore = self.last_lighting.unwrap_or_else(LightingState::panic);
        debug_assert!(restore.enabled, "must never exit with the lighting disabled");
        self.io(|d| d.set_lighting(&restore));
    }
}
