// src/primitives_processing.rs
use crate::datatypes::{Attributes, ElementBuffer, Plane, Scene, Vertex};
use crate::internal_datatypes::ViewportTransform;
use crate::viewport_process::apply_viewport;
use crate::buffer_writing::discretize_triangle;

/// true, если треугольник целиком за пределами клип-области.
fn triangle_outside_clip<A: Attributes>(
    vertices: &[Vertex<A>; 3],
    clip_x: f32, clip_y: f32, clip_w: f32, clip_h: f32,
) -> bool {
    let v0 = &vertices[0];
    let v1 = &vertices[1];
    let v2 = &vertices[2];
    let min_x = v0.x.min(v1.x).min(v2.x);
    let max_x = v0.x.max(v1.x).max(v2.x);
    let min_y = v0.y.min(v1.y).min(v2.y);
    let max_y = v0.y.max(v1.y).max(v2.y);
    max_x < clip_x
        || min_x >= clip_x + clip_w
        || max_y < clip_y
        || min_y >= clip_y + clip_h
}

pub(crate) fn process_triangles<A: Attributes>(
    buffer: &mut ElementBuffer<A>,
    plane: &Plane,
    scene: &Scene<A>,
    layer: u32,
    xform: &ViewportTransform,
    clip_x: u32, clip_y: u32, clip_w: u32, clip_h: u32,
) {
    let clip_xf = clip_x as f32;
    let clip_yf = clip_y as f32;
    let clip_wf = clip_w as f32;
    let clip_hf = clip_h as f32;

    for &tri_idx in &plane.triangles {
        debug_assert!(
            tri_idx < scene.triangles.len(),
            "Triangle index {} out of bounds (count: {})",
            tri_idx, scene.triangles.len()
        );
        let tri = &scene.triangles[tri_idx];

        debug_assert!(
            tri.vertices.iter().all(|&idx| idx < scene.vertices.len()),
            "Triangle {} references out-of-bounds vertices",
            tri.id
        );

        let mut transformed = [
            scene.vertices[tri.vertices[0]],
            scene.vertices[tri.vertices[1]],
            scene.vertices[tri.vertices[2]],
        ];

        for v in &mut transformed {
            apply_viewport(v, xform);
        }

        if triangle_outside_clip(&transformed, clip_xf, clip_yf, clip_wf, clip_hf) {
            continue;
        }

        discretize_triangle(
            buffer,
            &transformed,
            tri.id,
            layer,
            clip_x, clip_y, clip_w, clip_h,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::Triangle;

    fn vert<A: Attributes>(x: f32, y: f32, depth: f32, attr: A) -> Vertex<A> {
        Vertex::with_depth(x, y, depth, attr)
    }

    fn identity_transform() -> ViewportTransform {
        ViewportTransform {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            has_rotation: false,
            cos: 1.0,
            sin: 0.0,
            cx: 0.0,
            cy: 0.0,
            y_aspect: 1.0,
        }
    }

    fn scale_offset_transform(sx: f32, sy: f32, ox: f32, oy: f32) -> ViewportTransform {
        ViewportTransform {
            scale_x: sx,
            scale_y: sy,
            offset_x: ox,
            offset_y: oy,
            has_rotation: false,
            cos: 1.0,
            sin: 0.0,
            cx: 0.0,
            cy: 0.0,
            y_aspect: 1.0,
        }
    }

    // -----------------------------------------------------------------
    // triangle_outside_clip — «полностью за пределами»
    // -----------------------------------------------------------------

    #[test]
    fn fully_left_is_outside() {
        let verts = [
            vert(-10.0, 10.0, 0.0, ()),
            vert(-5.0, 10.0, 0.0, ()),
            vert(-10.0, 20.0, 0.0, ()),
        ];
        assert!(triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn fully_right_is_outside() {
        let verts = [
            vert(110.0, 10.0, 0.0, ()),
            vert(115.0, 10.0, 0.0, ()),
            vert(110.0, 20.0, 0.0, ()),
        ];
        assert!(triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn fully_above_is_outside() {
        let verts = [
            vert(10.0, -10.0, 0.0, ()),
            vert(20.0, -10.0, 0.0, ()),
            vert(10.0, -5.0, 0.0, ()),
        ];
        assert!(triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn fully_below_is_outside() {
        let verts = [
            vert(10.0, 110.0, 0.0, ()),
            vert(20.0, 110.0, 0.0, ()),
            vert(10.0, 115.0, 0.0, ()),
        ];
        assert!(triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn fully_inside_is_not_outside() {
        let verts = [
            vert(10.0, 10.0, 0.0, ()),
            vert(90.0, 10.0, 0.0, ()),
            vert(10.0, 90.0, 0.0, ()),
        ];
        assert!(!triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    // -----------------------------------------------------------------
    // triangle_outside_clip — границы (асимметрия `<` слева, `>=` справа)
    // -----------------------------------------------------------------

    #[test]
    fn touching_left_edge_is_not_outside() {
        // max_x == clip_x. Проверка `max_x < clip_x` строгая — не отсекаем.
        let verts = [
            vert(-5.0, 10.0, 0.0, ()),
            vert(0.0, 10.0, 0.0, ()),
            vert(-5.0, 20.0, 0.0, ()),
        ];
        assert!(!triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn just_left_of_clip_is_outside() {
        // max_x == -0.001 < clip_x = 0.0 — строгое `<` сработало.
        let verts = [
            vert(-5.0, 10.0, 0.0, ()),
            vert(-0.001, 10.0, 0.0, ()),
            vert(-5.0, 20.0, 0.0, ()),
        ];
        assert!(triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn touching_right_edge_is_outside() {
        // min_x == clip_x + clip_w. Проверка `min_x >= clip_x + clip_w` —
        // касание правого края считается «за пределами». Это контракт.
        let verts = [
            vert(100.0, 10.0, 0.0, ()),
            vert(105.0, 10.0, 0.0, ()),
            vert(100.0, 20.0, 0.0, ()),
        ];
        assert!(triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn just_inside_right_edge_is_not_outside() {
        // min_x == 99.999 < 100.0 — не отсекаем.
        let verts = [
            vert(99.999, 10.0, 0.0, ()),
            vert(105.0, 10.0, 0.0, ()),
            vert(99.999, 20.0, 0.0, ()),
        ];
        assert!(!triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn touching_top_edge_is_not_outside() {
        // max_y == clip_y. Проверка `max_y < clip_y` строгая.
        let verts = [
            vert(10.0, -5.0, 0.0, ()),
            vert(20.0, -5.0, 0.0, ()),
            vert(10.0, 0.0, 0.0, ()),
        ];
        assert!(!triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn touching_bottom_edge_is_outside() {
        // min_y == clip_y + clip_h. Проверка `min_y >= clip_y + clip_h`.
        let verts = [
            vert(10.0, 100.0, 0.0, ()),
            vert(20.0, 100.0, 0.0, ()),
            vert(10.0, 105.0, 0.0, ()),
        ];
        assert!(triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn huge_triangle_is_not_outside() {
        let verts = [
            vert(-1000.0, -1000.0, 0.0, ()),
            vert(1000.0, -1000.0, 0.0, ()),
            vert(-1000.0, 1000.0, 0.0, ()),
        ];
        assert!(!triangle_outside_clip(&verts, 0.0, 0.0, 100.0, 100.0));
    }

    // -----------------------------------------------------------------
    // process_triangles
    // -----------------------------------------------------------------

    #[test]
    fn empty_plane_writes_nothing() {
        let scene = Scene::<()>::new();
        let plane = Plane { triangles: vec![] };
        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1]);
        process_triangles(
            &mut buf, &plane, &scene, 0,
            &identity_transform(), 0, 0, 4, 4,
        );
        assert_eq!(buf.cells().count(), 0);
    }

    #[test]
    fn single_inside_triangle_writes_cells_with_its_id() {
        let mut scene = Scene::<()>::new();
        let v0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, ()));
        let v1 = scene.add_vertex(Vertex::with_depth(2.0, 0.0, 0.0, ()));
        let v2 = scene.add_vertex(Vertex::with_depth(0.0, 2.0, 0.0, ()));
        let tri = scene.add_triangle(Triangle { id: 42, vertices: [v0, v1, v2] });
        let plane = Plane { triangles: vec![tri] };

        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1]);
        process_triangles(
            &mut buf, &plane, &scene, 0,
            &identity_transform(), 0, 0, 4, 4,
        );

        let cells: Vec<_> = buf.cells().collect();
        assert!(!cells.is_empty(), "expected some cells to be written");
        for (_, _, _, stack) in cells {
            assert_eq!(stack[0].object_id, 42);
        }
    }

    #[test]
    fn outside_triangle_is_skipped() {
        let mut scene = Scene::<()>::new();
        let v0 = scene.add_vertex(Vertex::with_depth(-100.0, -100.0, 0.0, ()));
        let v1 = scene.add_vertex(Vertex::with_depth(-98.0, -100.0, 0.0, ()));
        let v2 = scene.add_vertex(Vertex::with_depth(-100.0, -98.0, 0.0, ()));
        let tri = scene.add_triangle(Triangle { id: 1, vertices: [v0, v1, v2] });
        let plane = Plane { triangles: vec![tri] };

        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1]);
        process_triangles(
            &mut buf, &plane, &scene, 0,
            &identity_transform(), 0, 0, 4, 4,
        );
        assert_eq!(buf.cells().count(), 0);
    }

    #[test]
    fn transform_is_applied_before_clipping() {
        // В координатах сцены треугольник в [10..12] — далеко за буфером.
        // С offset_x = -10 попадает в [0..2] и должен быть записан.
        let mut scene = Scene::<()>::new();
        let v0 = scene.add_vertex(Vertex::with_depth(10.0, 0.0, 0.0, ()));
        let v1 = scene.add_vertex(Vertex::with_depth(12.0, 0.0, 0.0, ()));
        let v2 = scene.add_vertex(Vertex::with_depth(10.0, 2.0, 0.0, ()));
        let tri = scene.add_triangle(Triangle { id: 7, vertices: [v0, v1, v2] });
        let plane = Plane { triangles: vec![tri] };

        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1]);
        let xform = scale_offset_transform(1.0, 1.0, -10.0, 0.0);
        process_triangles(
            &mut buf, &plane, &scene, 0,
            &xform, 0, 0, 4, 4,
        );

        assert!(
            buf.cells().count() > 0,
            "triangle should be visible after transform, but nothing was written"
        );
    }

    #[test]
    fn transform_can_push_triangle_out_of_clip() {
        // Тот же треугольник у начала координат, но offset_x = 100
        // выталкивает его за клип, и он должен быть пропущен.
        let mut scene = Scene::<()>::new();
        let v0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, ()));
        let v1 = scene.add_vertex(Vertex::with_depth(2.0, 0.0, 0.0, ()));
        let v2 = scene.add_vertex(Vertex::with_depth(0.0, 2.0, 0.0, ()));
        let tri = scene.add_triangle(Triangle { id: 1, vertices: [v0, v1, v2] });
        let plane = Plane { triangles: vec![tri] };

        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1]);
        let xform = scale_offset_transform(1.0, 1.0, 100.0, 0.0);
        process_triangles(
            &mut buf, &plane, &scene, 0,
            &xform, 0, 0, 4, 4,
        );
        assert_eq!(buf.cells().count(), 0);
    }

    #[test]
    fn only_in_view_triangles_are_written() {
        let mut scene = Scene::<()>::new();
        // Triangle 10: у начала координат, попадёт в буфер.
        let a0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, ()));
        let a1 = scene.add_vertex(Vertex::with_depth(2.0, 0.0, 0.0, ()));
        let a2 = scene.add_vertex(Vertex::with_depth(0.0, 2.0, 0.0, ()));
        let tri_a = scene.add_triangle(Triangle { id: 10, vertices: [a0, a1, a2] });
        // Triangle 20: далеко за пределами клипа.
        let b0 = scene.add_vertex(Vertex::with_depth(-100.0, -100.0, 0.0, ()));
        let b1 = scene.add_vertex(Vertex::with_depth(-98.0, -100.0, 0.0, ()));
        let b2 = scene.add_vertex(Vertex::with_depth(-100.0, -98.0, 0.0, ()));
        let tri_b = scene.add_triangle(Triangle { id: 20, vertices: [b0, b1, b2] });

        let plane = Plane { triangles: vec![tri_a, tri_b] };
        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1]);
        process_triangles(
            &mut buf, &plane, &scene, 0,
            &identity_transform(), 0, 0, 4, 4,
        );

        let cells: Vec<_> = buf.cells().collect();
        assert!(!cells.is_empty(), "expected cells from triangle 10");
        for (_, _, _, stack) in cells {
            assert_eq!(
                stack[0].object_id, 10,
                "only triangle 10 should have been written"
            );
        }
    }

    #[test]
    fn layer_index_is_passed_through() {
        let mut scene = Scene::<()>::new();
        let v0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, ()));
        let v1 = scene.add_vertex(Vertex::with_depth(2.0, 0.0, 0.0, ()));
        let v2 = scene.add_vertex(Vertex::with_depth(0.0, 2.0, 0.0, ()));
        let tri = scene.add_triangle(Triangle { id: 1, vertices: [v0, v1, v2] });
        let plane = Plane { triangles: vec![tri] };

        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1, 1, 1]);
        process_triangles(
            &mut buf, &plane, &scene, 2,
            &identity_transform(), 0, 0, 4, 4,
        );

        let cells: Vec<_> = buf.cells().collect();
        assert!(!cells.is_empty());
        for (_, _, layer, _) in cells {
            assert_eq!(layer, 2);
        }
    }
}