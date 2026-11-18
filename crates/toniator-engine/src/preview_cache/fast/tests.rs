use super::*;
use toniator_domain::{CanvasSpec, ChannelId, ColorValue, PatternMechanismId};
use toniator_patterns::{
    CanonicalFillRule, CanonicalMark, FamilySiteId, FamilySiteProvenance, Point2, SiteScope,
};
use toniator_render::{GeometryOutput, RenderLayer};

/// Builds a current solid circle scene; unsupported fixtures retain an explicit empty stroke output.
/// # Panics
/// Panics if the fixture violates renderer-owned scene validation.
fn scene(width: f64, unsupported: bool) -> Arc<RenderScene> {
    let geometry = if unsupported {
        GeometryOutput::CanonicalStrokes(Vec::new())
    } else {
        GeometryOutput::CanonicalMarks(vec![CanonicalMark::Circle {
            source_site_id: FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            },
            center: Point2::new(width / 2.0, 32.0),
            radius: 8.0,
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
            fill_rule: CanonicalFillRule::NonZero,
        }])
    };
    Arc::new(
        RenderScene::new_modeled(
            CanvasSpec {
                width,
                height: 64.0,
            },
            "cache-test".into(),
            format!("cache-{unsupported}"),
            toniator_domain::HalftoneChannelModel::Rgb,
            vec![
                RenderLayer::new(
                    ChannelId(1),
                    true,
                    ColorValue {
                        red: 0.2,
                        green: 0.4,
                        blue: 0.8,
                        alpha: 1.0,
                    },
                    1.0,
                    geometry,
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    )
}
/// Installs accepted immutable authority before selecting approximate request mode.
fn cache(width: f64) -> PreviewCache {
    let mut cache = PreviewCache::default();
    cache.accept_scene((1, 2, 3), scene(width, false), 4096);
    cache.set_quality(Quality::Fast);
    cache
}
/// Constructs a native request in the current full backend/epoch namespace.
fn key(cache: &PreviewCache, scale: f64) -> RootKey {
    RootKey::new(
        scale,
        RasterBackground::Transparent,
        RasterizationLimits::default(),
    )
    .with_provenance(cache.provenance())
}
/// Delivers typed fake worker facts through the REAL publication/resource reconciliation boundary.
fn deliver(cache: &mut PreviewCache, job: Job, outcome: Outcome, seq: u64, charge: usize) {
    cache.complete(Completion {
        artwork: job.artwork,
        key: job.key,
        authority: job.cancelled,
        presentation: job.presentation,
        kind: Completed::Fast(FastCompleted {
            sequence: seq,
            charge,
            outcome,
        }),
    });
}
/// Acknowledges queued retirement before returning the next purpose-specific Fast command.
/// # Panics
/// Panics if the cache has no expected work or dispatches an unrelated CPU command.
fn next(cache: &mut PreviewCache, seq: &mut u64) -> Job {
    loop {
        let job = cache.take_job().expect("queued cache work");
        if matches!(&job.kind, JobKind::Fast(FastJob::Retire { .. })) {
            *seq += 1;
            deliver(cache, job, Outcome::Retired, *seq, 0);
            continue;
        }
        if matches!(&job.kind, JobKind::Retire { .. }) {
            let result = job.run(&AtomicBool::new(false));
            cache.complete(result);
            continue;
        }
        return job;
    }
}
/// Completes admission/preparation with metadata-only fixtures; native object creation remains absent.
/// # Panics
/// Panics if planning does not preserve the two-phase worker/retirement protocol.
fn prepare(cache: &mut PreviewCache, key: RootKey, region: Region, seq: &mut u64) {
    assert!(matches!(
        cache.plan_fast((1, 2, 3), key, region).unwrap(),
        FastPlan::Waiting
    ));
    let job = next(cache, seq);
    assert!(matches!(job.kind, JobKind::Fast(FastJob::Admit { .. })));
    *seq += 1;
    deliver(
        cache,
        job,
        Outcome::Admitted {
            frame: Frame {
                width: cache.scene.as_ref().unwrap().canvas().width as u32,
                height: 64,
                scale: 1.0,
                background: RasterBackground::Transparent,
            },
            required: 64 * 1024 * 1024,
        },
        *seq,
        0,
    );
    let job = next(cache, seq);
    assert!(matches!(job.kind, JobKind::Fast(FastJob::Prepare)));
    *seq += 1;
    deliver(
        cache,
        job,
        Outcome::Prepared(Facts {
            frame: Frame {
                width: cache.scene.as_ref().unwrap().canvas().width as u32,
                height: 64,
                scale: 1.0,
                background: RasterBackground::Transparent,
            },
            required: 64 * 1024 * 1024,
            device: 17,
            root: 23,
            initialized: true,
        }),
        *seq,
        64 * 1024 * 1024,
    );
}
/// Produces one native-gutter fake paint through normal immutable completion acceptance.
/// # Panics
/// Panics if expected paint work is missing or fixture dimensions are invalid.
fn paint(cache: &mut PreviewCache, seq: &mut u64) {
    let job = next(cache, seq);
    let JobKind::Fast(FastJob::Paint { region }) = &job.kind else {
        panic!("expected cell paint")
    };
    let region = *region;
    let surface = RasterSurface::new(
        region.width,
        region.height,
        vec![0; region.width as usize * region.height as usize * 4],
    )
    .unwrap();
    *seq += 1;
    deliver(
        cache,
        job,
        Outcome::Painted {
            region,
            surface,
            device: 17,
            root: 23,
        },
        *seq,
        64 * 1024 * 1024,
    );
}
/// Proves sparse Fast cells never publish or mix, while whole-region assembly grants no CPU certificate.
/// # Panics
/// Panics if partial Fast work appears Ready or an epoch/backend mutation retains current permission.
#[test]
fn fast_cache_atomic_whole_region_permission_and_namespace() {
    let mut cache = cache(3200.0);
    let key = key(&cache, 1.0);
    let region = Region {
        x: 0,
        y: 0,
        width: 3200,
        height: 64,
    };
    let mut seq = 0;
    prepare(&mut cache, key, region, &mut seq);
    for _ in 0..2 {
        paint(&mut cache, &mut seq);
        assert!(matches!(
            cache.plan_fast((1, 2, 3), key, region).unwrap(),
            FastPlan::Waiting
        ));
    }
    paint(&mut cache, &mut seq);
    assert!(matches!(
        cache.plan_fast((1, 2, 3), key, region).unwrap(),
        FastPlan::Waiting
    ));
    let job = next(&mut cache, &mut seq);
    let JobKind::Fast(FastJob::Assemble { region, pixels }) = &job.kind else {
        panic!("whole assembly")
    };
    let region = *region;
    let bytes = copy_pixels(region, pixels).unwrap();
    let surface = RasterSurface::new(region.width, region.height, bytes.clone()).unwrap();
    seq += 1;
    deliver(
        &mut cache,
        job,
        Outcome::Assembled {
            region,
            surface,
            upload: bytes,
        },
        seq,
        64 * 1024 * 1024,
    );
    let FastPlan::Ready(mut view) = cache.plan_fast((1, 2, 3), key, region).unwrap() else {
        panic!("complete view")
    };
    assert!(matches!(view.permission, ViewPermission::Fast(_)));
    assert!(view.complete && view.is_current() && view.covers(region));
    let id = view.pixels[0].identity();
    assert_ne!(
        id,
        PixelIdentity {
            provenance: Provenance::default(),
            id: id.id
        }
    );
    view.key.provenance.algorithm = Algorithm::AccurateAreaV1;
    assert!(!view.is_current());
}
/// Proves post-toggle upload/paint cannot publish and worker charges survive until actual ACK.
/// # Panics
/// Panics if stale completion changes pixels, clears accounting early or current intent is mislabeled.
#[test]
fn fast_cache_late_completion_retains_charge_until_retirement_ack() {
    let mut cache = cache(64.0);
    let key = key(&cache, 1.0);
    let region = Region {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    let mut seq = 0;
    prepare(&mut cache, key, region, &mut seq);
    let late = next(&mut cache, &mut seq);
    cache.set_quality(Quality::Accurate);
    assert_eq!(cache.statistics().fast_reserved_bytes, 64 * 1024 * 1024);
    let surface = RasterSurface::new(64, 64, vec![0; 64 * 64 * 4]).unwrap();
    seq += 1;
    deliver(
        &mut cache,
        late,
        Outcome::Painted {
            region,
            surface,
            device: 17,
            root: 23,
        },
        seq,
        64 * 1024 * 1024,
    );
    assert!(cache.fast.entry.is_none());
    assert_eq!(cache.provenance().effective, Quality::Accurate);
    assert_eq!(cache.statistics().fast_reserved_bytes, 64 * 1024 * 1024);
    let retire = cache.take_job().unwrap();
    seq += 1;
    deliver(&mut cache, retire, Outcome::Retired, seq, 0);
    assert_eq!(cache.statistics().fast_reserved_bytes, 0);
}
/// Proves explicit None rebinding accepts same-scene work but an unobserved revision replacement does not.
/// # Panics
/// Panics if the guard infers revision authority from an equal fingerprint or rejects explicit None reuse.
#[test]
fn fast_cache_none_rebind_preserves_scene_jobs_and_unobserved_edit_rejects() {
    let mut cache = cache(64.0);
    let key = key(&cache, 1.0);
    let region = Region {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    let mut seq = 0;
    prepare(&mut cache, key, region, &mut seq);
    let job = next(&mut cache, &mut seq);
    let canonical = cache.scene.as_ref().unwrap().clone();
    assert!(cache.rebind_unchanged((1, 2, 3), (1, 3, 3)));
    seq += 1;
    deliver(
        &mut cache,
        job,
        Outcome::Painted {
            region,
            surface: RasterSurface::new(64, 64, vec![0; 64 * 64 * 4]).unwrap(),
            device: 17,
            root: 23,
        },
        seq,
        64 * 1024 * 1024,
    );
    assert_eq!(cache.fast.entry.as_ref().unwrap().cells.len(), 1);
    assert!(Arc::ptr_eq(cache.scene.as_ref().unwrap(), &canonical));
    let stale = next(&mut cache, &mut seq);
    cache.accept_scene((1, 4, 3), canonical, 4096);
    seq += 1;
    deliver(
        &mut cache,
        stale,
        Outcome::Assembled {
            region,
            surface: RasterSurface::new(64, 64, vec![0; 64 * 64 * 4]).unwrap(),
            upload: vec![0; 64 * 64 * 4],
        },
        seq,
        64 * 1024 * 1024,
    );
    assert!(cache.fast.entry.is_none());
}
/// Proves target/resource failures include backdrop/work-policy/projection while device/content scope is wider.
/// # Panics
/// Panics if a failure leaks across complete target contracts or pan-like equal requests retry a device.
#[test]
fn fast_cache_failure_intent_has_full_contract_and_typed_retry_scope() {
    let stamp = Provenance::fast(1, 7);
    let base = RootKey::new(
        4.0,
        RasterBackground::Transparent,
        RasterizationLimits::default(),
    )
    .with_provenance(stamp);
    let latch = Latch {
        intent: FailureIntent {
            scene: "scene".into(),
            key: base,
        },
        reason: FallbackReason::Resource("cap".into()),
    };
    assert!(matches_latch(&latch, "scene", base, 7));
    let white = RootKey::new(
        4.0,
        RasterBackground::OpaqueWhite,
        RasterizationLimits::default(),
    )
    .with_provenance(stamp);
    assert!(!matches_latch(&latch, "scene", white, 7));
    let work = RootKey::new(
        4.0,
        RasterBackground::Transparent,
        RasterizationLimits::new(123).unwrap(),
    )
    .with_provenance(stamp);
    assert!(!matches_latch(&latch, "scene", work, 7));
    let fit = base.with_target(toniator_render::PreviewRasterTarget::new(64, 64).unwrap());
    assert!(!matches_latch(&latch, "scene", fit, 7));
    let device = Latch {
        reason: FallbackReason::Unavailable("driver".into()),
        ..latch.clone()
    };
    assert!(matches_latch(&device, "other-scene", fit, 7));
    assert!(!matches_latch(&device, "other-scene", fit, 8));
    let content = Latch {
        reason: FallbackReason::Unsupported("marks".into()),
        ..latch
    };
    assert!(matches_latch(&content, "scene", white, 8));
    assert!(!matches_latch(&content, "other", white, 7));
}
/// Proves Clear drops scene references on the worker and charges checked-out retirement until ACK.
/// # Panics
/// Panics if clearing merely queues a drop while prematurely reporting its payload released.
#[test]
fn fast_cache_clear_notifies_retirement_without_an_accepted_native_root() {
    let mut cache = cache(64.0);
    let canonical = Arc::downgrade(cache.scene.as_ref().unwrap());
    cache.clear();
    assert!(cache.queued());
    assert!(canonical.upgrade().is_some());
    let mut worker = WorkerState::default();
    while let Some(job) = cache.take_job() {
        let charge = cache.statistics().retired_root_payload_bytes;
        let result = worker.run(job, &AtomicBool::new(false));
        if charge > 0 {
            assert!(cache.statistics().retired_root_payload_bytes >= charge);
        }
        cache.complete(result);
    }
    assert!(canonical.upgrade().is_none());
    assert_eq!(cache.statistics().retired_root_payload_bytes, 0);
    let ack = worker.finish();
    cache.worker_retired(ack);
    assert_eq!(cache.statistics().fast_reserved_bytes, 0);
}

#[cfg(feature = "fast-preview")]
/// Proves unsupported complete scenes preserve warm CPU geometry/certificates and never initialize a GPU.
/// # Panics
/// Panics if structural refusal discards compatible Accurate roots or a fallback namespace says Fast.
#[test]
fn fast_cache_unsupported_preserves_warm_accurate_root() {
    let mut cache = PreviewCache::default();
    cache.accept_scene((1, 2, 3), scene(64.0, true), 4096);
    let cpu = key(&cache, 1.0);
    let region = Region {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    let mut worker = WorkerState::default();
    loop {
        if matches!(cache.plan((1, 2, 3), cpu, region).unwrap(), Plan::Ready(_)) {
            break;
        }
        let job = cache.take_job().unwrap();
        cache.complete(worker.run(job, &AtomicBool::new(false)));
    }
    let root = cache.entries[0].root.as_ref().unwrap().clone();
    cache.set_quality(Quality::Fast);
    let fast = key(&cache, 1.0);
    assert!(matches!(
        cache.plan_fast((1, 2, 3), fast, region).unwrap(),
        FastPlan::Waiting
    ));
    while let Some(job) = cache.take_job() {
        cache.complete(worker.run(job, &AtomicBool::new(false)));
        if cache.provenance().effective == Quality::Accurate {
            break;
        }
    }
    assert!(matches!(
        cache.plan_fast((1, 2, 3), fast, region).unwrap(),
        FastPlan::CpuFallback {
            reason: FallbackReason::Unsupported(_),
            ..
        }
    ));
    assert!(Arc::ptr_eq(&root, cache.entries[0].root.as_ref().unwrap()));
    let rebound = key(&cache, 1.0);
    assert!(matches!(
        cache.plan((1, 2, 3), rebound, region).unwrap(),
        Plan::Ready(_)
    ));
    assert_eq!(cache.statistics().fast_device_initializations, 0);
}

#[cfg(feature = "fast-preview")]
/// Proves unavailable initialization is attempted once across pans/scene edits and once on explicit OFF→ON.
/// # Panics
/// Panics if automatic cache replanning retries a failed adapter or explicit retry stays latched.
#[test]
fn fast_cache_initialization_failure_latches_until_explicit_retry() {
    let mut cache = cache(64.0);
    let attempts = Arc::new(AtomicUsize::new(0));
    let mut worker = WorkerState {
        unavailable_probe: Some(attempts.clone()),
        ..Default::default()
    };
    let region = Region {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
    };
    let first = key(&cache, 1.0);
    assert!(matches!(
        cache.plan_fast((1, 2, 3), first, region).unwrap(),
        FastPlan::Waiting
    ));
    while let Some(job) = cache.take_job() {
        cache.complete(worker.run(job, &AtomicBool::new(false)));
    }
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
    for x in [0, 4, 12] {
        assert!(matches!(
            cache
                .plan_fast(
                    (1, 2, 3),
                    first,
                    Region {
                        x,
                        y: 0,
                        width: 32,
                        height: 32
                    }
                )
                .unwrap(),
            FastPlan::CpuFallback { .. }
        ));
    }
    cache.accept_scene((1, 4, 3), scene(64.0, false), 4096);
    assert!(matches!(
        cache.plan_fast((1, 4, 3), first, region).unwrap(),
        FastPlan::CpuFallback { .. }
    ));
    while let Some(job) = cache.take_job() {
        cache.complete(worker.run(job, &AtomicBool::new(false)));
    }
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
    cache.set_quality(Quality::Accurate);
    cache.set_quality(Quality::Fast);
    let retry = key(&cache, 1.0);
    assert!(matches!(
        cache.plan_fast((1, 4, 3), retry, region).unwrap(),
        FastPlan::Waiting
    ));
    while let Some(job) = cache.take_job() {
        cache.complete(worker.run(job, &AtomicBool::new(false)));
    }
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
}

/// Proves transient CPU/GTK occupancy waits for both retirement acknowledgements instead of latching refusal.
/// # Panics
/// Panics if intrinsic admission counts purgeable inactive payload or preparation runs before ownership drops.
#[test]
fn fast_cache_two_phase_retirement_charge_continuity() {
    let mut c = PreviewCache::default();
    c.accept_scene((1, 2, 3), scene(64., false), 4096);
    let cpu = key(&c, 1.);
    let r = Region {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    assert!(matches!(c.plan((1, 2, 3), cpu, r).unwrap(), Plan::Waiting));
    let job = c.take_job().unwrap();
    c.complete(job.run(&AtomicBool::new(false)));
    c.entries[0].prepared_bytes = 2_000_000_000;
    c.entries[0].uploads.push((71, r, vec![0; 4096]));
    c.textures_with_retained(100_000_000, 0, 1024, None);
    c.set_quality(Quality::Fast);
    let k = key(&c, 4.);
    let mut seq = 0;
    c.plan_fast((1, 2, 3), k, r).unwrap();
    let job = next(&mut c, &mut seq);
    assert!(
        matches!(job.kind,JobKind::Fast(FastJob::Admit{available,..}) if available>7_696_000_000)
    );
    deliver(
        &mut c,
        job,
        Outcome::Admitted {
            frame: Frame {
                width: 256,
                height: 256,
                scale: 4.,
                background: RasterBackground::Transparent,
            },
            required: 7_696_000_000,
        },
        2,
        0,
    );
    let before = c.statistics().retired_root_payload_bytes;
    assert!(before >= 2_000_004_096);
    let retired = c.take_job().unwrap();
    assert!(matches!(retired.kind, JobKind::Retire { .. }));
    assert_eq!(before, c.statistics().retired_root_payload_bytes);
    assert!(!c.fast_prepare_ready());
    c.complete(retired.run(&AtomicBool::new(false)));
    assert!(!c.queued());
    assert!(c.take_job().is_none());
    assert!(c.fallback_reason().is_none());
    c.textures_with_retained(1024, 0, 1024, None);
    assert!(c.queued());
    assert!(matches!(
        c.take_job().unwrap().kind,
        JobKind::Fast(FastJob::Prepare)
    ));
    assert_eq!(c.statistics().fast_reserved_bytes, 7_696_000_000);
    c.set_quality(Quality::Accurate);
    assert_eq!(
        c.statistics().fast_reserved_bytes,
        7_696_000_000,
        "queued cancellation does not release checked-out preparation"
    );
    let retirement = c.take_job().unwrap();
    deliver(&mut c, retirement, Outcome::Retired, 3, 0);
    assert_eq!(c.statistics().fast_reserved_bytes, 0);
}

/// Reconciles checked-out mutable CPU certification across a Fast refusal without reviving stale views.
/// # Panics
/// Panics if a preserved numerical root is stranded with invalid generation or its pixels acquire Fast authority.
#[test]
fn fast_cache_checked_out_cpu_certification_reconciles_after_fallback() {
    let mut c = PreviewCache::default();
    c.accept_scene((1, 2, 3), scene(64., true), 4096);
    let cpu = key(&c, 1.);
    let r = Region {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    c.plan((1, 2, 3), cpu, r).unwrap();
    let prepare = c.take_job().unwrap();
    c.complete(prepare.run(&AtomicBool::new(false)));
    c.plan((1, 2, 3), cpu, r).unwrap();
    let certify = c.take_job().unwrap();
    assert!(matches!(certify.kind, JobKind::Certify { .. }));
    let root = c.entries[0].root.as_ref().unwrap().clone();
    let old_authority = c.fast.authority();
    c.set_quality(Quality::Fast);
    let fast = key(&c, 1.);
    let mut seq = 0;
    c.plan_fast((1, 2, 3), fast, r).unwrap();
    let admission = next(&mut c, &mut seq);
    deliver(
        &mut c,
        admission,
        Outcome::Failed(FallbackReason::Unsupported("fixture strokes".into())),
        1,
        0,
    );
    c.entries[0].validity.store(0, Ordering::Release); // the checked-out mutable operation owns this lease.
    c.complete(certify.run(&AtomicBool::new(false)));
    assert!(old_authority.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(&root, c.entries[0].root.as_ref().unwrap()));
    let accurate = key(&c, 1.);
    let mut ready = false;
    for _ in 0..32 {
        if let Plan::Ready(v) = c.plan((1, 2, 3), accurate, r).unwrap() {
            assert_eq!(v.key.provenance.effective, Quality::Accurate);
            assert!(matches!(v.permission, ViewPermission::Accurate(_)));
            ready = true;
            break;
        }
        let Some(job) = c.take_job() else {
            break;
        };
        let mut worker = WorkerState::default();
        c.complete(worker.run(job, &AtomicBool::new(false)));
    }
    assert!(ready, "preserved CPU root eventually ready");
}

#[cfg(feature = "fast-preview")]
/// Proves cancellation inside the first owned initialization does not become an unavailable-device latch.
/// # Panics
/// Panics if source replacement prevents a legitimate second factory invocation in the same retry epoch.
#[test]
fn fast_cache_cancelled_initialization_allows_source_replacement_attempt() {
    let mut c = cache(64.);
    let attempts = Arc::new(AtomicUsize::new(0));
    let (entered, observed) = std::sync::mpsc::channel();
    let (resume, continue_initialization) = std::sync::mpsc::channel();
    let mut worker = WorkerState {
        unavailable_probe: Some(attempts.clone()),
        initialization_gate: Some((entered, continue_initialization)),
        ..Default::default()
    };
    let r = Region {
        x: 0,
        y: 0,
        width: 32,
        height: 32,
    };
    let k = key(&c, 1.);
    c.plan_fast((1, 2, 3), k, r).unwrap();
    let job = loop {
        let job = c.take_job().unwrap();
        if matches!(job.kind, JobKind::Fast(FastJob::Prepare)) {
            break job;
        }
        c.complete(worker.run(job, &AtomicBool::new(false)));
    };
    let thread = std::thread::spawn(move || {
        let result = worker.run(job, &AtomicBool::new(false));
        (worker, result)
    });
    observed
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    c.accept_scene((1, 4, 3), scene(64., false), 4096);
    resume.send(()).unwrap();
    let (mut worker, result) = thread.join().unwrap();
    c.complete(result);
    assert_eq!(attempts.load(Ordering::Relaxed), 1);
    assert!(c.fallback_reason().is_none());
    assert!(worker.attempted_epoch.is_none());
    let k = key(&c, 1.);
    c.plan_fast((1, 4, 3), k, r).unwrap();
    while let Some(job) = c.take_job() {
        c.complete(worker.run(job, &AtomicBool::new(false)));
    }
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert!(matches!(
        c.fallback_reason(),
        Some((_, FallbackReason::Unavailable(_)))
    ));
}
