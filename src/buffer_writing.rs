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
        let dy = y as f32 - min_y as f32;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn assert_approx_eq(actual: f32, expected: f32, msg: &str) {
        let diff = (actual - expected).abs();
        let scale = expected.abs().max(actual.abs()).max(1.0);
        assert!(
            diff <= 1e-5 * scale,
            "{}: expected {}, got {} (diff {})",
            msg, expected, actual, diff
        );
    }

    fn make_buffer(width: usize, height: usize, capacity: usize) -> ElementBuffer<()> {
        ElementBuffer::new(width, height, vec![capacity])
    }

    fn vert<A: Attributes>(x: f32, y: f32, depth: f32, attr: A) -> Vertex<A> {
        Vertex::with_depth(x, y, depth, attr)
    }

    // -----------------------------------------------------------------
    // edge_func
    // -----------------------------------------------------------------

    #[test]
    fn edge_func_positive_area_for_ccw() {
        // (0,0) -> (1,0) -> (0,1) — CCW, удвоенная площадь = 1.
        assert_approx_eq(edge_func(0.0, 0.0, 1.0, 0.0, 0.0, 1.0), 1.0, "ccw");
    }

    #[test]
    fn edge_func_negative_area_for_cw() {
        assert_approx_eq(edge_func(0.0, 0.0, 0.0, 1.0, 1.0, 0.0), -1.0, "cw");
    }

    #[test]
    fn edge_func_zero_for_collinear_points() {
        assert_approx_eq(edge_func(0.0, 0.0, 1.0, 1.0, 2.0, 2.0), 0.0, "collinear");
    }

    #[test]
    fn edge_func_flips_sign_on_swap() {
        let a = edge_func(0.0, 0.0, 1.0, 0.0, 0.0, 1.0);
        let b = edge_func(0.0, 0.0, 0.0, 1.0, 1.0, 0.0);
        assert_approx_eq(a, -b, "swap");
    }

    #[test]
    fn edge_func_is_cyclic() {
        let e1 = edge_func(0.0, 0.0, 1.0, 0.0, 0.0, 1.0);
        let e2 = edge_func(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let e3 = edge_func(0.0, 1.0, 0.0, 0.0, 1.0, 0.0);
        assert_approx_eq(e1, e2, "cyclic");
        assert_approx_eq(e2, e3, "cyclic");
    }

    // -----------------------------------------------------------------
    // discretize_triangle — геометрия
    // -----------------------------------------------------------------

    #[test]
    fn triangle_at_origin_fills_expected_cells() {
        // Треугольник (0,0)-(2,0)-(0,2). Внутри x+y <= 2.
        // Пиксели с центром внутри: (0,0) @ (0.5,0.5) sum=1;
        // (1,0) @ (1.5,0.5) sum=2 boundary; (0,1) @ (0.5,1.5) sum=2 boundary.
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(0.0, 0.0, 0.0, ()),
            vert(2.0, 0.0, 1.0, ()),
            vert(0.0, 2.0, 2.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);

        let mut cells: Vec<_> = buf.cells().map(|(x, y, _, _)| (x, y)).collect();
        cells.sort();
        assert_eq!(cells, vec![(0, 0), (0, 1), (1, 0)]);
    }

    #[test]
    fn depth_is_interpolated_at_pixel_centers() {
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(0.0, 0.0, 0.0, ()),
            vert(2.0, 0.0, 1.0, ()),
            vert(0.0, 2.0, 2.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 7, 0, 0, 0, 4, 4);

        // (0,0): w = (0.5, 0.25, 0.25) -> depth 0.75
        let s = buf.stack(0, 0, 0).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].object_id, 7);
        assert_approx_eq(s[0].depth, 0.75, "depth at (0,0)");

        // (1,0): w = (0, 0.75, 0.25) -> depth 1.25
        let s = buf.stack(1, 0, 0).unwrap();
        assert_approx_eq(s[0].depth, 1.25, "depth at (1,0)");

        // (0,1): w = (0, 0.25, 0.75) -> depth 1.75
        let s = buf.stack(0, 1, 0).unwrap();
        assert_approx_eq(s[0].depth, 1.75, "depth at (0,1)");
    }

    #[test]
    fn winding_order_does_not_affect_result() {
        let all = [
            vert(0.0, 0.0, 0.0, ()),
            vert(2.0, 0.0, 1.0, ()),
            vert(0.0, 2.0, 2.0, ()),
        ];

        let mut buf_ccw = make_buffer(4, 4, 1);
        discretize_triangle(&mut buf_ccw, &[all[0], all[1], all[2]], 1, 0, 0, 0, 4, 4);

        let mut buf_cw = make_buffer(4, 4, 1);
        discretize_triangle(&mut buf_cw, &[all[0], all[2], all[1]], 1, 0, 0, 0, 4, 4);

        let mut ccw_cells: Vec<_> = buf_ccw.cells().map(|(x, y, _, _)| (x, y)).collect();
        let mut cw_cells: Vec<_> = buf_cw.cells().map(|(x, y, _, _)| (x, y)).collect();
        ccw_cells.sort();
        cw_cells.sort();
        assert_eq!(ccw_cells, cw_cells, "same cells");

        for &(x, y) in &ccw_cells {
            let d_ccw = buf_ccw.stack(x, y, 0).unwrap()[0].depth;
            let d_cw = buf_cw.stack(x, y, 0).unwrap()[0].depth;
            assert_approx_eq(d_ccw, d_cw, &format!("depth at ({},{})", x, y));
        }
    }

    #[test]
    fn degenerate_triangle_writes_nothing() {
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(0.5, 0.5, 1.0, ()),
            vert(1.5, 1.5, 1.0, ()),
            vert(2.5, 2.5, 1.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);
        assert_eq!(buf.cells().count(), 0);
    }

    #[test]
    fn triangle_outside_buffer_writes_nothing() {
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(-10.0, 0.0, 1.0, ()),
            vert(-10.0, 3.0, 1.0, ()),
            vert(-5.0, 0.0, 1.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);
        assert_eq!(buf.cells().count(), 0);
    }

    #[test]
    fn negative_coords_are_clipped_to_buffer() {
        // Треугольник (-1,-1),(2,-1),(-1,2): внутри x+y <= 1.
        // Единственный пиксель в буфере с центром внутри — (0,0).
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(-1.0, -1.0, 0.0, ()),
            vert(2.0, -1.0, 0.0, ()),
            vert(-1.0, 2.0, 0.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);

        let cells: Vec<_> = buf.cells().map(|(x, y, _, _)| (x, y)).collect();
        assert_eq!(cells, vec![(0, 0)]);
    }

    #[test]
    fn clip_restricts_writes_to_clip_rect() {
        // Большой треугольник, покрывающий буфер (x+y <= 5).
        // Клип (1,1,2,2) -> должны остаться только пиксели (1..=2)x(1..=2).
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(-5.0, -5.0, 0.0, ()),
            vert(10.0, -5.0, 0.0, ()),
            vert(-5.0, 10.0, 0.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 1, 1, 2, 2);

        let mut cells: Vec<_> = buf.cells().map(|(x, y, _, _)| (x, y)).collect();
        cells.sort();
        assert_eq!(cells, vec![(1, 1), (1, 2), (2, 1), (2, 2)]);
    }

    #[test]
    fn layer_and_object_id_are_preserved() {
        // Слой 2 в трёхслойном буфере.
        let mut buf = ElementBuffer::<()>::new(4, 4, vec![1, 1, 1]);
        let verts = [
            vert(0.5, 0.5, 0.0, ()),
            vert(1.5, 0.5, 0.0, ()),
            vert(0.5, 1.5, 0.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 99, 2, 0, 0, 4, 4);

        for (_, _, layer, stack) in buf.cells() {
            assert_eq!(layer, 2);
            for el in stack {
                assert_eq!(el.object_id, 99);
            }
        }
    }

    // -----------------------------------------------------------------
    // discretize_triangle — атрибуты
    // -----------------------------------------------------------------

    #[test]
    fn f32_attribute_is_interpolated() {
        let mut buf = ElementBuffer::<f32>::new(4, 4, vec![1]);
        let verts = [
            Vertex::with_depth(0.0, 0.0, 0.0, 100.0),
            Vertex::with_depth(2.0, 0.0, 0.0, 200.0),
            Vertex::with_depth(0.0, 2.0, 0.0, 300.0),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);

        // (0,0): 0.5*100 + 0.25*200 + 0.25*300 = 175
        assert_approx_eq(buf.stack(0, 0, 0).unwrap()[0].attr, 175.0, "attr (0,0)");
        // (1,0): 0*100 + 0.75*200 + 0.25*300 = 225
        assert_approx_eq(buf.stack(1, 0, 0).unwrap()[0].attr, 225.0, "attr (1,0)");
    }

    #[test]
    fn array_attribute_is_interpolated_component_wise() {
        let mut buf = ElementBuffer::<[f32; 2]>::new(4, 4, vec![1]);
        let verts = [
            Vertex::with_depth(0.0, 0.0, 0.0, [0.0, 10.0]),
            Vertex::with_depth(2.0, 0.0, 0.0, [20.0, 30.0]),
            Vertex::with_depth(0.0, 2.0, 0.0, [40.0, 50.0]),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);

        // (0,0): w = (0.5, 0.25, 0.25)
        // attr[0] = 0.5*0 + 0.25*20 + 0.25*40 = 15
        // attr[1] = 0.5*10 + 0.25*30 + 0.25*50 = 25
        let a = buf.stack(0, 0, 0).unwrap()[0].attr;
        assert_approx_eq(a[0], 15.0, "attr[0]");
        assert_approx_eq(a[1], 25.0, "attr[1]");
    }

    /// Атрибут-зонд: возвращает сумму барицентрических весов.
    /// Позволяет проверить контракт `w[0] + w[1] + w[2] == 1` из `Attributes::bary`.
    #[derive(Clone, Copy, Debug)]
    struct WeightSum(f32);

    impl Attributes for WeightSum {
        fn placeholder() -> Self {
            WeightSum(0.0)
        }
        fn bary(_: &Self, _: &Self, _: &Self, w: [f32; 3]) -> Self {
            WeightSum(w[0] + w[1] + w[2])
        }
    }

    #[test]
    fn barycentric_weights_sum_to_one() {
        let mut buf = ElementBuffer::<WeightSum>::new(8, 8, vec![1]);
        let verts = [
            Vertex::with_depth(0.0, 0.0, 0.0, WeightSum(0.0)),
            Vertex::with_depth(7.0, 0.0, 0.0, WeightSum(0.0)),
            Vertex::with_depth(0.0, 7.0, 0.0, WeightSum(0.0)),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 8, 8);

        let mut seen = 0;
        for (x, y, _, stack) in buf.cells() {
            for el in stack {
                seen += 1;
                assert_approx_eq(
                    el.attr.0,
                    1.0,
                    &format!("bary sum at ({},{})", x, y),
                );
            }
        }
        assert!(seen > 0, "expected at least one written cell");
    }

    // -----------------------------------------------------------------
    // discretize_triangle — защита от переполнения и нефинитных данных
    // -----------------------------------------------------------------

    #[test]
    fn clip_saturation_keeps_triangle_visible() {
        // clip_x = 10, clip_w = u32::MAX - 4: без `saturating_add`
        // `clip_x + clip_w` завернулось бы в 5, и `clip_x2 - 1 = 4`.
        // Треугольник на x in [25, 30] оказался бы за пределами «завёрнутого» клипа.
        // С `saturating_add` clip_x2 = u32::MAX, min(buffer.width=100) = 100,
        // и треугольник полностью виден.
        let mut buf = make_buffer(100, 10, 1);
        let verts = [
            vert(25.0, 1.0, 0.0, ()),
            vert(25.0, 9.0, 0.0, ()),
            vert(30.0, 5.0, 0.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 10, 0, u32::MAX - 4, 10);
        assert!(
            buf.cells().count() > 0,
            "triangle lost due to clip overflow"
        );
    }

    #[test]
    #[should_panic(expected = "non-finite vertex data")]
    fn nan_x_panics() {
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(f32::NAN, 0.0, 0.0, ()),
            vert(1.0, 0.0, 0.0, ()),
            vert(0.0, 1.0, 0.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);
    }

    #[test]
    #[should_panic(expected = "non-finite vertex data")]
    fn infinite_depth_panics() {
        let mut buf = make_buffer(4, 4, 1);
        let verts = [
            vert(0.0, 0.0, f32::INFINITY, ()),
            vert(1.0, 0.0, 0.0, ()),
            vert(0.0, 1.0, 0.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 4, 4);
    }

    #[test]
    fn bottom_edge_covers_row_zero_when_sampled_at_pixel_center() {
        // Треугольник с плоским нижним ребром на y = 0.25.
        // Центр первого пикселя строки 0 — (x + 0.5, 0.5) — выше нижнего ребра,
        // значит пиксель покрыт. Баговый сэмплинг в (x + 0.5, 0) даёт y = 0 < 0.25,
        // и вся строка 0 теряется.
        let mut buf = make_buffer(12, 12, 1);
        let verts = [
            vert(0.0, 0.25, 0.0, ()),
            vert(10.0, 0.25, 0.0, ()),
            vert(0.0, 10.25, 0.0, ()),
        ];
        discretize_triangle(&mut buf, &verts, 1, 0, 0, 0, 12, 12);

        let cells: Vec<(usize, usize)> = buf.cells().map(|(x, y, _, _)| (x, y)).collect();

        // Точка-улика: в баговом варианте сэмпл (0.5, 0) даёт w2 = -0.025,
        // пиксель не пишется вовсе.
        assert!(
            cells.contains(&(0, 0)),
            "pixel (0,0) should be covered: its center (0.5, 0.5) is inside"
        );

        // Вся нижняя кромка: центр каждого пикселя строки 0 (x + 0.5, 0.5)
        // выше нижнего ребра и левее гипотенузы x + y = 10.25, значит
        // x = 0..=9 покрыты.
        let row0: Vec<(usize, usize)> =
            cells.iter().copied().filter(|&(_, y)| y == 0).collect();
        assert_eq!(
            row0.len(),
            10,
            "bottom row should cover exactly 10 pixels, got {:?}",
            row0
        );

        // За гипотенузой — не покрыто.
        assert!(!cells.contains(&(10, 0)), "pixel (10,0) is outside");

        // Итого: сумма по строкам 10 + 9 + 8 + ... + 1 = 55.
        // Баговый вариант теряет всю строку 0, остаётся 45.
        assert_eq!(cells.len(), 55, "total covered cells");
    }
}