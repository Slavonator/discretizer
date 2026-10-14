// src/buffer_writing.rs
use crate::datatypes::{Attributes, Element, ElementBuffer, Vertex};

/// Удвоенная ориентированная площадь треугольника `(a, b, c)`.
/// Знак зависит от порядка обхода: положительна при одном,
/// отрицательна при противоположном.
#[inline(always)]
fn edge_func(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32) -> f32 {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// Записывает один фрагмент в буфер.
///
/// Сортировка и вытеснение — ответственность `ElementBuffer::push`.
#[inline(always)]
fn write_element<A: Attributes>(
    buffer: &mut ElementBuffer<A>,
    x: u32,
    y: u32,
    layer: u32,
    object_id: u32,
    depth: f32,
    attr: A,
) {
    debug_assert!(depth.is_finite(), "Fragment depth must be finite");
    buffer.push(
        x as usize,
        y as usize,
        layer as usize,
        Element { object_id, depth, attr },
    );
}

/// Растеризует один треугольник.
///
/// Координаты вершин — в координатах буфера (после `apply_viewport`).
/// Bounding box обрезается по клип-области `(clip_x, clip_y, clip_w, clip_h)`
/// и по размерам буфера. Глубина и атрибуты интерполируются
/// барицентрически; конкретный способ интерполяции атрибутов
/// определяется реализацией `Attributes::bary` для типа `A`.
///
/// # Panics
/// Если координаты или глубина любой из вершин не конечны.
pub(crate) fn discretize_triangle<A: Attributes>(
    buffer: &mut ElementBuffer<A>,
    vertices: &[Vertex<A>; 3],
    tri_id: u32,
    layer: u32,
    clip_x: u32,
    clip_y: u32,
    clip_w: u32,
    clip_h: u32,
) {
    let v0 = &vertices[0];
    let v1 = &vertices[1];
    let v2 = &vertices[2];

    assert!(
        [v0, v1, v2].iter().all(|v|
            v.x.is_finite() && v.y.is_finite() && v.depth.is_finite()
        ),
        "Triangle {} has non-finite vertex data",
        tri_id
    );

    // Bounding box треугольника в координатах буфера.
    let min_x_f = v0.x.min(v1.x).min(v2.x);
    let min_y_f = v0.y.min(v1.y).min(v2.y);
    let max_x_f = v0.x.max(v1.x).max(v2.x);
    let max_y_f = v0.y.max(v1.y).max(v2.y);

    // Границы клип-области, дополнительно обрезанные по буферу.
    // `saturating_add` — на случай, если пользователь задал порт
    // шире, чем буфер: сумма не переполнится.
    let clip_x2 = clip_x.saturating_add(clip_w).min(buffer.width as u32);
    let clip_y2 = clip_y.saturating_add(clip_h).min(buffer.height as u32);

    // Пересечение bounding box и клипа. Считаем в i64, чтобы
    // отрицательные значения из `floor`/`ceil` не превращались
    // в огромные u32 при касте.
    let min_x = (min_x_f.floor() as i64).max(clip_x as i64).max(0);
    let min_y = (min_y_f.floor() as i64).max(clip_y as i64).max(0);
    let max_x = (max_x_f.ceil() as i64)
        .min(clip_x2 as i64 - 1)
        .min(buffer.width as i64 - 1);
    let max_y = (max_y_f.ceil() as i64)
        .min(clip_y2 as i64 - 1)
        .min(buffer.height as i64 - 1);

    if min_x > max_x || min_y > max_y {
        return;
    }

    let min_x = min_x as u32;
    let min_y = min_y as u32;
    let max_x = max_x as u32;
    let max_y = max_y as u32;

    // Удвоенная ориентированная площадь. При нуле треугольник вырожден.
    let area = edge_func(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
    if area == 0.0 {
        return;
    }
    let inv_area = 1.0 / area;

    // Приращения барицентрических координат по x и y.
    let dw0_dx = (v1.y - v2.y) * inv_area;
    let dw0_dy = (v2.x - v1.x) * inv_area;
    let dw1_dx = (v2.y - v0.y) * inv_area;
    let dw1_dy = (v0.x - v2.x) * inv_area;
    let dw2_dx = (v0.y - v1.y) * inv_area;
    let dw2_dy = (v1.x - v0.x) * inv_area;

    // Веса в центре первого пикселя bounding box.
    let start_x = min_x as f32 + 0.5;
    let start_y = min_y as f32 + 0.5;
    let w0_start = edge_func(v1.x, v1.y, v2.x, v2.y, start_x, start_y) * inv_area;
    let w1_start = edge_func(v2.x, v2.y, v0.x, v0.y, start_x, start_y) * inv_area;
    let w2_start = edge_func(v0.x, v0.y, v1.x, v1.y, start_x, start_y) * inv_area;

    for y in min_y..=max_y {
        let dy = y as f32 - start_y;
        let mut w0 = w0_start + dy * dw0_dy;
        let mut w1 = w1_start + dy * dw1_dy;
        let mut w2 = w2_start + dy * dw2_dy;

        for x in min_x..=max_x {
            // Внутри треугольника: все веса одного знака.
            // Знак знаменателя зависит от порядка обхода — отсюда две ветки.
            let inside = (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0)
                || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);

            if inside {
                // Глубина — структурное поле вершины, линейная интерполяция.
                let depth = w0 * v0.depth + w1 * v1.depth + w2 * v2.depth;

                // Атрибуты — через реализацию `Attributes::bary`.
                let attr = A::bary(&v0.attr, &v1.attr, &v2.attr, [w0, w1, w2]);

                write_element(buffer, x, y, layer, tri_id, depth, attr);
            }

            w0 += dw0_dx;
            w1 += dw1_dx;
            w2 += dw2_dx;
        }
    }
}