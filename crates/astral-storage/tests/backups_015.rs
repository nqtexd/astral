use astral_core::*;
use astral_storage::*;
#[test]
fn saves_retain_five_distinct_previous_versions_and_skip_recovery_backups() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path()).unwrap();
    let mut p = Project::new("Version 0", 64, 64, 24, 24);
    workspace.save("Film.astral", &p).unwrap();
    for i in 1..8 {
        p.name = format!("Version {i}");
        p.revision = i;
        workspace.save("Film.astral", &p).unwrap();
    }
    let roots = std::fs::read_dir(dir.path().join(".astral-backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 1);
    let files = std::fs::read_dir(&roots[0])
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    assert_eq!(files.len(), 5);
    let mut names = files
        .iter()
        .map(|f| decode(&std::fs::read(f).unwrap()).unwrap().name)
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        vec![
            "Version 2",
            "Version 3",
            "Version 4",
            "Version 5",
            "Version 6"
        ]
    );
    assert_eq!(workspace.load("Film.astral").unwrap().name, "Version 7");
    workspace.save_recovery(&p).unwrap();
    p.name = "Recovered".into();
    workspace.save_recovery(&p).unwrap();
    assert_eq!(
        std::fs::read_dir(dir.path().join(".astral-backups"))
            .unwrap()
            .count(),
        1
    );
}
