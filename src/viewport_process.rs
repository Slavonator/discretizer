// src/viewport_process.rs
use crate::datatypes::*;
use crate::internal_datatypes::ViewportTransform;

/// Вычисляет преобразование из координат плоскости в координаты порта.
///
/// `out_w`, `out_h` — фактические размеры области порта в буфере
/// (результат `Port::resolved_rect`). Нулевые размеры сюда попасть
/// не должны — за это отвечает `resolved_rect`.
///
/// # Panics
/// Если `out_w` или `out_h` не положительны, `view.width` или `view.height`
/// равны нулю, либо `port.element_aspect_ratio` не положителен.
pub(crate) fn compute_viewport_transform(
    view: &View,
    port: &Port,
    out_w: f32,
    out_h: f32,
) -> ViewportTransform {
    assert!(out_w > 0.0 && out_h > 0.0, "Port resolution must be positive");
    assert!(
        view.width != 0.0 && view.height != 0.0,
        "View dimensions must be non-zero (got {} × {})",
        view.width, view.height
    );
    assert!(
        port.element_aspect_ratio > 0.0 && port.element_aspect_ratio.is_finite(),
        "element_aspect_ratio must be positive and finite, got {}",
        port.element_aspect_ratio
    );

    let aspect = port.element_aspect_ratio;

    // Размеры вида в плоскости, с учётом неквадратности дискреты.
    let abs_w = view.width.abs();
    let abs_h = view.height.abs() * aspect;

    let out_aspect = out_w / out_h;
    let vp_aspect = abs_w / abs_h;

    let (base_scale_x, base_scale_y) = match port.scaling_mode {
        ScalingMode::Stretch => (out_w / abs_w, out_h / abs_h),
        ScalingMode::None => (1.0, 1.0),
        ScalingMode::Contain | ScalingMode::Cover => {
            let use_width = match port.scaling_mode {
                ScalingMode::Contain => vp_aspect > out_aspect,
                ScalingMode::Cover => vp_aspect <= out_aspect,
                _ => unreachable!(),
            };
            if use_width {
                let s = out_w / abs_w;
                (s, s)
            } else {
                let s = out_h / abs_h;
                (s, s)
            }
        }
    };

    let scaled_w = abs_w * base_scale_x;
    let scaled_h = abs_h * base_scale_y;

    let offset_px_x = port.buffer_offset_x as f32;
    let offset_px_y = port.buffer_offset_y as f32;

    let offset_x_base = -view.x * base_scale_x;
    let offset_y_base = -view.y * base_scale_y;

    // Отражение по отрицательным размерам вида.
    let (scale_x, offset_x) = if view.width < 0.0 {
        (-base_scale_x, out_w - (offset_x_base + scaled_w) + offset_px_x)
    } else {
        (base_scale_x, offset_x_base + offset_px_x)
    };

    let (scale_y, offset_y) = if view.height < 0.0 {
        (-base_scale_y, out_h - (offset_y_base + scaled_h) + offset_px_y)
    } else {
        (base_scale_y, offset_y_base + offset_px_y)
    };

    assert!(
        scale_x.is_finite() && scale_y.is_finite(),
        "Resulting scale must be finite (got {} × {})",
        scale_x, scale_y
    );
    assert!(
        offset_x.is_finite() && offset_y.is_finite(),
        "Resulting offset must be finite (got {}, {})",
        offset_x, offset_y
    );

    let (cos, sin, cx, cy) = if view.rotation_angle != 0.0 {
        let cx = view.x + view.width / 2.0;
        let cy = view.y + view.height / 2.0;
        (
            view.rotation_angle.cos(),
            view.rotation_angle.sin(),
            cx,
            cy,
        )
    } else {
        (1.0, 0.0, 0.0, 0.0)
    };

    ViewportTransform {
        scale_x,
        scale_y,
        offset_x,
        offset_y,
        has_rotation: view.rotation_angle != 0.0,
        cos,
        sin,
        cx,
        cy,
        y_aspect: aspect,
    }
}

/// Применяет преобразование к вершине на месте.
pub(crate) fn apply_viewport<A: Attributes>(
    vertex: &mut Vertex<A>,
    xform: &ViewportTransform,
) {
    let mut x = vertex.x;
    let mut y = vertex.y * xform.y_aspect;

    if xform.has_rotation {
        let dx = x - xform.cx;
        let dy = y - xform.cy;
        x = xform.cx + dx * xform.cos - dy * xform.sin;
        y = xform.cy + dx * xform.sin + dy * xform.cos;
    }

    vertex.x = x * xform.scale_x + xform.offset_x;
    vertex.y = y * xform.scale_y + xform.offset_y;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Сравнение f32 с относительной погрешностью.
    /// Работает и для больших чисел, и для нуля.
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

    /// Конструктор `ViewportTransform` без вращения — для тестов `apply_viewport`.
    fn no_rotation(scale_x: f32, scale_y: f32, offset_x: f32, offset_y: f32) -> ViewportTransform {
        ViewportTransform {
            scale_x,
            scale_y,
            offset_x,
            offset_y,
            has_rotation: false,
            cos: 1.0,
            sin: 0.0,
            cx: 0.0,
            cy: 0.0,
            y_aspect: 1.0,
        }
    }

    // -----------------------------------------------------------------
    // compute_viewport_transform
    // -----------------------------------------------------------------

    #[test]
    fn stretch_default_aspect() {
        let view = View::new(0.0, 0.0, 100.0, 100.0);
        let port = Port::new(ScalingMode::Stretch);
        let x = compute_viewport_transform(&view, &port, 200.0, 400.0);

        assert_approx_eq(x.scale_x, 2.0, "scale_x");
        assert_approx_eq(x.scale_y, 4.0, "scale_y");
        assert_approx_eq(x.offset_x, 0.0, "offset_x");
        assert_approx_eq(x.offset_y, 0.0, "offset_y");
        assert!(!x.has_rotation, "has_rotation should be false");
        assert_approx_eq(x.cos, 1.0, "cos");
        assert_approx_eq(x.sin, 0.0, "sin");
        assert_approx_eq(x.cx, 0.0, "cx");
        assert_approx_eq(x.cy, 0.0, "cy");
        assert_approx_eq(x.y_aspect, 1.0, "y_aspect");
    }

    #[test]
    fn stretch_with_view_origin_and_buffer_offset() {
        let view = View::new(10.0, 20.0, 100.0, 100.0);
        let port = Port::new(ScalingMode::Stretch).with_buffer_offset(5, 7);
        let x = compute_viewport_transform(&view, &port, 200.0, 100.0);

        assert_approx_eq(x.scale_x, 2.0, "scale_x");
        assert_approx_eq(x.scale_y, 1.0, "scale_y");
        // offset = -view.x/y * scale + buffer_offset
        assert_approx_eq(x.offset_x, -20.0 + 5.0, "offset_x");
        assert_approx_eq(x.offset_y, -20.0 + 7.0, "offset_y");
    }

    #[test]
    fn none_mode_keeps_unit_scale() {
        let view = View::new(10.0, 20.0, 100.0, 50.0);
        let port = Port::new(ScalingMode::None).with_buffer_offset(5, 7);
        let x = compute_viewport_transform(&view, &port, 200.0, 200.0);

        assert_approx_eq(x.scale_x, 1.0, "scale_x");
        assert_approx_eq(x.scale_y, 1.0, "scale_y");
        assert_approx_eq(x.offset_x, -10.0 + 5.0, "offset_x");
        assert_approx_eq(x.offset_y, -20.0 + 7.0, "offset_y");
    }

    #[test]
    fn contain_view_wider_than_port_fits_by_width() {
        // view aspect = 2.0, port aspect = 1.0
        let view = View::new(0.0, 0.0, 200.0, 100.0);
        let port = Port::new(ScalingMode::Contain);
        let x = compute_viewport_transform(&view, &port, 100.0, 100.0);

        assert_approx_eq(x.scale_x, 0.5, "scale_x");
        assert_approx_eq(x.scale_y, 0.5, "scale_y");
    }

    #[test]
    fn contain_view_taller_than_port_fits_by_height() {
        // view aspect = 0.5, port aspect = 1.0
        let view = View::new(0.0, 0.0, 100.0, 200.0);
        let port = Port::new(ScalingMode::Contain);
        let x = compute_viewport_transform(&view, &port, 100.0, 100.0);

        assert_approx_eq(x.scale_x, 0.5, "scale_x");
        assert_approx_eq(x.scale_y, 0.5, "scale_y");
    }

    #[test]
    fn cover_view_wider_than_port_fits_by_height() {
        let view = View::new(0.0, 0.0, 200.0, 100.0);
        let port = Port::new(ScalingMode::Cover);
        let x = compute_viewport_transform(&view, &port, 100.0, 100.0);

        assert_approx_eq(x.scale_x, 1.0, "scale_x");
        assert_approx_eq(x.scale_y, 1.0, "scale_y");
    }

    #[test]
    fn cover_view_taller_than_port_fits_by_width() {
        let view = View::new(0.0, 0.0, 100.0, 200.0);
        let port = Port::new(ScalingMode::Cover);
        let x = compute_viewport_transform(&view, &port, 100.0, 100.0);

        assert_approx_eq(x.scale_x, 1.0, "scale_x");
        assert_approx_eq(x.scale_y, 1.0, "scale_y");
    }

    #[test]
    fn negative_width_flips_x_scale_and_offsets() {
        let view = View::new(0.0, 0.0, -100.0, 100.0);
        let port = Port::new(ScalingMode::Stretch);
        let x = compute_viewport_transform(&view, &port, 200.0, 100.0);

        assert_approx_eq(x.scale_x, -2.0, "scale_x");
        // out_w - (offset_x_base + scaled_w) = 200 - (0 + 200) = 0
        assert_approx_eq(x.offset_x, 0.0, "offset_x");
    }

    #[test]
    fn negative_height_flips_y_scale_and_offsets() {
        let view = View::new(0.0, 0.0, 100.0, -100.0);
        let port = Port::new(ScalingMode::Stretch);
        let x = compute_viewport_transform(&view, &port, 200.0, 400.0);

        assert_approx_eq(x.scale_y, -4.0, "scale_y");
        assert_approx_eq(x.offset_y, 0.0, "offset_y");
    }

    #[test]
    fn element_aspect_ratio_scales_height_and_is_exposed() {
        let view = View::new(0.0, 0.0, 100.0, 100.0);
        let port = Port::new(ScalingMode::Stretch).with_aspect_ratio(2.0);
        let x = compute_viewport_transform(&view, &port, 100.0, 200.0);

        // abs_h = 100 * 2 = 200; scale_y = 200 / 200 = 1
        assert_approx_eq(x.scale_y, 1.0, "scale_y");
        assert_approx_eq(x.y_aspect, 2.0, "y_aspect");
    }

    #[test]
    fn rotation_sets_trig_and_center() {
        use std::f32::consts::FRAC_PI_2;
        let view = View::new(10.0, 20.0, 100.0, 50.0).with_rotation(FRAC_PI_2);
        let port = Port::new(ScalingMode::Stretch);
        let x = compute_viewport_transform(&view, &port, 100.0, 50.0);

        assert!(x.has_rotation, "has_rotation should be true");
        assert_approx_eq(x.cos, 0.0, "cos");
        assert_approx_eq(x.sin, 1.0, "sin");
        assert_approx_eq(x.cx, 60.0, "cx"); // 10 + 100/2
        assert_approx_eq(x.cy, 45.0, "cy"); // 20 + 50/2
    }

    #[test]
    fn no_rotation_sets_identity_trig() {
        let view = View::new(10.0, 20.0, 100.0, 50.0);
        let port = Port::new(ScalingMode::Stretch);
        let x = compute_viewport_transform(&view, &port, 100.0, 50.0);

        assert!(!x.has_rotation, "has_rotation should be false");
        assert_approx_eq(x.cos, 1.0, "cos");
        assert_approx_eq(x.sin, 0.0, "sin");
        assert_approx_eq(x.cx, 0.0, "cx");
        assert_approx_eq(x.cy, 0.0, "cy");
    }

    // --- Panics (публичный контракт из документации) ---

    #[test]
    #[should_panic(expected = "Port resolution must be positive")]
    fn panics_on_zero_out_width() {
        let view = View::new(0.0, 0.0, 100.0, 100.0);
        let port = Port::new(ScalingMode::Stretch);
        let _ = compute_viewport_transform(&view, &port, 0.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "Port resolution must be positive")]
    fn panics_on_zero_out_height() {
        let view = View::new(0.0, 0.0, 100.0, 100.0);
        let port = Port::new(ScalingMode::Stretch);
        let _ = compute_viewport_transform(&view, &port, 100.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "View dimensions must be non-zero")]
    fn panics_on_zero_view_width() {
        // Обходим валидацию в `View::new`, чтобы проверить именно
        // `assert!` внутри `compute_viewport_transform`.
        let view = View {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 100.0,
            rotation_angle: 0.0,
        };
        let port = Port::new(ScalingMode::Stretch);
        let _ = compute_viewport_transform(&view, &port, 100.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "View dimensions must be non-zero")]
    fn panics_on_zero_view_height() {
        let view = View {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 0.0,
            rotation_angle: 0.0,
        };
        let port = Port::new(ScalingMode::Stretch);
        let _ = compute_viewport_transform(&view, &port, 100.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "element_aspect_ratio must be positive")]
    fn panics_on_zero_aspect_ratio() {
        // Обходим валидацию в `Port::with_aspect_ratio`, чтобы проверить
        // именно `assert!` внутри `compute_viewport_transform`.
        let mut port = Port::new(ScalingMode::Stretch);
        port.element_aspect_ratio = 0.0;
        let view = View::new(0.0, 0.0, 100.0, 100.0);
        let _ = compute_viewport_transform(&view, &port, 100.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "element_aspect_ratio must be positive")]
    fn panics_on_non_finite_aspect_ratio() {
        let mut port = Port::new(ScalingMode::Stretch);
        port.element_aspect_ratio = f32::NAN;
        let view = View::new(0.0, 0.0, 100.0, 100.0);
        let _ = compute_viewport_transform(&view, &port, 100.0, 100.0);
    }

    // Отдельно проверяем, что сам `Port::with_aspect_ratio` паникует
    // до того, как мы дойдём до `compute_viewport_transform`.
    #[test]
    #[should_panic]
    fn port_with_aspect_ratio_panics_on_zero() {
        let _ = Port::new(ScalingMode::Stretch).with_aspect_ratio(0.0);
    }

    #[test]
    #[should_panic]
    fn port_with_aspect_ratio_panics_on_negative() {
        let _ = Port::new(ScalingMode::Stretch).with_aspect_ratio(-1.0);
    }

    #[test]
    #[should_panic]
    fn port_with_aspect_ratio_panics_on_infinity() {
        let _ = Port::new(ScalingMode::Stretch).with_aspect_ratio(f32::INFINITY);
    }

    // -----------------------------------------------------------------
    // apply_viewport
    // -----------------------------------------------------------------

    #[test]
    fn apply_viewport_scale_and_offset_no_rotation() {
        let x = no_rotation(2.0, 3.0, 10.0, 20.0);
        let mut v = Vertex::new(5.0, 7.0, ());
        apply_viewport(&mut v, &x);

        assert_approx_eq(v.x, 5.0 * 2.0 + 10.0, "x");
        assert_approx_eq(v.y, 7.0 * 3.0 + 20.0, "y");
    }

    #[test]
    fn apply_viewport_multiplies_y_by_aspect() {
        let mut xform = no_rotation(1.0, 1.0, 0.0, 0.0);
        xform.y_aspect = 2.0;
        let mut v = Vertex::new(5.0, 7.0, ());
        apply_viewport(&mut v, &xform);

        assert_approx_eq(v.x, 5.0, "x");
        assert_approx_eq(v.y, 7.0 * 2.0, "y");
    }

    #[test]
    fn apply_viewport_rotates_around_center_90deg() {
        use std::f32::consts::FRAC_PI_2;
        let xform = ViewportTransform {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            has_rotation: true,
            cos: FRAC_PI_2.cos(),
            sin: FRAC_PI_2.sin(),
            cx: 10.0,
            cy: 20.0,
            y_aspect: 1.0,
        };
        // Точка справа от центра.
        let mut v = Vertex::new(15.0, 20.0, ());
        apply_viewport(&mut v, &xform);

        assert_approx_eq(v.x, 10.0, "x");
        assert_approx_eq(v.y, 25.0, "y");
    }

    #[test]
    fn apply_viewport_rotation_preserves_center() {
        use std::f32::consts::FRAC_PI_4;
        let xform = ViewportTransform {
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            has_rotation: true,
            cos: FRAC_PI_4.cos(),
            sin: FRAC_PI_4.sin(),
            cx: 10.0,
            cy: 20.0,
            y_aspect: 1.0,
        };
        let mut v = Vertex::new(10.0, 20.0, ());
        apply_viewport(&mut v, &xform);

        assert_approx_eq(v.x, 10.0, "x");
        assert_approx_eq(v.y, 20.0, "y");
    }

    #[test]
    fn apply_viewport_combined_order() {
        // Фиксируем порядок операций:
        //   1) y *= y_aspect
        //   2) поворот вокруг (cx, cy)
        //   3) x/y *= scale, += offset
        use std::f32::consts::FRAC_PI_2;
        let xform = ViewportTransform {
            scale_x: 2.0,
            scale_y: 3.0,
            offset_x: 10.0,
            offset_y: 20.0,
            has_rotation: true,
            cos: FRAC_PI_2.cos(),
            sin: FRAC_PI_2.sin(),
            cx: 0.0,
            cy: 0.0,
            y_aspect: 2.0,
        };
        let mut v = Vertex::new(1.0, 1.0, ());
        apply_viewport(&mut v, &xform);

        // (1, 1) → y_aspect → (1, 2)
        // поворот 90° вокруг (0,0): (1*0 - 2*1, 1*1 + 2*0) = (-2, 1)
        // scale/offset: (-2*2 + 10, 1*3 + 20) = (6, 23)
        assert_approx_eq(v.x, 6.0, "x");
        assert_approx_eq(v.y, 23.0, "y");
    }

    #[test]
    fn apply_viewport_does_not_touch_depth_or_attr() {
        let x = no_rotation(2.0, 2.0, 0.0, 0.0);
        let mut v = Vertex::with_depth(1.0, 1.0, 42.5, ());
        apply_viewport(&mut v, &x);

        assert_approx_eq(v.depth, 42.5, "depth must be unchanged");
    }
    
    #[test]
    fn apply_viewport_rotation_with_aspect_documents_current_behavior() {
        use std::f32::consts::FRAC_PI_2;
        let view = View::new(0.0, 100.0, 100.0, 100.0).with_rotation(FRAC_PI_2);
        let port = Port::new(ScalingMode::Stretch).with_aspect_ratio(2.0);
        let xform = compute_viewport_transform(&view, &port, 100.0, 200.0);

        // cy = 100 + 50 = 150 (не 300 = 150 * y_aspect)
        assert_approx_eq(xform.cy, 150.0, "cy");
        assert_approx_eq(xform.y_aspect, 2.0, "y_aspect");

        let mut v = Vertex::new(50.0, 150.0, ());
        apply_viewport(&mut v, &xform);

        // y = 150 * 2 = 300
        // dx = 50 - 50 = 0, dy = 300 - 150 = 150
        // поворот 90°: x' = 50 - 150 = -100, y' = 150
        // scale_x = scale_y = 1, offset_x = 0, offset_y = -100
        assert_approx_eq(v.x, -100.0, "x");
        assert_approx_eq(v.y, 50.0, "y");
    }

    #[test]
    #[should_panic(expected = "View width must be non-zero")]
    fn view_new_panics_on_zero_width() {
        let _ = View::new(0.0, 0.0, 0.0, 100.0);
    }

    #[test]
    #[should_panic(expected = "View height must be non-zero")]
    fn view_new_panics_on_zero_height() {
        let _ = View::new(0.0, 0.0, 100.0, 0.0);
    }
}