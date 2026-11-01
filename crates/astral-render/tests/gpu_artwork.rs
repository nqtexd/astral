#![cfg(feature = "gpu")]
use astral_core::*;
use astral_render::*;
#[test]
fn physical_gpu_matches_cpu_interiors_and_alpha() {
    let mut gpu = match gpu::GpuRenderer::new(64, 64) {
        Ok(gpu) => gpu,
        Err(error) => {
            assert!(
                std::env::var_os("ASTRAL_REQUIRE_GPU").is_none(),
                "GPU required: {error:#}"
            );
            eprintln!("No physical GPU: {error:#}");
            return;
        }
    };
    let list = DisplayList {
        width: 64,
        height: 64,
        background: [24, 32, 48, 255],
        primitives: vec![Primitive::Path {
            contours: vec![vec![
                Point::new(8., 8.),
                Point::new(56., 8.),
                Point::new(56., 56.),
                Point::new(8., 56.),
            ]],
            color: [240, 100, 40, 128],
        }],
    };
    let pixels = gpu.render(&list, false).unwrap();
    let inside = &pixels[(32 * 64 + 32) * 4..][..4];
    assert!((inside[0] as i32 - 132).abs() < 3, "{inside:?}");
    assert_eq!(inside[3], 255);
    let transparent = DisplayList {
        width: 64,
        height: 64,
        background: [0; 4],
        primitives: vec![Primitive::Circle {
            center: Point::new(32., 32.),
            radius: 20.,
            color: [80, 160, 240, 128],
        }],
    };
    let pixels = gpu.render(&transparent, false).unwrap();
    let center = &pixels[(32 * 64 + 32) * 4..][..4];
    for (a, b) in center.iter().zip([80, 160, 240, 128]) {
        assert!((*a as i32 - b).abs() < 3, "{center:?}");
    }
    eprintln!("Physical GPU export: {}", gpu.adapter);
}
