use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft, PersonalAuthoredResource,
    PersonalAuthoredResourceKind, PersonalResourceId,
};

/// Builds one finite two-point open path for personal motif validation.
fn open_path() -> AuthoredStructureDraft {
    AuthoredStructureDraft::new(
        toniator_domain::AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 0.0 },
            end: AuthoredPoint2 { x: 1.0, y: 0.0 },
        }],
    )
    .expect("fixed open path validates")
}

/// Requires canonical personal IDs and a resource-kind topology match before persistence can own data.
#[test]
fn personal_resource_validation_retains_exact_open_motif_geometry() {
    let id = PersonalResourceId::new("user-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into())
        .expect("canonical lowercase UUID validates");
    let resource = PersonalAuthoredResource::new(
        id,
        "Asymmetric motif".into(),
        PersonalAuthoredResourceKind::Motif,
        open_path(),
    )
    .expect("open geometry validates as a motif resource");
    assert_eq!(resource.draft().segments().len(), 1);
    assert!(PersonalResourceId::new("user-AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into()).is_err());
    assert!(
        PersonalAuthoredResource::new(
            PersonalResourceId::new("user-bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into()).unwrap(),
            "Wrong kind".into(),
            PersonalAuthoredResourceKind::Shape,
            open_path(),
        )
        .is_err()
    );
}
