// src/discretization.rs
use crate::datatypes::*;
use crate::viewport_process::compute_viewport_transform;
use crate::primitives_processing::process_triangles;

/// Основная функция дискретизации.
///
/// Проходит по слоям в порядке, заданном в `Configuration::layers`,
/// для каждой пары `(view, port)` вычисляет viewport-преобразование
/// и раскладывает геометрию плоскости по клеткам буфера.
///
/// Никакого смешивания не производится: каждый попавший в клетку элемент
/// сохраняется в стеке клетки, отсортированный по глубине. Интерпретатор
/// сам решает, что с этим стеком делать.
pub fn discretize<A: Attributes>(task: &Task<A>) -> ElementBuffer<A> {
    let config = &task.config;
    let layers_count = config.layers.len();

    assert!(
        layers_count <= u32::MAX as usize,
        "Too many layers: {} (max {})",
        layers_count, u32::MAX
    );

    let mut buffer = ElementBuffer::new(
        config.output_width,
        config.output_height,
        layers_count,
        config.max_overlap,
    );

    for (layer_index, layer) in config.layers.iter().enumerate() {
        let layer_id = layer_index as u32;
        let plane = &task.scene.planes[layer.plane_index];

        for &(view_idx, port_idx) in &layer.bindings {
            let view = &config.views[view_idx];
            let port = &config.ports[port_idx];

            let (clip_x, clip_y, clip_w, clip_h) = port.resolved_rect(config);
            let xform = compute_viewport_transform(
                view,
                port,
                clip_w as f32,
                clip_h as f32,
            );

            process_triangles(
                &mut buffer,
                plane,
                &task.scene,
                layer_id,
                &xform,
                clip_x, clip_y, clip_w, clip_h,
            );
        }
    }

    buffer
}