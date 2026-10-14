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