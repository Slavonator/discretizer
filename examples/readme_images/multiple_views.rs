use ab_glyph::{FontRef, PxScale};
use image::{ImageBuffer, Rgba};
use imageproc::drawing::draw_text_mut;
use discretizer::*;

const W: usize = 800;
const H: usize = 400;

// Материалы: интерпретатор по ним выбирает цвет.
const OBJ_PLANE_FILL: u32 = 0;
const OBJ_PLANE_OUTLINE: u32 = 1;
const OBJ_INNER1: u32 = 2;
const OBJ_INNER2: u32 = 3;
const OBJ_RECT1: u32 = 4;
const OBJ_RECT2: u32 = 5;

/// Отрезок → тонкий четырёхугольник (2 треугольника).
fn add_line_as_quad(
    scene: &mut Scene<()>,
    tris: &mut Vec<usize>,
    x1: f32, y1: f32, x2: f32, y2: f32,
    thickness: f32,
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
        vids[i] = scene.add_vertex(Vertex::with_depth(*x, *y, 0.0, ()));
    }
    tris.push(scene.add_triangle(Triangle { id: object_id, vertices: [vids[0], vids[1], vids[2]] }));
    tris.push(scene.add_triangle(Triangle { id: object_id, vertices: [vids[0], vids[2], vids[3]] }));
}

/// Замкнутый контур по точкам — набор отрезков.
fn add_outline(
    scene: &mut Scene<()>,
    tris: &mut Vec<usize>,
    points: &[(f32, f32)],
    thickness: f32,
    object_id: u32,
) {
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        add_line_as_quad(scene, tris, x1, y1, x2, y2, thickness, object_id);
    }
}

/// Четырёхугольник как два треугольника.
fn add_quad_tri(
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
    tris.push(scene.add_triangle(Triangle { id: object_id, vertices: [vids[0], vids[1], vids[2]] }));
    tris.push(scene.add_triangle(Triangle { id: object_id, vertices: [vids[0], vids[2], vids[3]] }));
}

fn main() {
    let mut scene: Scene<()> = Scene::new();

    // ---------- Plane 0: реальная сцена ----------
    let mut plane0_tris: Vec<usize> = Vec::new();

    // Основная плоскость (синяя заливка)
    add_quad_tri(
        &mut scene, &mut plane0_tris,
        [(1.0, 1.0), (2.0, 19.0), (18.0, 19.0), (17.0, 1.0)],
        1.5,
        OBJ_PLANE_FILL,
    );

    // Красный треугольник
    let v_red0 = scene.add_vertex(Vertex::with_depth(3.0, 6.0, 1.4, ()));
    let v_red1 = scene.add_vertex(Vertex::with_depth(4.0, 15.0, 1.4, ()));
    let v_red2 = scene.add_vertex(Vertex::with_depth(13.0, 14.0, 1.4, ()));
    plane0_tris.push(scene.add_triangle(Triangle {
        id: OBJ_INNER1,
        vertices: [v_red0, v_red1, v_red2],
    }));

    // Зелёный треугольник
    let v_grn0 = scene.add_vertex(Vertex::with_depth(6.0, 4.0, 1.3, ()));
    let v_grn1 = scene.add_vertex(Vertex::with_depth(13.0, 4.0, 1.3, ()));
    let v_grn2 = scene.add_vertex(Vertex::with_depth(11.0, 16.0, 1.3, ()));
    plane0_tris.push(scene.add_triangle(Triangle {
        id: OBJ_INNER2,
        vertices: [v_grn0, v_grn1, v_grn2],
    }));

    // Обводка основной плоскости.
    // Толщина в координатах плоскости. 0.2 × 20 px/unit ≈ 4 px в левой панели.
    let outline_thickness = 0.2;
    add_outline(
        &mut scene, &mut plane0_tris,
        &[(1.0, 1.0), (2.0, 19.0), (18.0, 19.0), (17.0, 1.0)],
        outline_thickness,
        OBJ_PLANE_OUTLINE,
    );

    let plane0_idx = scene.add_plane(Plane { triangles: plane0_tris });

    // ---------- Plane 1: аннотации (два прямоугольника-обводки) ----------
    // Их видит только view 0 (левая панель), viewports 1/2 их не захватывают.
    let mut plane1_tris: Vec<usize> = Vec::new();

    add_outline(
        &mut scene, &mut plane1_tris,
        &[(3.0, 6.0), (3.0, 8.0), (6.0, 8.0), (6.0, 6.0)],
        outline_thickness,
        OBJ_RECT1,
    );
    add_outline(
        &mut scene, &mut plane1_tris,
        &[(7.0, 7.0), (7.0, 9.0), (10.0, 9.0), (10.0, 7.0)],
        outline_thickness,
        OBJ_RECT2,
    );

    let plane1_idx = scene.add_plane(Plane { triangles: plane1_tris });

    // ---------- Виды и порты ----------
    let mut config = Configuration::new(W, H);

    // View 0: вся сцена (с поворотом, как в оригинале)
    let view0 = config.add_view(View::new(20.0, 0.0, -20.0, 20.0).with_rotation(3.1415));
    // View 1: маленький фрагмент в правой панели
    let view1 = config.add_view(View::new(10.05, 6.85, -2.95, 1.95).with_rotation(3.1415));
    // View 2: другой фрагмент, перекрывается с view 1
    let view2 = config.add_view(View::new(6.05, 5.85, -2.95, 1.95).with_rotation(3.1415));

    // Port 0: левая панель 400×400
    let port0 = config.add_port(
        Port::new(ScalingMode::Contain)
            .with_buffer_size(400, 400),
    );
    // Port 1: правая панель 400×300 со смещением
    let port1 = config.add_port(
        Port::new(ScalingMode::Stretch)
            .with_buffer_offset(390, 50)
            .with_buffer_size(400, 300),
    );
    // Port 2: тот же прямоугольник, что и port 1 — outputs накладываются
    let port2 = config.add_port(
        Port::new(ScalingMode::Stretch)
            .with_buffer_offset(390, 50)
            .with_buffer_size(400, 300),
    );

    // ---------- Слои ----------
    // Слой 0: реальная сцена — рисуется во все три порта.
    config.add_layer(Layer::new(
        plane0_idx,
        vec![(view0, port0), (view1, port1), (view2, port2)],
        1
    ));
    // Слой 1: аннотации — только в левую панель.
    config.add_layer(Layer::new(plane1_idx, vec![(view0, port0)], 1));

    // ---------- Дискретизация ----------
    let task = Task::new(scene, config);
    let buffer = discretize(&task);

    // ---------- Интерпретация ----------
    let bg = [240u8, 240u8, 240u8, 255u8];
    let palette = [
        [130u8, 130u8, 230u8, 255u8], // OBJ_PLANE_FILL
        [ 90u8,  90u8, 190u8, 255u8], // OBJ_PLANE_OUTLINE
        [200u8,  50u8,  50u8, 255u8], // OBJ_INNER1
        [140u8, 230u8, 140u8, 255u8], // OBJ_INNER2
        [ 90u8, 190u8, 190u8, 255u8], // OBJ_RECT1 (cyan)
        [ 60u8,  60u8,  60u8, 255u8], // OBJ_RECT2 (dark grey)
    ];

    let mut img = ImageBuffer::new(W as u32, H as u32);
    for py in 0..H {
        for px in 0..W {
            // Идём от верхнего слоя к нижнему, берём первый непустой.
            let color = (0..buffer.layers)
                .rev()
                .find_map(|l| {
                    buffer
                        .stack(px, py, l)
                        .and_then(|s| s.first())
                        .map(|e| palette[e.object_id as usize])
                })
                .unwrap_or(bg);
            img.put_pixel(px as u32, py as u32, Rgba(color));
        }
    }

    // ---------- Текст ----------
    let font_data = include_bytes!("../assets/DejaVuSans.ttf");
    let font = FontRef::try_from_slice(font_data).expect("Error loading font");
    let scale = PxScale { x: 20.0, y: 20.0 };
    let black = Rgba([0, 0, 0, 255]);

    draw_text_mut(&mut img, black, 50, 25, scale, &font, "Plane");
    draw_text_mut(&mut img, black, 400, 30, scale, &font, "Views overlap");

    img.save("multiple_viewports.png").expect("Failed to save PNG");
    println!("Saved multiple_viewports.png");
}