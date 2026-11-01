//! Recoverable checkpoints independent of whether a browser download was accepted.
//! Clean saves and dirty snapshots retain separate validated records and previous-valid fallbacks.
//! Imported media stay separate; checkpoints never promise to embed or repair missing sources.

use crate::opfs;
use crate::recovery_policy::{Action, Policy};
use crate::recovery_record::{self, MAX_BYTES};
use filmcraft_engine::Session;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

const SAVED: &str = "recovery/academy-saved-v1.json";
const SAVED_PREVIOUS: &str = "recovery/academy-saved-previous-v1.json";
const DIRTY: &str = "recovery/academy-dirty-v1.json";
const DIRTY_PREVIOUS: &str = "recovery/academy-dirty-previous-v1.json";
const KEEP_MAX: f64 = 4.0 * 1024.0 * 1024.0 * 1024.0;
type Done = Box<dyn FnOnce(bool)>;

struct Job {
    sequence: u64,
    dirty: bool,
    bytes: Vec<u8>,
    done: Vec<Done>,
}
#[derive(Default)]
struct Writer {
    pending: BTreeMap<bool, Job>,
    failed: BTreeMap<bool, Job>,
    active: bool,
    sequence: u64,
    clock: f64,
    completed: u64,
    error: Option<String>,
}
thread_local! { static WRITER: RefCell<Writer> = RefCell::new(Writer::default()); }

pub fn status() -> Value {
    WRITER.with(|w| {
        let w = w.borrow();
        json!({"available":opfs::available(),"pending":w.pending.len()+usize::from(w.active),"failed":w.failed.len(),"completed":w.completed,"error":w.error,
            "note":"Browser checkpoint acknowledgement does not confirm a downloaded file on disk"})
    })
}

fn queue(native: &[u8], dirty: bool, done: Option<Done>) -> Result<(), String> {
    if !opfs::available() {
        return Err("Durable browser storage is unavailable; download the native project".into());
    }
    let at = WRITER.with(|w| {
        let mut w = w.borrow_mut();
        w.clock = js_sys::Date::now().max(w.clock + 0.001);
        w.clock
    });
    let bytes = recovery_record::encode(native, dirty, at)?;
    WRITER.with(|w| {
        let mut w = w.borrow_mut();
        w.sequence = w.sequence.saturating_add(1);
        let mut callbacks = w.pending.remove(&dirty).map(|j| j.done).unwrap_or_default();
        callbacks.extend(done);
        w.failed.remove(&dirty);
        if w.failed.is_empty() {
            w.error = None;
        }
        let sequence = w.sequence;
        w.pending.insert(dirty, Job { sequence, dirty, bytes, done: callbacks });
    });
    start_writer();
    Ok(())
}

async fn write_record(job: &Job) -> Result<(), String> {
    let (current, previous) = if job.dirty { (DIRTY, DIRTY_PREVIOUS) } else { (SAVED, SAVED_PREVIOUS) };
    // Never replace the last valid fallback with a corrupt current record.
    if let Some(old) = opfs::read_limited(current, MAX_BYTES).await
        && recovery_record::decode(&old).is_ok()
    {
        let blob = crate::fs::bytes_to_blob(&old, "application/json").map_err(|e| format!("Recovery backup: {e:?}"))?;
        opfs::store_blob(previous, &blob).await.map_err(|e| format!("Recovery backup: {e:?}"))?;
    }
    let blob = crate::fs::bytes_to_blob(&job.bytes, "application/json").map_err(|e| format!("Recovery checkpoint: {e:?}"))?;
    opfs::store_blob(current, &blob).await.map_err(|e| format!("Recovery checkpoint: {e:?}"))
}

fn start_writer() {
    let start = WRITER.with(|w| {
        let mut w = w.borrow_mut();
        if w.active || w.pending.is_empty() {
            return false;
        }
        w.active = true;
        true
    });
    if !start {
        return;
    }
    wasm_bindgen_futures::spawn_local(async {
        loop {
            let next = WRITER.with(|w| {
                let mut w = w.borrow_mut();
                let key = w.pending.values().min_by_key(|j| j.sequence).map(|j| j.dirty);
                match key {
                    Some(key) => w.pending.remove(&key),
                    None => {
                        w.active = false;
                        None
                    }
                }
            });
            let Some(mut job) = next else { break };
            let result = write_record(&job).await;
            for callback in std::mem::take(&mut job.done) {
                callback(result.is_ok());
            }
            WRITER.with(|w| {
                let mut w = w.borrow_mut();
                match &result {
                    Ok(()) => {
                        w.completed = job.sequence;
                        if w.failed.get(&job.dirty).is_some_and(|j| j.sequence <= job.sequence) {
                            w.failed.remove(&job.dirty);
                        }
                        if w.failed.is_empty() {
                            w.error = None;
                        }
                    }
                    Err(error) => {
                        w.error = Some(error.clone());
                        w.failed.insert(job.dirty, job);
                    }
                }
            });
            if let Err(error) = result {
                log::warn!("{error}");
                crate::post(move |w, _| w.app.status(format!("Browser backup failed: {error}. Your project remains open; retry or download it.")));
            }
        }
    });
}

/// Queue a native Save independently of its download request. Success here means queued.
pub fn keep_saved(native: &[u8]) -> Result<(), String> {
    queue(native, false, None)
}

/// Capture current work without marking the engine saved or changing the document.
pub fn checkpoint(s: &Session) -> Result<(), String> {
    let bytes = filmcraft_format::encode_with_view(&s.project, Some(&s.project_view()), false);
    queue(&bytes, s.is_dirty(), None)
}

/// Wait for actual OPFS closes. Failures retain live work and checkpoint bytes for retry.
pub async fn flush() -> Result<Value, String> {
    WRITER.with(|w| {
        let mut w = w.borrow_mut();
        if !w.active {
            for (key, job) in std::mem::take(&mut w.failed) {
                w.pending.entry(key).or_insert(job);
            }
            w.error = None;
        }
    });
    start_writer();
    let start = web_time::Instant::now();
    while WRITER.with(|w| w.borrow().active) {
        if start.elapsed().as_secs_f64() > 30.0 {
            return Err("Browser backup is still pending; keep the editor open and retry".into());
        }
        crate::fs::sleep(8).await;
    }
    if !opfs::available() {
        return Err("Durable browser storage is unavailable; download the native project".into());
    }
    if let Some(error) = WRITER.with(|w| w.borrow().error.clone()) {
        return Err(error);
    }
    Ok(status())
}

enum Outcome {
    Snapshot { revision: u64, ok: bool },
    Clean { ok: bool },
}
pub struct Autosave {
    policy: Policy,
    outcomes: Rc<RefCell<Vec<Outcome>>>,
    clock: web_time::Instant,
}
impl Default for Autosave {
    fn default() -> Self {
        Self { policy: Policy::default(), outcomes: Rc::default(), clock: web_time::Instant::now() }
    }
}
impl Autosave {
    pub fn tick(&mut self, s: &Session, ctx: &egui::Context) {
        if !opfs::available() {
            return;
        }
        let now = self.clock.elapsed().as_secs_f64();
        for outcome in self.outcomes.borrow_mut().drain(..) {
            match outcome {
                Outcome::Snapshot { revision, ok } => self.policy.written(revision, ok, now),
                Outcome::Clean { ok } => self.policy.cleaned(ok, now),
            }
        }
        let (dirty, revision) = match self.policy.tick(now, s.revision, s.saved_revision) {
            Action::Nothing => return,
            Action::WakeAfter(secs) => {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(secs));
                return;
            }
            Action::WriteSnapshot(revision) => (true, Some(revision)),
            Action::WriteClean => (false, None),
        };
        let bytes = filmcraft_format::encode_with_view(&s.project, Some(&s.project_view()), false);
        let (outcomes, repaint) = (self.outcomes.clone(), ctx.clone());
        let done: Done = Box::new(move |ok| {
            outcomes.borrow_mut().push(match revision {
                Some(revision) => Outcome::Snapshot { revision, ok },
                None => Outcome::Clean { ok },
            });
            repaint.request_repaint();
        });
        if let Err(error) = queue(&bytes, dirty, Some(done)) {
            self.outcomes.borrow_mut().push(match revision {
                Some(revision) => Outcome::Snapshot { revision, ok: false },
                None => Outcome::Clean { ok: false },
            });
            log::warn!("{error}");
        }
    }
}

/// Load the newest valid clean/dirty checkpoint, then previous-valid or legacy fallback.
/// Missing media is reported by normal project opening; it never substitutes the demo.
pub async fn load_snapshot() -> Option<(String, Vec<u8>, bool)> {
    if !opfs::available() {
        return None;
    }
    let mut candidates = Vec::new();
    for path in [SAVED, DIRTY, SAVED_PREVIOUS, DIRTY_PREVIOUS] {
        if let Some(bytes) = opfs::read_limited(path, MAX_BYTES).await {
            candidates.push(bytes);
        }
    }
    if let Some(record) = recovery_record::latest(candidates) {
        WRITER.with(|w| w.borrow_mut().clock = record.at);
        let name: String = record.name.chars().take(240).map(|c| if c == '/' || c == '\\' || c.is_control() { '-' } else { c }).collect();
        return Some((format!("{name}.fcproj"), record.native, record.dirty));
    }
    // The old dirty pair is read only, never deleted or marked clean by this implementation.
    let meta: Value = serde_json::from_slice(&opfs::read_limited("recovery/meta.json", 64 * 1024).await?).ok()?;
    if meta.get("dirty").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let bytes = opfs::read_limited("recovery/snapshot.fcproj", MAX_BYTES).await?;
    let loaded = filmcraft_format::decode(&bytes).ok()?;
    Some((format!("{}.fcproj", loaded.project.name), bytes, true))
}

/// Keep an imported media copy; project records themselves do not embed source media.
pub fn keep_media(path: &str, file: &web_sys::File) {
    if !opfs::available() || file.size() > KEEP_MAX {
        return;
    }
    let Some(name) = path.strip_prefix("/files/").map(str::to_string) else { return };
    let blob: web_sys::Blob = file.clone().into();
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = opfs::store_blob(&format!("media/{name}"), &blob).await {
            log::warn!("keeping {name} in OPFS: {e:?}");
        }
    });
}
pub async fn restore_media() -> usize {
    let files = opfs::files_in("media").await;
    let n = files.len();
    for (name, file) in files {
        crate::fs::register_blob(&name, file.into());
    }
    n
}
