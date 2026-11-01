use astral_core::*;
use astral_render::*;

#[test]
fn custom_contours_render_holes_animate_and_reject_invalid_geometry() {
    let mut p = Project::new("Artwork", 100, 100, 60, 120);
    p.schema = 7;
    p.background = [255; 4];
    let outer = vec![
        Point::new(-30., -30.),
        Point::new(30., -30.),
        Point::new(30., 30.),
        Point::new(-30., 30.),
    ];
    let inner = vec![
        Point::new(-10., -10.),
        Point::new(-10., 10.),
        Point::new(10., 10.),
        Point::new(10., -10.),
    ];
    let g = Graphic {
        id: Id::new_v4(),
        name: "Desk frame".into(),
        kind: GraphicKind::Rectangle,
        contours: vec![outer, inner],
        start: 0,
        end: 119,
        size: Point::new(60., 60.),
        color: [15, 30, 45, 255],
        layer: GraphicLayer::Background,
        text: String::new(),
        draw_on: false,
        keys: vec![GraphicKey {
            frame: 0,
            position: Point::new(50., 50.),
            scale: 1.,
            rotation: 0.,
            opacity: 1.,
            easing: Easing::Linear,
        }],
    };
    p.graphics.push(g);
    p.validate().unwrap();
    let image = rasterize(&display_list(&p, 0.), 100, 100).unwrap();
    assert_eq!(image.pixel(50, 50).unwrap().red(), 255);
    assert!(image.pixel(25, 25).unwrap().red() < 30);
    let original = png(&p, 0., 100, 100).unwrap();
    p.graphics[0].keys[0].rotation = 0.3;
    assert_ne!(original, png(&p, 0., 100, 100).unwrap());
    p.schema = 6;
    assert!(p.validate().is_err());
    p.schema = 7;
    p.graphics[0].contours[0][0].x = f32::NAN;
    assert!(p.validate().is_err());
}

#[test]
fn old_graphics_default_to_their_original_builtin_outline() {
    let p = motion_lab_demo().unwrap();
    let before = png(&p, 75., 320, 180).unwrap();
    let restored: Project = serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
    assert!(restored.graphics.iter().all(|g| g.contours.is_empty()));
    assert_eq!(before, png(&restored, 75., 320, 180).unwrap());
}
