use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureDraft,
    AuthoredStructureFieldId, AuthoredStructureId, AuthoredStructureKind, CanvasSpec, Document,
    DocumentCommand, DocumentHistory, DocumentSession, InvalidationLevel, SourceReference,
    authored_structure_field_contracts,
};

/// Builds one explicit finite authored point for topology-focused test fixtures.
fn point(x: f64, y: f64) -> AuthoredPoint2 {
    AuthoredPoint2 { x, y }
}

/// Builds one finite line segment without adding implicit closure or smoothing semantics.
fn line(start: AuthoredPoint2, end: AuthoredPoint2) -> AuthoredCurveSegment {
    AuthoredCurveSegment::Line { start, end }
}

/// Builds a default modeled document with no authored structures for command and descriptor tests.
fn document() -> Document {
    Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document is valid")
}

/// Builds a two-segment explicit open authored path for stable-ID and replacement tests.
fn open_draft() -> AuthoredStructureDraft {
    AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            line(point(1.0, 2.0), point(3.0, 2.0)),
            line(point(3.0, 2.0), point(5.0, 4.0)),
        ],
    )
    .expect("finite C0 open path is valid")
}

/// Builds a closed one-segment degenerate shape, which Stage 20C accepts without fill semantics.
fn closed_draft() -> AuthoredStructureDraft {
    AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![line(point(2.0, 2.0), point(2.0, 2.0))],
    )
    .expect("degenerate closed shape remains valid construction data")
}

/// Rebuilds a modeled document with an explicit authored-structure store for document-bound tests.
fn document_with_structures(structures: Vec<AuthoredStructure>) -> Document {
    try_document_with_structures(structures).expect("explicit authored store is valid")
}

/// Attempts to rebuild a modeled document with an explicit authored store for validation-failure tests.
///
/// # Errors
///
/// Returns the authoritative authored-store or existing modeled-document validation diagnostic
/// without constructing a partial document.
fn try_document_with_structures(
    structures: Vec<AuthoredStructure>,
) -> Result<Document, toniator_domain::ValidationError> {
    let base = document();
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        base.pattern_definitions().to_vec(),
        base.channel_model().unwrap(),
        base.channel_topology().unwrap().clone(),
        structures,
    )
}

/// Builds one valid degenerate closed structure with a caller-selected stable identity.
fn closed_structure(id: u64) -> AuthoredStructure {
    AuthoredStructure::new(
        AuthoredStructureId(id),
        AuthoredStructureKind::ClosedShape,
        closed_draft().segments().to_vec(),
    )
    .expect("finite degenerate closed structure is valid")
}

/// Builds a valid document at the 4,096 authored-structure store cap for command-bound tests.
fn store_capacity_document() -> Document {
    document_with_structures((1..=4_096).map(closed_structure).collect())
}

/// Builds a valid document at the 65,536 authored-segment aggregate cap for command-bound tests.
fn aggregate_segment_capacity_document() -> Document {
    let maximum_segments = vec![line(point(0.0, 0.0), point(0.0, 0.0)); 4_096];
    document_with_structures(
        (1..=16)
            .map(|id| {
                AuthoredStructure::new(
                    AuthoredStructureId(id),
                    AuthoredStructureKind::ClosedShape,
                    maximum_segments.clone(),
                )
                .expect("maximum finite authored structure is valid")
            })
            .collect(),
    )
}

/// Produces a history with a retained redo entry so command failure must preserve both stacks.
fn history_with_redo(document: Document) -> DocumentHistory {
    let channel_id = document.channel_topology().unwrap().channels()[0].id;
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    history
        .apply(&DocumentCommand::SetVisibility {
            channel_id,
            visible: false,
        })
        .expect("default visibility transition succeeds");
    history.undo().expect("successful transition is undoable");
    assert!(!history.can_undo());
    assert!(history.can_redo());
    history
}

/// Asserts a rejected command leaves the authoritative snapshot, revision, and history stacks unchanged.
fn assert_rejected_command_is_history_atomic(
    history: &mut DocumentHistory,
    command: DocumentCommand,
    expected_error: &str,
) {
    let before = history.document().clone();
    let revision = history.revision();
    let can_undo = history.can_undo();
    let can_redo = history.can_redo();
    let error = history.apply(&command).unwrap_err();
    assert_eq!(error.to_string(), expected_error);
    assert_eq!(history.document(), &before);
    assert_eq!(history.revision(), revision);
    assert_eq!(history.can_undo(), can_undo);
    assert_eq!(history.can_redo(), can_redo);
}

/// Validates finite explicit coordinates, declared open/closed topology, exact continuity, and degeneracy.
#[test]
fn authored_structures_validate_finite_explicit_open_and_closed_topology() {
    assert_eq!(open_draft().kind(), AuthoredStructureKind::OpenPath);
    assert_eq!(closed_draft().kind(), AuthoredStructureKind::ClosedShape);
    assert_eq!(
        AuthoredStructureDraft::new(
            AuthoredStructureKind::OpenPath,
            vec![line(point(1.0, 1.0), point(1.0, 1.0))],
        )
        .unwrap()
        .segments()
        .len(),
        1
    );
    let nonfinite = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![line(point(f64::NAN, 0.0), point(1.0, 0.0))],
    )
    .unwrap_err();
    assert_eq!(nonfinite.path(), "authored_structures.segments.coordinates");
    assert_eq!(
        nonfinite.message(),
        "authored structure coordinates must be finite"
    );
    let discontinuity = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            line(point(0.0, 0.0), point(1.0, 0.0)),
            line(point(2.0, 0.0), point(3.0, 0.0)),
        ],
    )
    .unwrap_err();
    assert_eq!(
        discontinuity.path(),
        "authored_structures.segments.continuity"
    );
    assert_eq!(
        discontinuity.message(),
        "adjacent authored segment endpoints must be exactly equal"
    );
    let open_coincident = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            line(point(0.0, 0.0), point(1.0, 0.0)),
            line(point(1.0, 0.0), point(0.0, 0.0)),
        ],
    )
    .unwrap();
    assert_eq!(open_coincident.kind(), AuthoredStructureKind::OpenPath);
    let closure = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![line(point(0.0, 0.0), point(1.0, 0.0))],
    )
    .unwrap_err();
    assert_eq!(closure.path(), "authored_structures.closure");
    assert_eq!(
        closure.message(),
        "closed authored shapes require the final endpoint to equal the initial start"
    );
    let empty = AuthoredStructureDraft::new(AuthoredStructureKind::OpenPath, vec![]).unwrap_err();
    assert_eq!(empty.path(), "authored_structures.segments.empty");
    assert_eq!(
        empty.message(),
        "authored structures require at least one segment"
    );
    let too_many_segments = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![line(point(0.0, 0.0), point(0.0, 0.0)); 4_097],
    )
    .unwrap_err();
    assert_eq!(
        too_many_segments.path(),
        "authored_structures.segments.limit"
    );
    assert_eq!(
        too_many_segments.message(),
        "authored structures support at most 4096 segments"
    );
    let zero_id = AuthoredStructure::new(
        AuthoredStructureId(0),
        AuthoredStructureKind::OpenPath,
        open_draft().segments().to_vec(),
    )
    .unwrap_err();
    assert_eq!(zero_id.path(), "authored_structures.id");
    assert_eq!(zero_id.message(), "authored structure IDs must be nonzero");
    let duplicate_id = Document::with_source_topology_and_authored_structures(
        document().id(),
        document().canvas().clone(),
        document().source().clone(),
        document().pattern_definitions().to_vec(),
        document().channel_model().unwrap(),
        document().channel_topology().unwrap().clone(),
        vec![closed_structure(1), closed_structure(1)],
    )
    .unwrap_err();
    assert_eq!(duplicate_id.path(), "authored_structures.id");
    assert_eq!(
        duplicate_id.message(),
        "authored structure IDs must be unique within a document"
    );
    let store_limit = Document::with_source_topology_and_authored_structures(
        document().id(),
        document().canvas().clone(),
        document().source().clone(),
        document().pattern_definitions().to_vec(),
        document().channel_model().unwrap(),
        document().channel_topology().unwrap().clone(),
        (1..=4_097).map(closed_structure).collect(),
    )
    .unwrap_err();
    assert_eq!(store_limit.path(), "authored_structures.limit");
    assert_eq!(
        store_limit.message(),
        "documents support at most 4096 authored structures"
    );
    let maximum_segments = vec![line(point(0.0, 0.0), point(0.0, 0.0)); 4_096];
    let total_limit = try_document_with_structures(
        (1..=17)
            .map(|id| {
                AuthoredStructure::new(
                    AuthoredStructureId(id),
                    AuthoredStructureKind::ClosedShape,
                    maximum_segments.clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap_err();
    assert_eq!(total_limit.path(), "authored_structures.segment_limit");
    assert_eq!(
        total_limit.message(),
        "documents support at most 65536 authored segments"
    );
}

/// Proves add/duplicate/replace/remove commands allocate IDs and preserve atomic history transitions.
#[test]
fn authored_structure_commands_allocate_duplicate_replace_remove_and_history_atomically() {
    let initial = document();
    let mut history = DocumentHistory::new(DocumentSession::new(initial.clone()).unwrap());
    let add = history
        .apply(&DocumentCommand::AddAuthoredStructure {
            draft: open_draft(),
        })
        .unwrap();
    assert_eq!(
        add.created_authored_structure_id,
        Some(AuthoredStructureId(1))
    );
    assert_eq!(add.invalidation, InvalidationLevel::Family);
    assert!(add.affected_channels.is_empty());
    let original = history
        .document()
        .authored_structure(AuthoredStructureId(1))
        .unwrap()
        .clone();
    let duplicate = history
        .apply(&DocumentCommand::DuplicateAuthoredStructure {
            structure_id: original.id(),
        })
        .unwrap();
    assert_eq!(
        duplicate.created_authored_structure_id,
        Some(AuthoredStructureId(2))
    );
    assert_eq!(history.document().authored_structures().len(), 2);
    let replacement = closed_draft();
    let replace = history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: original.clone(),
            replacement: replacement.clone(),
        })
        .unwrap();
    assert_eq!(replace.invalidation, InvalidationLevel::Family);
    let replaced = history
        .document()
        .authored_structure(AuthoredStructureId(1))
        .unwrap();
    assert_eq!(replaced.id(), AuthoredStructureId(1));
    assert_eq!(replaced.kind(), AuthoredStructureKind::ClosedShape);
    let closed_replacement = AuthoredStructureDraft::new(
        AuthoredStructureKind::ClosedShape,
        vec![line(point(8.0, 8.0), point(8.0, 8.0))],
    )
    .unwrap();
    let closed_result = history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: replaced.clone(),
            replacement: closed_replacement,
        })
        .unwrap();
    assert_eq!(closed_result.invalidation, InvalidationLevel::Realization);
    let before_failure = history.document().clone();
    let before_revision = history.revision();
    let stale = history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: original,
            replacement,
        })
        .unwrap_err();
    assert_eq!(
        stale.to_string(),
        "authored_structures.edit.stale: authored structure replacement base is stale"
    );
    assert_eq!(history.document(), &before_failure);
    assert_eq!(history.revision(), before_revision);
    let noop_base = history
        .document()
        .authored_structure(AuthoredStructureId(1))
        .unwrap()
        .clone();
    let noop = history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: noop_base.clone(),
            replacement: AuthoredStructureDraft::new(
                noop_base.kind(),
                noop_base.segments().to_vec(),
            )
            .unwrap(),
        })
        .unwrap_err();
    assert_eq!(
        noop.to_string(),
        "authored_structures.edit.noop: authored structure replacement is a semantic no-op"
    );
    assert_eq!(history.document(), &before_failure);
    assert_eq!(history.revision(), before_revision);
    let missing_duplicate = history
        .apply(&DocumentCommand::DuplicateAuthoredStructure {
            structure_id: AuthoredStructureId(999),
        })
        .unwrap_err();
    assert_eq!(
        missing_duplicate.to_string(),
        "authored_structures.reference: authored structure to duplicate does not exist"
    );
    assert_eq!(history.document(), &before_failure);
    assert_eq!(history.revision(), before_revision);
    let missing_remove = history
        .apply(&DocumentCommand::RemoveUnreferencedAuthoredStructure {
            structure_id: AuthoredStructureId(999),
        })
        .unwrap_err();
    assert_eq!(
        missing_remove.to_string(),
        "authored_structures.remove.missing: authored structure to remove does not exist"
    );
    assert_eq!(history.document(), &before_failure);
    assert_eq!(history.revision(), before_revision);
    let removed = history
        .apply(&DocumentCommand::RemoveUnreferencedAuthoredStructure {
            structure_id: AuthoredStructureId(2),
        })
        .unwrap();
    assert_eq!(removed.invalidation, InvalidationLevel::Family);
    history.undo().unwrap();
    assert_eq!(history.document(), &before_failure);
    history.redo().unwrap();
    assert_eq!(history.document().authored_structures().len(), 1);

    let exhausted = document_with_structures(vec![
        AuthoredStructure::new(
            AuthoredStructureId(u64::MAX),
            AuthoredStructureKind::ClosedShape,
            closed_draft().segments().to_vec(),
        )
        .unwrap(),
    ]);
    let mut exhausted_history =
        DocumentHistory::new(DocumentSession::new(exhausted.clone()).unwrap());
    let exhausted_error = exhausted_history
        .apply(&DocumentCommand::DuplicateAuthoredStructure {
            structure_id: AuthoredStructureId(u64::MAX),
        })
        .unwrap_err();
    assert_eq!(
        exhausted_error.to_string(),
        "authored_structures.id: document ID space is exhausted"
    );
    assert_eq!(exhausted_history.document(), &exhausted);
    assert_eq!(exhausted_history.revision().0, 0);

    let mut store_bound_history = history_with_redo(store_capacity_document());
    assert_rejected_command_is_history_atomic(
        &mut store_bound_history,
        DocumentCommand::AddAuthoredStructure {
            draft: closed_draft(),
        },
        "authored_structures.limit: documents support at most 4096 authored structures",
    );
    let mut aggregate_bound_history = history_with_redo(aggregate_segment_capacity_document());
    assert_rejected_command_is_history_atomic(
        &mut aggregate_bound_history,
        DocumentCommand::AddAuthoredStructure {
            draft: closed_draft(),
        },
        "authored_structures.segment_limit: documents support at most 65536 authored segments",
    );
}

/// Proves descriptors remain value-free and reflect command invalidation at the authored-structure boundary.
#[test]
fn authored_structure_descriptors_match_commands_validation_and_invalidation() {
    let structure = AuthoredStructure::new(
        AuthoredStructureId(7),
        AuthoredStructureKind::ClosedShape,
        closed_draft().segments().to_vec(),
    )
    .unwrap();
    let document = Document::with_source_topology_and_authored_structures(
        document().id(),
        document().canvas().clone(),
        document().source().clone(),
        document().pattern_definitions().to_vec(),
        document().channel_model().unwrap(),
        document().channel_topology().unwrap().clone(),
        vec![structure],
    )
    .unwrap();
    let descriptors = document.authored_structure_descriptors();
    assert_eq!(descriptors.len(), 2);
    let kind = descriptors
        .iter()
        .find(|descriptor| descriptor.field == AuthoredStructureFieldId::Kind)
        .unwrap();
    assert!(kind.shared_edit);
    assert_eq!(kind.invalidation, InvalidationLevel::Family);
    let segments = descriptors
        .iter()
        .find(|descriptor| descriptor.field == AuthoredStructureFieldId::Segments)
        .unwrap();
    assert_eq!(segments.maximum_segments, Some(4_096));
    assert_eq!(segments.invalidation, InvalidationLevel::Realization);
    let contract_segments = authored_structure_field_contracts()
        .iter()
        .find(|contract| contract.field == AuthoredStructureFieldId::Segments)
        .unwrap();
    assert_eq!(
        contract_segments.invalidation,
        InvalidationLevel::Family,
        "the value-free contract is conservative because an open-path target requires Family"
    );
    let open_document = document_with_structures(vec![
        AuthoredStructure::new(
            AuthoredStructureId(8),
            AuthoredStructureKind::OpenPath,
            open_draft().segments().to_vec(),
        )
        .unwrap(),
    ]);
    let open_segments = open_document
        .authored_structure_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.field == AuthoredStructureFieldId::Segments)
        .unwrap();
    assert_eq!(open_segments.invalidation, contract_segments.invalidation);
}

/// Proves stable IDs resolve without labels, positions, or array indices becoming aliases.
#[test]
fn authored_structure_ids_resolve_stably_without_name_or_position_aliases() {
    let first = AuthoredStructure::new(
        AuthoredStructureId(9),
        AuthoredStructureKind::OpenPath,
        open_draft().segments().to_vec(),
    )
    .unwrap();
    let second = AuthoredStructure::new(
        AuthoredStructureId(3),
        AuthoredStructureKind::ClosedShape,
        closed_draft().segments().to_vec(),
    )
    .unwrap();
    let base = document();
    let document = Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        base.pattern_definitions().to_vec(),
        base.channel_model().unwrap(),
        base.channel_topology().unwrap().clone(),
        vec![first, second],
    )
    .unwrap();
    assert_eq!(
        document.authored_structures()[0].id(),
        AuthoredStructureId(9)
    );
    assert_eq!(
        document.authored_structures()[1].id(),
        AuthoredStructureId(3)
    );
    assert_eq!(
        document
            .authored_structure(AuthoredStructureId(3))
            .unwrap()
            .kind(),
        AuthoredStructureKind::ClosedShape
    );
    assert!(
        document
            .authored_structure(AuthoredStructureId(1))
            .is_none()
    );
}
