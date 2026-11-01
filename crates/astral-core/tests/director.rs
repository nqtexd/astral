use astral_core::{director::*, *};

#[test]
fn seeded_creation_is_editable_reviewed_and_one_undo_transaction() {
    for seed in 0..8 {
        let p = Project::new("Auto", 960, 540, 24, 144);
        let result = plan(
            &p,
            AutoSpec {
                seed: Some(seed),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(p.characters.is_empty());
        assert!(
            result.quality_gate_passed,
            "seed {seed}: {:?}",
            result.review
        );
        assert!(!result.review.contacts.is_empty());
        let mut s = Session::new(p).unwrap();
        s.apply(Batch {
            project: s.project.id,
            expected_revision: 0,
            commands: result.plan.commands,
        })
        .unwrap();
        assert_eq!(s.project.characters.len(), 2);
        assert_eq!(storyboard::read(&s.project).unwrap().shots.len(), 4);
        assert!(s.project.characters.iter().all(|c| c.keys.len() > 50));
        assert!(!s.project.effects.is_empty());
        s.undo(s.project.id, 1).unwrap();
        assert!(s.project.characters.is_empty());
    }
}

#[test]
fn inspect_never_mutates_and_repairs_are_bounded() {
    let mut p = Project::new("Length drift", 960, 540, 24, 144);
    let mut c = stick_character("A".into(), Point::new(300., 350.), [255; 4]);
    c.keys[0].pose.joints[6].x += 80.;
    p.characters.push(c);
    let before = serde_json::to_value(&p).unwrap();
    let inspect = plan(
        &p,
        AutoSpec {
            mode: AutoMode::Inspect,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(inspect.plan.commands.is_empty());
    assert!(!inspect.diagnostics_before.is_empty());
    assert_eq!(serde_json::to_value(&p).unwrap(), before);
    let fixed = plan(
        &p,
        AutoSpec {
            mode: AutoMode::Improve,
            max_repair_attempts: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fixed.repair_rounds, 1);
    assert!(
        fixed
            .diagnostics_after
            .iter()
            .all(|i| i.kind != "limb_length")
    );
    assert!(
        plan(
            &p,
            AutoSpec {
                max_repair_attempts: 4,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn continuation_moves_limbs_and_preserves_the_source_keys_and_later_content() {
    let mut p = Project::new("Continue", 960, 540, 24, 100);
    let mut c = stick_character("Runner".into(), Point::new(200., 350.), [255; 4]);
    let mut second = c.keys[0].clone();
    second.frame = 10;
    second.pose.root.x = 300.;
    c.keys.push(second);
    c.keys[0].easing = Easing::Linear;
    let id = c.id;
    p.characters.push(c);
    let original = serde_json::to_value(&p.characters[0].keys[0]).unwrap();
    let result = plan(
        &p,
        AutoSpec {
            mode: AutoMode::Continue,
            start: 10,
            end: 55,
            characters: vec![id],
            ..Default::default()
        },
    )
    .unwrap();
    let mut s = Session::new(p).unwrap();
    s.apply(Batch {
        project: s.project.id,
        expected_revision: 0,
        commands: result.plan.commands,
    })
    .unwrap();
    let c = &s.project.characters[0];
    assert_eq!(serde_json::to_value(&c.keys[0]).unwrap(), original);
    assert!(c.keys.last().unwrap().pose.root.x > 300.);
    assert_ne!(c.keys[2].pose.joints, c.keys[10].pose.joints);
    assert!(
        plan(
            &s.project,
            AutoSpec {
                mode: AutoMode::Continue,
                start: 10,
                end: 55,
                characters: vec![id],
                ..Default::default()
            }
        )
        .is_err()
    );
}
