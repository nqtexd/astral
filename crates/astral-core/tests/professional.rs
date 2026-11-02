use astral_core::*;
#[test]
fn archived_source_is_shared_across_undo_and_upgrades_old_metadata() {
    let mut p = Project::new("Archive", 64, 64, 24, 24);
    p.metadata.insert("legacy".into(),serde_json::json!({"filename":"source.styk","source_base64":"A".repeat(4*1024*1024),"document":{"unknown_key":"preserved"}}));
    let mut s = Session::new(p).unwrap();
    let original = s.project.original_source.clone().unwrap();
    assert_eq!(s.project.schema, 9);
    for i in 0..25 {
        s.apply(Batch {
            project: s.project.id,
            expected_revision: s.project.revision,
            commands: vec![Command::SetProject {
                name: Some(format!("Edit {i}")),
                duration: None,
                background: None,
            }],
        })
        .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &original,
            s.project.original_source.as_ref().unwrap()
        ));
    }
    for _ in 0..25 {
        s.undo(s.project.id, s.project.revision).unwrap();
    }
    assert_eq!(s.project.name, "Archive");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&original.canonical_json).unwrap()["unknown_key"],
        "preserved"
    );
}
#[test]
fn bone_corrective_drives_the_same_uv_used_by_rendering() {
    let mut p = Project::new("Corrective", 64, 64, 24, 24);
    let mut c = stick_character("Rig".into(), Point::default(), [0, 0, 0, 255]);
    c.joints = vec![
        Joint {
            name: "root".into(),
            parent: None,
            position: Point::default(),
        },
        Joint {
            name: "arm".into(),
            parent: Some(0),
            position: Point::new(10., 0.),
        },
    ];
    c.keys = vec![PoseKey {
        frame: 0,
        pose: Pose {
            root: Point::default(),
            joints: vec![Point::default(), Point::new(0., 10.)],
        },
        easing: Easing::Linear,
    }];
    let character = c.id;
    p.characters.push(c);
    let raster = RasterLayer {
        id: Id::new_v4(),
        name: "Skin".into(),
        start: 0,
        end: 23,
        sources: vec![Id::new_v4()],
        source_fps: 24,
        looping: false,
        size: Point::new(100., 100.),
        keys: vec![RasterKey {
            frame: 0,
            position: Point::new(50., 50.),
            scale: Point::new(1., 1.),
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
        cels: vec![],
        mesh: Some(DeformMesh {
            columns: 8,
            rows: 8,
            mode: DeformMode::Similarity,
            curve: vec![],
            pins: vec![MeshPin {
                id: Id::new_v4(),
                name: "Elbow correction".into(),
                rest: Point::new(0.5, 0.5),
                keys: vec![PinKey {
                    frame: 0,
                    offset: Point::default(),
                }],
                driver: Some(PinDriver {
                    character,
                    joint: 1,
                    rest_angle: 0.,
                    angle_range: std::f32::consts::FRAC_PI_2,
                    offset: Point::new(0.2, 0.),
                }),
            }],
        }),
        layer: GraphicLayer::Foreground,
        visible: true,
    };
    let resolved = raster.resolve_correctives(&evaluate(&p, 0.));
    assert!(
        resolved
            .world_point(Point::new(0.5, 0.5), 0.)
            .distance(Point::new(70., 50.))
            < 0.001
    );
    assert_eq!(
        raster.mesh.unwrap().pins[0].keys[0].offset,
        Point::default()
    );
}
