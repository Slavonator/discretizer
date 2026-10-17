use discretizer::*;
use image::{open, RgbaImage};
use std::env;
use std::io::Write;
use terminal_size::terminal_size;

/// Атрибуты вершины для вьюера: UV-координаты сэмплирования текстуры.
#[derive(Copy, Clone, Debug)]
struct Uv {
    u: f32,
    v: f32,
}

impl Attributes for Uv {
    fn placeholder() -> Self {
        Uv { u: 0.0, v: 0.0 }
    }

    fn bary(a: &Self, b: &Self, c: &Self, w: [f32; 3]) -> Self {
        Uv {
            u: a.u * w[0] + b.u * w[1] + c.u * w[2],
            v: a.v * w[0] + b.v * w[1] + c.v * w[2],
        }
    }
}

/// Считает оффсет, центрирующий вид в порту при заданном аспекте дискреты.
fn offset_to_center(
    buffer_width: u32,
    buffer_height: u32,
    view_width: f32,
    view_height: f32,
    aspect: f32,
) -> (u32, u32) {
    let out_w = buffer_width as f32;
    let out_h = buffer_height as f32;
    let abs_w = view_width.abs();
    let abs_h = view_height.abs() * aspect;

    let out_aspect = out_w / out_h;
    let vp_aspect = abs_w / abs_h;

    let scale = if vp_aspect > out_aspect {
        out_w / abs_w
    } else {
        out_h / abs_h
    };

    let scaled_w = abs_w * scale;
    let scaled_h = abs_h * scale;

    let offset_x = ((out_w - scaled_w) / 2.0).ceil() as u32;
    let offset_y = ((out_h - scaled_h) / 2.0).ceil() as u32;
    (offset_x, offset_y)
}

/// Возвращает RGB-цвет текстуры для клетки буфера `(x, y)` слоя 0.
fn sample_color(buffer: &ElementBuffer<Uv>, img: &RgbaImage, x: usize, y: usize) -> (u8, u8, u8) {
    let stack = buffer.stack(x, y, 0).unwrap_or(&[]);
    match stack.first() {
        Some(elem) => {
            let u = elem.attr.u.clamp(0.0, 1.0);
            let v = elem.attr.v.clamp(0.0, 1.0);
            let px = (u * (img.width() - 1) as f32) as u32;
            let py = (v * (img.height() - 1) as f32) as u32;
            let [r, g, b, _a] = img.get_pixel(px, py).0;
            (r, g, b)
        }
        None => (0, 0, 0),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: viewer [filename]");
        return;
    }
    let filename = &args[1];
    println!("Loading: '{}'", filename);
    let img: RgbaImage = open(filename).expect("Failed to open image").into_rgba8();
    println!("Image loaded: {}x{}", img.width(), img.height());

    let width = img.width() as f32;
    let height = img.height() as f32;
    let (out_w, out_h) = terminal_size().expect("Unable to get output size");
    let out_w = out_w.0 as usize;
    let out_h = out_h.0 as usize;

    // Каждая ячейка терминала вмещает 2 субпикселя по вертикали.
    let buf_w = out_w;
    let buf_h = out_h * 2;

    // Полу-блок `▀` примерно квадратный => aspect = 1.0.
    let aspect: f32 = 1.0;

    // 1. Сцена с UV-атрибутами.
    let mut scene: Scene<Uv> = Scene::new();

    // 2. Вершины прямоугольника изображения.
    let v0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, Uv { u: 0.0, v: 0.0 }));
    let v1 = scene.add_vertex(Vertex::with_depth(width, 0.0, 0.0, Uv { u: 1.0, v: 0.0 }));
    let v2 = scene.add_vertex(Vertex::with_depth(width, height, 0.0, Uv { u: 1.0, v: 1.0 }));
    let v3 = scene.add_vertex(Vertex::with_depth(0.0, height, 0.0, Uv { u: 0.0, v: 1.0 }));

    // 3. Два треугольника, образующие прямоугольник.
    let t1 = scene.add_triangle(Triangle { id: 1, vertices: [v0, v1, v2] });
    let t2 = scene.add_triangle(Triangle { id: 2, vertices: [v0, v2, v3] });

    // 4. Плоскость, ссылающаяся на оба треугольника.
    let plane_idx = scene.add_plane(Plane { triangles: vec![t1, t2] });

    // 5. Конфигурация вывода.
    let (offset_x, offset_y) = offset_to_center(
        buf_w as u32, buf_h as u32, width, height, aspect,
    );

    let mut config = Configuration::new(buf_w, buf_h);

    let view_idx = config.add_view(View::new(0.0, 0.0, width, height));

    let port = Port::new(ScalingMode::Contain)
        .with_aspect_ratio(aspect)
        .with_buffer_offset(offset_x, offset_y);
    let port_idx = config.add_port(port);

    config.add_layer(Layer::new(plane_idx, vec![(view_idx, port_idx)], 1));

    // 6. Задача дискретизации.
    let task = Task::new(scene, config);

    // 7. Дискретизация.
    let buffer: ElementBuffer<Uv> = discretize(&task);

    // 8. Вывод: на одну ячейку терминала — 2 субпикселя.
    //    `▀` закрашивает верхнюю половину цветом fg, нижняя — цвет фона.
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());

    for cy in 0..out_h {
        let y_top = cy * 2;
        let y_bot = cy * 2 + 1;
        for x in 0..out_w {
            let (tr, tg, tb) = sample_color(&buffer, &img, x, y_top);
            let (br, bg, bb) = sample_color(&buffer, &img, x, y_bot);
            write!(
                out,
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                tr, tg, tb, br, bg, bb
            )
            .unwrap();
        }
        writeln!(out, "\x1b[0m").unwrap();
    }
}