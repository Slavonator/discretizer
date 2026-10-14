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