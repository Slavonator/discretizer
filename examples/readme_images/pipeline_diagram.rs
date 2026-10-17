use discretizer::*;
use image::{ImageBuffer, Rgba};
use ab_glyph::{FontRef, PxScale};

const W: usize = 800;
const H: usize = 240;

/// Идентификаторы материалов: по ним интерпретатор выбирает цвет.
const OBJ_BLOCK: u32 = 0;
const OBJ_ARROW: u32 = 1;

/// Добавляет отрезок как два треугольника (квадрат, повёрнутый вдоль отрезка).
/// Толщина — полная, перпендикулярная отрезку.
fn add_line_as_quad(
    scene: &mut Scene<()>,
    plane_triangles: &mut Vec<usize>,
    x1: f32, y1: f32, x2: f32, y2: f32,
    thickness: f32,
    object_id: u32,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    assert!(len > 1e-6, "Degenerate line");
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
        vids[i] = scene.add_vertex(Vertex::new(*x, *y, ()));
    }

    plane_triangles.push(scene.add_triangle(Triangle {
        id: object_id,
        vertices: [vids[0], vids[1], vids[2]],
    }));
    plane_triangles.push(scene.add_triangle(Triangle {
        id: object_id,
        vertices: [vids[0], vids[2], vids[3]],
    }));
}

fn main() {
    let vw = 80.0_f32;
    let vh = 24.0_f32;

    // --- Провайдер: строим геометрию в координатах плоскости ---
    let mut scene: Scene<()> = Scene::new();
    let mut plane_triangles: Vec<usize> = Vec::new();

    let box_w = 16.0;
    let box_h = 8.0;
    let margin = 8.0;
    let y = (vh - box_h) / 2.0;
    let x1 = 8.0;
    let x2 = x1 + box_w + margin;
    let x3 = x2 + box_w + margin;
    let thickness = 0.5;

    // Три прямоугольника
    for &x in &[x1, x2, x3] {
        let rect = [
            (x, y, x + box_w, y),
            (x + box_w, y, x + box_w, y + box_h),
            (x + box_w, y + box_h, x, y + box_h),
            (x, y + box_h, x, y),
        ];
        for &(lx1, ly1, lx2, ly2) in &rect {
            add_line_as_quad(
                &mut scene, &mut plane_triangles,
                lx1, ly1, lx2, ly2, thickness, OBJ_BLOCK,
            );
        }
    }

    // Стрелки
    let arrow_y = y + box_h / 2.0;
    let tip_w = 1.0;
    let tip_h = 1.0;
    let tip_x = x2 - (thickness / 10.0);
    add_line_as_quad(&mut scene, &mut plane_triangles, x1 + box_w, arrow_y, tip_x, arrow_y, 0.5, OBJ_ARROW);
    add_line_as_quad(&mut scene, &mut plane_triangles, tip_x, arrow_y, tip_x - tip_h, arrow_y - tip_w, 0.5, OBJ_ARROW);
    add_line_as_quad(&mut scene, &mut plane_triangles, tip_x, arrow_y, tip_x - tip_h, arrow_y + tip_w, 0.5, OBJ_ARROW);

    let tip_x2 = x3 - (thickness / 10.0);
    add_line_as_quad(&mut scene, &mut plane_triangles, x2 + box_w, arrow_y, tip_x2, arrow_y, 0.5, OBJ_ARROW);
    add_line_as_quad(&mut scene, &mut plane_triangles, tip_x2, arrow_y, tip_x2 - tip_h, arrow_y - tip_w, 0.5, OBJ_ARROW);
    add_line_as_quad(&mut scene, &mut plane_triangles, tip_x2, arrow_y, tip_x2 - tip_h, arrow_y + tip_w, 0.5, OBJ_ARROW);

    let plane_idx = scene.add_plane(Plane { triangles: plane_triangles });

    // --- Конфигурация: вид, порт, слой ---
    let mut config = Configuration::new(W, H);

    let view_idx = config.add_view(View::new(0.0, 0.0, vw, vh));
    let port_idx = config.add_port(Port::new(ScalingMode::Cover));

    config.add_layer(Layer::new(plane_idx, vec![(view_idx, port_idx)], 1));

    // --- Дискретизация ---
    let task = Task::new(scene, config);
    let buffer = discretize(&task);

    // --- Интерпретатор: рендер в PNG ---
    let bg = [240u8, 240, 240, 255];
    let block_color = [50u8, 120, 220, 255];
    let arrow_color = [180u8, 180, 180, 255];

    let mut img = ImageBuffer::new(W as u32, H as u32);
    for py in 0..H {
        for px in 0..W {
            let color = match buffer.stack(px, py, 0).and_then(|s| s.first()) {
                Some(elem) => match elem.object_id {
                    OBJ_BLOCK => block_color,
                    OBJ_ARROW => arrow_color,
                    _ => bg,
                },
                None => bg,
            };
            img.put_pixel(px as u32, py as u32, Rgba(color));
        }
    }

    // --- Текст поверх изображения ---
    let words = ["провайдер", "дискретизатор", "интерпретатор"];
    let font_data = include_bytes!("../assets/DejaVuSans.ttf");
    let font = FontRef::try_from_slice(font_data).expect("Error loading font");
    let scale = PxScale { x: 20.0, y: 20.0 };
    let grey = Rgba([70u8, 70u8, 70u8, 255u8]);

    let scale_factor = 10.0;
    let centers = [
        (x1 + box_w / 2.0, y + box_h / 2.0),
        (x2 + box_w / 2.0, y + box_h / 2.0),
        (x3 + box_w / 2.0, y + box_h / 2.0),
    ];

    for (i, (cx, cy)) in centers.iter().enumerate() {
        let text = words[i];
        let px = (cx * scale_factor) as i32;
        let py = (cy * scale_factor) as i32;
        let text_width = text.chars().count() as i32 * 10;
        let x_pos = px - text_width / 2;
        let y_pos = py - 12;
        imageproc::drawing::draw_text_mut(&mut img, grey, x_pos, y_pos, scale, &font, text);
    }

    img.save("pipeline.png").expect("Failed to save PNG with text");
    println!("Saved pipeline.png");
}