use ab_glyph::{FontRef, PxScale};
use image::{ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;
use overwhelement::*;

const W: usize = 800;
const H: usize = 800;

// Идентификаторы материалов: интерпретатор по ним выбирает цвет.
const OBJ_PLANE_FILL: u32 = 0;
const OBJ_PLANE_OUTLINE: u32 = 1;
const OBJ_VIEW_FILL: u32 = 2;
const OBJ_VIEW_OUTLINE: u32 = 3;

/// Добавляет отрезок как два треугольника — тонкий четырёхугольник,
/// повёрнутый вдоль отрезка. Толщина — в координатах плоскости.
fn add_line_as_quad(
    scene: &mut Scene<()>,
    tris: &mut Vec<usize>,
    x1: f32, y1: f32, x2: f32, y2: f32,
    thickness: f32,
    depth: f32,
    object_id: u32,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 { return; }
    let px = -dy / len * thickness * 0.5;
    let py =  dx / len * thickness * 0.5;

    let corners = [
        (x1 + px, y1 + py),
        (x2 + px, y2 + py),
        (x2 - px, y2 - py),
        (x1 - px, y1 - py),
    ];

    let mut vids = [0usize; 4];
    for (i, (x, y)) in corners.iter().enumerate() {
        vids[i] = scene.add_vertex(Vertex::with_depth(*x, *y, depth, ()));
    }
    tris.push(scene.add_triangle(Triangle {
        id: object_id,
        vertices: [vids[0], vids[1], vids[2]],
    }));
    tris.push(scene.add_triangle(Triangle {
        id: object_id,
        vertices: [vids[0], vids[2], vids[3]],
    }));
}

/// Добавляет четырёхугольник как два треугольника.
fn add_quad(
    scene: &mut Scene<()>,
    tris: &mut Vec<usize>,
    corners: [(f32, f32); 4],
    depth: f32,
    object_id: u32,
) {
    let mut vids = [0usize; 4];
    for (i, (x, y)) in corners.iter().enumerate() {
        vids[i] = scene.add_vertex(Vertex::with_depth(*x, *y, depth, ()));
    }
    tris.push(scene.add_triangle(Triangle {
        id: object_id,
        vertices: [vids[0], vids[1], vids[2]],
    }));
    tris.push(scene.add_triangle(Triangle {
        id: object_id,
        vertices: [vids[0], vids[2], vids[3]],
    }));
}

fn main() {
    let mut scene: Scene<()> = Scene::new();
    let mut tris: Vec<usize> = Vec::new();

    // --- Плоскость: заливка ---
    let plane_corners = [(1.0, 1.0), (10.0, 1.0), (14.0, 11.0), (4.0, 11.0)];
    add_quad(&mut scene, &mut tris, plane_corners, 1.0, OBJ_PLANE_FILL);

    // --- Плоскость: обводка ---
    // Толщина подобрана так, чтобы в буфере получилось ~5 пикселей:
    // 1 единица плоскости ≈ 53 пикселя при Contain на 800×800.
    let outline_thickness = 0.1;
    for i in 0..4 {
        let (x1, y1) = plane_corners[i];
        let (x2, y2) = plane_corners[(i + 1) % 4];
        add_line_as_quad(
            &mut scene, &mut tris,
            x1, y1, x2, y2,
            outline_thickness, 0.9, OBJ_PLANE_OUTLINE,
        );
    }

    // --- View: заливка ---
    let view_corners = [(1.5, 1.25), (3.15, 6.8), (7.1, 6.8), (5.0, 1.25)];
    add_quad(&mut scene, &mut tris, view_corners, 0.3, OBJ_VIEW_FILL);

    // --- View: обводка ---
    for i in 0..4 {
        let (x1, y1) = view_corners[i];
        let (x2, y2) = view_corners[(i + 1) % 4];
        add_line_as_quad(
            &mut scene, &mut tris,
            x1, y1, x2, y2,
            outline_thickness, 0.2, OBJ_VIEW_OUTLINE,
        );
    }

    let plane_idx = scene.add_plane(Plane { triangles: tris });

    // --- Конфигурация ---
    // View захватывает всю сцену: отрицательная ширина + поворот 180°,
    // как в старом примере. Это даёт визуально тот же кадр.
    let view = View::new(15.0, 0.0, -15.0, 15.0).with_rotation(3.1415);
    let port = Port::new(ScalingMode::Contain);

    let mut config = Configuration::new(W, H);
    let view_idx = config.add_view(view);
    let port_idx = config.add_port(port);
    config.add_layer(Layer::new(plane_idx, vec![(view_idx, port_idx)]));

    let task = Task::new(scene, config);
    let buffer = discretize(&task);

    // --- Интерпретация ---
    let bg            = [240u8, 240u8, 240u8, 255u8];
    let plane_fill    = [130u8, 130u8, 230u8, 255u8];
    let plane_outline = [ 90u8,  90u8, 190u8, 255u8];
    let view_fill     = [140u8, 230u8, 140u8, 255u8];
    let view_outline  = [110u8, 190u8, 110u8, 255u8];

    let mut img = ImageBuffer::new(W as u32, H as u32);
    for py in 0..H {
        for px in 0..W {
            let color = match buffer.stack(px, py, 0).and_then(|s| s.first()) {
                Some(e) => match e.object_id {
                    OBJ_PLANE_FILL    => plane_fill,
                    OBJ_PLANE_OUTLINE => plane_outline,
                    OBJ_VIEW_FILL     => view_fill,
                    OBJ_VIEW_OUTLINE  => view_outline,
                    _ => bg,
                },
                None => bg,
            };
            img.put_pixel(px as u32, py as u32, Rgba(color));
        }
    }

    // --- Текст поверх ---
    let font_data = include_bytes!("../assets/DejaVuSans.ttf");
    let font = FontRef::try_from_slice(font_data).expect("Error loading font");
    let scale = PxScale { x: 30.0, y: 30.0 };
    let black = Rgba([0u8, 0u8, 0u8, 255u8]);

    draw_text_mut(&mut img, black, 226, 226, scale, &font, "Plane");
    draw_text_mut(&mut img, black, 180, 450, scale, &font, "View");

    img.save("view.png").expect("Failed to save PNG");
}