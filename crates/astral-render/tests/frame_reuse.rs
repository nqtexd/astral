use astral_core::*;
use astral_render::*;
use std::sync::Arc;
#[test]
fn exact_holds_reuse_pixels_but_colour_framing_and_size_changes_do_not() {
    let mut cache = FrameCache::default();
    let list = DisplayList {
        width: 10,
        height: 10,
        background: [255; 4],
        primitives: vec![Primitive::Circle {
            center: Point::new(5., 5.),
            radius: 3.,
            color: [240, 20, 40, 255],
        }],
    };
    let render = |cache: &mut FrameCache, list: DisplayList, w, h, fill| {
        cache
            .render(list, w, h, fill, |list| rgba_list_framed(list, w, h, fill))
            .unwrap()
    };
    let first = render(&mut cache, list.clone(), 40, 20, false);
    let held = render(&mut cache, list.clone(), 40, 20, false);
    assert!(Arc::ptr_eq(&first, &held));
    assert_eq!(cache.hits, 1);
    let fill = render(&mut cache, list.clone(), 40, 20, true);
    assert_ne!(fill, first);
    assert_eq!(cache.hits, 1);
    let large = render(&mut cache, list.clone(), 80, 40, true);
    assert_eq!(large.len(), 80 * 40 * 4);
    assert_eq!(cache.hits, 1);
    let mut recoloured = list;
    recoloured.primitives[0] = Primitive::Circle {
        center: Point::new(5., 5.),
        radius: 3.,
        color: [20, 40, 240, 255],
    };
    let blue = render(&mut cache, recoloured, 40, 20, false);
    assert_ne!(blue, first);
    assert_eq!(cache.hits, 1);
}
#[test]
fn a_bitmap_with_replaced_pixels_cannot_reuse_an_old_render() {
    let id = Id::new_v4();
    let make = |pixels: Vec<u8>| DisplayList {
        width: 10,
        height: 10,
        background: [0, 0, 0, 255],
        primitives: vec![Primitive::Image {
            bitmap: Arc::new(DecodedBitmap {
                id,
                width: 1,
                height: 1,
                pixels,
            }),
            vertices: vec![
                TextureVertex {
                    position: Point::new(0., 0.),
                    uv: Point::new(0., 0.),
                },
                TextureVertex {
                    position: Point::new(10., 0.),
                    uv: Point::new(1., 0.),
                },
                TextureVertex {
                    position: Point::new(10., 10.),
                    uv: Point::new(1., 1.),
                },
                TextureVertex {
                    position: Point::new(0., 10.),
                    uv: Point::new(0., 1.),
                },
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            opacity: 1.,
        }],
    };
    let mut cache = FrameCache::default();
    let red = cache
        .render(make(vec![255, 0, 0, 255]), 20, 20, false, |list| {
            rgba_list_framed(list, 20, 20, false)
        })
        .unwrap();
    let blue = cache
        .render(make(vec![0, 0, 255, 255]), 20, 20, false, |list| {
            rgba_list_framed(list, 20, 20, false)
        })
        .unwrap();
    assert_ne!(red, blue);
    assert_eq!(cache.hits, 0);
}
