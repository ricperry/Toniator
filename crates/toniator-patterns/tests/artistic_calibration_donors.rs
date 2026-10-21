//! Bundled measurements retain exact current producer identity without runtime fitting.
use toniator_domain::*;
use toniator_patterns::PresetRegistry;

#[path = "../src/builtin_calibrations.rs"]
mod builtin_calibrations;

/// Creates an ordinary source-free receiver; artwork and dimensions do not define fit identity.
/// # Panics
/// Panics if the fixed valid canvas cannot construct its authoritative document.
fn receiver(width: f64, height: f64) -> Document {
    Document::new_default_document(CanvasSpec { width, height }, SourceReference::Unassigned)
        .unwrap()
}

/// Covers all current catalog resources on both project-wide artwork dimensions, ALL and named.
/// A default replacement attaches its measured artifact to active and hidden ALL bindings, so
/// preparation has no missing exact target; repeated replacement and switching reuse the bundle.
/// # Panics
/// Panics if a catalog record lacks a current exact companion or domain publication loses it.
#[test]
fn all_bundled_recipes_publish_complete_current_companions_without_missing_targets() {
    let registry = PresetRegistry::bundled();
    assert_eq!(registry.entries().len(), 16);
    for (width, height) in [(1024.0, 1024.0), (900.0, 620.0)] {
        for entry in registry.entries() {
            let saved = entry
                .saved_calibration
                .as_ref()
                .unwrap_or_else(|| panic!("{}", entry.metadata.id));
            let mut history =
                DocumentHistory::new(DocumentSession::new(receiver(width, height)).unwrap());
            for _ in 0..2 {
                registry
                    .apply_to_document_base(&mut history, &entry.metadata.id)
                    .unwrap();
                history
                    .document()
                    .require_saved_calibration_authority()
                    .unwrap();
                assert_eq!(
                    history.document().baseline_saved_calibration().unwrap(),
                    Some(saved)
                );
                for channel in history.document().channel_topology().unwrap().channels() {
                    assert_eq!(
                        history
                            .document()
                            .saved_calibration_for(channel.id)
                            .unwrap(),
                        Some(saved)
                    );
                }
            }
            let alternate = if entry.metadata.id == "straight-grid-circles" {
                "one-guide-lines"
            } else {
                "straight-grid-circles"
            };
            registry
                .apply_to_document_base(&mut history, alternate)
                .unwrap();
            registry
                .apply_to_document_base(&mut history, &entry.metadata.id)
                .unwrap();
            history
                .document()
                .require_saved_calibration_authority()
                .unwrap();
            assert_eq!(
                history.document().baseline_saved_calibration().unwrap(),
                Some(saved)
            );
            let mut named =
                DocumentHistory::new(DocumentSession::new(receiver(width, height)).unwrap());
            registry
                .apply_to_selected(&mut named, ChannelId(1), &entry.metadata.id)
                .unwrap();
            assert_eq!(
                named
                    .document()
                    .saved_calibration_for(ChannelId(1))
                    .unwrap(),
                Some(saved)
            );
        }
    }
}

/// Rejects changed structural response bindings instead of relabeling the immutable observations.
/// # Panics
/// Panics if a custom connected bias acquires the catalog's former measurement.
#[test]
fn changed_structural_recipe_has_no_bundled_measurement() {
    let registry = PresetRegistry::bundled();
    let entry = registry.find("one-guide-lines").unwrap();
    let mut changed = entry.recipe.clone();
    let PatternGeometryResponse::Connected(response) = &mut changed.output_settings[0].response
    else {
        panic!("connected fixture");
    };
    response.bias = 0.25;
    assert!(
        builtin_calibrations::calibration_for_recipe(&entry.metadata.id, &changed)
            .unwrap()
            .is_none()
    );
    assert!(
        builtin_calibrations::calibration_for_recipe("unknown-resource", &entry.recipe)
            .unwrap()
            .is_none()
    );
}

/// Refuses obsolete or unknown stored producer profiles even when complete binding bytes match.
/// The mean branch also refuses strict-profile observations paired with a variation payload.
/// # Panics
/// Panics if a changed profile is accepted, or if the genuine matching fixture stops validating.
#[test]
fn frozen_binding_and_literal_profile_both_gate_observations() {
    let registry = PresetRegistry::bundled();
    for id in ["one-guide-lines", "clustered-dispersion-random-links"] {
        let saved = registry
            .find(id)
            .unwrap()
            .saved_calibration
            .as_ref()
            .unwrap();
        let frozen = format!("{:?}", saved.binding());
        let variation = saved.variation().map(|v| {
            (
                v.raw_absolute(),
                v.raw_fraction_of_span(),
                v.normalized_tone_absolute(),
                v.low_tone_relative(),
                v.uniform_tone_absolute(),
                v.uniform_low_tone_relative(),
            )
        });
        let uniform = saved
            .uniform()
            .map(|v| (v.radius_per_nominal_spacing(), v.inverse()));
        assert_eq!(
            builtin_calibrations::restore_measurement(
                saved.binding().clone(),
                (&frozen, saved.profile(), saved.raw(), variation, uniform)
            )
            .unwrap(),
            Some(saved.clone())
        );
        for profile in [CALIBRATION_PROFILE, "unknown-future-producer"] {
            assert!(
                builtin_calibrations::restore_measurement(
                    saved.binding().clone(),
                    (&frozen, profile, saved.raw(), variation, uniform)
                )
                .unwrap()
                .is_none()
            );
        }
        assert!(
            builtin_calibrations::restore_measurement(
                saved.binding().clone(),
                (
                    "different complete binding",
                    saved.profile(),
                    saved.raw(),
                    variation,
                    uniform
                )
            )
            .unwrap()
            .is_none()
        );
        if variation.is_some() {
            let strict = saved
                .binding()
                .profile_for_reference(CALIBRATION_PROFILE_SEEDED);
            assert!(
                builtin_calibrations::restore_measurement(
                    saved.binding().clone(),
                    (&frozen, &strict, saved.raw(), variation, uniform)
                )
                .unwrap()
                .is_none()
            );
        }
    }
}
