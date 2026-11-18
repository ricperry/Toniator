//! Opt-in bounded Main Canvas qualification. No artwork, scheduling or pixel authority lives here.
use gtk::{gdk, glib};
use std::{
    cell::{Cell, RefCell},
    io::Write,
    sync::{OnceLock, mpsc},
    time::Duration,
};
use toniator_engine::{
    PreviewRasterTarget,
    preview_cache::{Artwork, Provenance},
};

/// Immutable requested target and accepted consumer identity; epochs never stand in for revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub artwork: Artwork,
    pub raster: PreviewRasterTarget,
    pub provenance: Provenance,
}
/// Successful live draw facts; callers already verify primary exact coverage and current authority.
pub(crate) enum Draw {
    Tiles {
        artwork: Artwork,
        provenance: Provenance,
        scale: f64,
        background: toniator_engine::RasterBackground,
    },
    Exact(Target),
}
type Observer = Box<dyn Fn(Draw)>;
thread_local! {
    static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
    static OFFSCREEN: Cell<u32> = const { Cell::new(0) };
    static STATE: RefCell<State> = RefCell::new(State::default());
}
/// One latest request and one feedback candidate bound memory independently of pan frequency.
#[derive(Default)]
struct State {
    next: u64,
    request: Option<Request>,
    candidate: Option<Candidate>,
    connected: Option<gdk::FrameClock>,
    painting_frame: Option<i64>,
}
struct Request {
    id: u64,
    target: Target,
    start: i64,
    captured: bool,
}
struct Candidate {
    id: u64,
    frame: i64,
    snapshot: i64,
    after: Option<i64>,
    clock: gdk::FrameClock,
}

/// Opens a finite nonblocking evidence channel once; normal runs create no writer or GTK timer.
fn sender() -> Option<&'static mpsc::SyncSender<serde_json::Value>> {
    static SENDER: OnceLock<Option<mpsc::SyncSender<serde_json::Value>>> = OnceLock::new();
    SENDER
        .get_or_init(|| {
            let path = std::env::var_os("TONIATOR_PREVIEW_MEASUREMENTS")?;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()?;
            let (send, receive) = mpsc::sync_channel::<serde_json::Value>(256);
            std::thread::Builder::new()
                .name("preview-evidence".into())
                .spawn(move || {
                    for record in receive {
                        if serde_json::to_writer(&mut file, &record).is_err()
                            || file.write_all(b"\n").is_err()
                        {
                            break;
                        }
                    }
                })
                .ok()?;
            Some(send)
        })
        .as_ref()
}
/// Reports whether explicit bounded observation is enabled; it does not change product decisions.
pub(crate) fn enabled() -> bool {
    sender().is_some()
}
/// Tries to queue one observation without waiting on IO; any overflow invalidates timing evidence.
pub(crate) fn emit(record: serde_json::Value) {
    if let Some(sender) = sender()
        && sender.try_send(record).is_err()
    {
        eprintln!("preview-measurement-overflow");
    }
}
/// Installs a weak Main-owned fact validator once; timer polls only existing GDK feedback.
pub(crate) fn install(observer: impl Fn(Draw) + 'static) {
    if !enabled() {
        return;
    }
    OBSERVER.with(|slot| *slot.borrow_mut() = Some(Box::new(observer)));
    glib::timeout_add_local(Duration::from_millis(8), || {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let Some(candidate) = state.candidate.as_ref() else { return; };
            let now = glib::monotonic_time();
            let timing = candidate.clock.timings(candidate.frame);
            let presentation = timing.as_ref().filter(|t| t.is_complete()).map_or(0, |t| t.presentation_time());
            if candidate.after.is_some() && presentation > 0 {
                emit(serde_json::json!({"event":"gdk_presented","id":candidate.id,"frame":candidate.frame,"snapshot_us":candidate.snapshot,"after_paint_us":candidate.after,"presentation_us":presentation,"observed_us":now}));
                state.candidate = None;
            } else if now - candidate.snapshot > 2_000_000 {
                emit(serde_json::json!({"event":"presentation_unavailable","id":candidate.id,"frame":candidate.frame,"snapshot_us":candidate.snapshot,"after_paint_us":candidate.after,"observed_us":now}));
                state.candidate = None;
            }
        });
        glib::ControlFlow::Continue
    });
}
/// Arms the latest exact Main request, preserving its start across repeated same-target planning.
/// A changed epoch invalidates previous candidates; at most 20,000 requests are observed per run.
pub(crate) fn request(target: Target, statistics: String) {
    if !enabled() {
        return;
    }
    STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        if let Some(old) = state.request.as_mut() && old.target.artwork == target.artwork && old.target.raster == target.raster && old.target.provenance.requested == target.provenance.requested {
            if old.target.provenance != target.provenance { old.target = target; old.captured = false; state.candidate = None; }
            return;
        }
        if state.next >= 20_000 { return; }
        if let Some(old) = state.request.take() && !old.captured { emit(serde_json::json!({"event":"superseded","id":old.id,"observed_us":glib::monotonic_time()})); }
        state.candidate = None;
        state.next += 1;
        let request = Request { id: state.next, target, start: glib::monotonic_time(), captured: false };
        emit(serde_json::json!({"event":"request","id":request.id,"start_us":request.start,"artwork":target.artwork,"target":format!("{:?}",target.raster),"viewport":target.raster.viewport(),"size":[target.raster.width(),target.raster.height()],"provenance":format!("{:?}",target.provenance),"statistics":statistics}));
        state.request = Some(request);
    });
}
/// Suppresses observer calls while GDK freezes a paintable offscreen for retained history.
pub(crate) fn offscreen<T>(f: impl FnOnce() -> T) -> T {
    OFFSCREEN.with(|v| v.set(v.get() + 1));
    let result = f();
    OFFSCREEN.with(|v| v.set(v.get() - 1));
    result
}
/// Delivers only real current Canvas draw facts; frozen/history projections never call this.
pub(crate) fn draw(facts: Draw) {
    if !enabled() || OFFSCREEN.with(|v| v.get() != 0) {
        return;
    }
    OBSERVER.with(|slot| {
        if let Some(observer) = slot.borrow().as_ref() {
            observer(facts);
        }
    });
}
/// Tracks a real widget clock's active frame cycle; snapshots outside it cannot become endpoints.
/// Signals are installed once per owned clock and never request paint or predict presentation.
pub(crate) fn track_clock(clock: &gdk::FrameClock) {
    if !enabled() {
        return;
    }
    STATE.with(|slot| {
        let mut state=slot.borrow_mut();
        if state.connected.as_ref()==Some(clock) { return; }
        clock.connect_before_paint(|clock| STATE.with(|s| {
            let mut state=s.borrow_mut();
            if state.connected.as_ref()==Some(clock) { state.painting_frame=Some(clock.frame_counter()); }
        }));
        clock.connect_after_paint(|clock| {
            STATE.with(|slot| {
                let mut state=slot.borrow_mut();
                if state.connected.as_ref()!=Some(clock) { return; }
                if let Some(candidate)=state.candidate.as_mut() && candidate.clock==*clock && candidate.frame==clock.frame_counter() && candidate.after.is_none() {
                    let now=glib::monotonic_time();candidate.after=Some(now);
                    emit(serde_json::json!({"event":"gtk_after_paint","id":candidate.id,"frame":candidate.frame,"after_paint_us":now}));
                }
                state.painting_frame=None;
            });
        });
        state.connected=Some(clock.clone());
        state.painting_frame=None;
    });
}
/// Captures successful exact Canvas snapshot and its SAME frame-clock counter after Main validation.
/// Feedback polling is bounded and never substitutes predicted presentation or an offscreen draw.
pub(crate) fn canvas(target: Target, clock: &gdk::FrameClock, statistics: String) {
    STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        if state.connected.as_ref()!=Some(clock) || state.painting_frame!=Some(clock.frame_counter()) { return; }
        let Some(request) = state.request.as_mut() else { return; };
        if request.target != target || request.captured { return; }
        request.captured = true;
        let (id, start) = (request.id, request.start);
        let now = glib::monotonic_time();
        let frame = clock.frame_counter();
        emit(serde_json::json!({"event":"canvas_exact_snapshot","id":id,"frame":frame,"snapshot_us":now,"start_us":start,"provenance":format!("{:?}",target.provenance),"statistics":statistics}));
        state.candidate = Some(Candidate { id, frame, snapshot: now, after: None, clock: clock.clone() });
    });
}
/// Exposes the last armed/captured identity to the in-process integration witness only.
#[cfg(test)]
pub(crate) fn captured() -> Option<(u64, Target, bool)> {
    STATE.with(|s| {
        s.borrow()
            .request
            .as_ref()
            .map(|r| (r.id, r.target, r.captured))
    })
}
/// Reports whether the current candidate still awaits bounded GDK feedback in the integration witness.
#[cfg(test)]
pub(crate) fn feedback_pending() -> bool {
    STATE.with(|s| s.borrow().candidate.is_some())
}
