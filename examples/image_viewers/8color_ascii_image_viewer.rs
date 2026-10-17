use discretizer::*;
use image::{open, RgbaImage};
use std::{env};
use terminal_size::terminal_size;

// 8 базовых цветов
const ANSI_PALETTE_8: [[u8; 3]; 8] = [
    [0, 0, 0],       // 0 Black
    [128, 0, 0],     // 1 Red
    [0, 128, 0],     // 2 Green
    [128, 128, 0],   // 3 Yellow
    [0, 0, 128],     // 4 Blue
    [128, 0, 128],   // 5 Magenta
    [0, 128, 128],   // 6 Cyan
    [192, 192, 192], // 7 White
];

const BAYER: [[f32; 4]; 4] = [
    [0.0, 8.0, 2.0, 10.0],
    [12.0, 4.0, 14.0, 6.0],
    [3.0, 11.0, 1.0, 9.0],
    [15.0, 7.0, 13.0, 5.0],
];

// Используемые цвета
const VARIANTS: [(char, f32, bool); 5] = [
    (' ', 0.0, false),          // 0% текста, 100% фона
    ('\u{2591}', 0.25, false),  // 25% текста, 75% фона (░)
    ('\u{2592}', 0.5, false),   // 50% текста, 50% фона (▒)
    ('\u{2591}', 0.25, true),   // 75% текста, 25% фона (инвертированный ░, так как ▓ у меня не работал)
    ('\u{2588}', 1.0, false),   // 100% текста, 0% фона (█)
];

// ---- Функции для ANSI-кодов (только 8 цветов) ----
fn ansi_fg(index: u8) -> String {
    format!("\x1b[{}m", 30 + index)
}

fn ansi_bg(index: u8) -> String {
    format!("\x1b[{}m", 40 + index)
}

// ---- Поиск оптимальной комбинации (только 8 цветов) ----
fn find_best_combination_8(target: [u8; 3]) -> (u8, u8, char) {
    let mut best_bg = 0;
    let mut best_fg = 0;
    let mut best_ch = ' ';
    let mut best_dist = f32::MAX;

    for bg in 0u8..8 {
        for fg in 0..8 {
            for (ch, ratio, swap) in VARIANTS {
                let (fg_actual, bg_actual) = if swap {
                    (bg, fg)
                } else {
                    (fg, bg)
                };

                let fg_color = ANSI_PALETTE_8[fg_actual as usize];
                let bg_color = ANSI_PALETTE_8[bg_actual as usize];

                let r = fg_color[0] as f32 * ratio + bg_color[0] as f32 * (1.0 - ratio);
                let g = fg_color[1] as f32 * ratio + bg_color[1] as f32 * (1.0 - ratio);
                let b = fg_color[2] as f32 * ratio + bg_color[2] as f32 * (1.0 - ratio);

                let d = (r - target[0] as f32).powi(2)
                      + (g - target[1] as f32).powi(2)
                      + (b - target[2] as f32).powi(2);

                if d < best_dist {
                    best_dist = d;
                    best_fg = fg_actual;
                    best_bg = bg_actual;
                    best_ch = ch;
                }
            }
        }
    }

    (best_fg, best_bg, best_ch)
}

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

// Вычисляет смещение для центрирования вьюпорта в буфере с учётом соотношения сторон элемента
fn offset_to_center(buffer_width: u32, buffer_height: u32, vp_width: f32, vp_height: f32, aspect: f32) -> (u32, u32) {
    let out_w = buffer_width as f32;
    let out_h = buffer_height as f32;
    let abs_w = vp_width.abs();
    let abs_h = vp_height.abs() * aspect;

    let out_aspect = out_w / out_h;
    let vp_aspect = abs_w / abs_h;

    let scale_w: f32;
    let scale_h: f32;
    if vp_aspect > out_aspect { 
        let scale = out_w / abs_w;
        (scale_w, scale_h) = (scale, scale);
    } else {
        let scale = out_h / abs_h;
        (scale_w, scale_h) = (scale, scale);
    }

    let scaled_viewport_width = abs_w * scale_w;
    let scaled_viewport_height = abs_h * scale_h;

    let offset_x = ((out_w - scaled_viewport_width) / 2.0).ceil() as u32;
    let offset_y = ((out_h - scaled_viewport_height) / 2.0).ceil() as u32;
    (offset_x, offset_y)
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

    // Терминальные символы обычно вдвое выше, чем шире: aspect = 0.5.
    let aspect: f32 = 0.5;

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
        out_w as u32, out_h as u32, width, height, aspect,
    );

    let mut config = Configuration::new(out_w, out_h);

    let view_idx = config.add_view(View::new(0.0, 0.0, width, height));

    let port = Port::new(ScalingMode::Contain)
        .with_aspect_ratio(aspect)
        .with_buffer_offset(offset_x, offset_y);
    let port_idx = config.add_port(port);

    config.add_layer(Layer::new(plane_idx, vec![(view_idx, port_idx)], 1));

    // 6. Задача дискретизации.
    let task = Task::new(scene, config);

    // 7. Дискретизация.
    let buffer = discretize(&task);
    // 8. Вывод в терминал с палитрой 8 цветов и дизерингом
    for y in 0..buffer.height {
        for x in 0..buffer.width {
            let stack = buffer.stack(x, y, 0).unwrap_or(&[]);
            let (r, g, b) = match stack.first() {
                Some(elem) => {
                    let u = elem.attr.u.clamp(0.0, 1.0);
                    let v = elem.attr.v.clamp(0.0, 1.0);
                    let px = (u * (img.width() - 1) as f32) as u32;
                    let py = (v * (img.height() - 1) as f32) as u32;

                    let [r0, g0, b0, _a] = img.get_pixel(px, py).0;

                    // Порог Байера — добавляет шум перед квантованием в 8 цветов.
                    let bayer = BAYER[(y % 4) as usize][(x % 4) as usize] / 16.0;
                    let offset = (bayer - 0.5) * 10.0;

                    let r = (r0 as f32 + offset).clamp(0.0, 255.0) as u8;
                    let g = (g0 as f32 + offset).clamp(0.0, 255.0) as u8;
                    let b = (b0 as f32 + offset).clamp(0.0, 255.0) as u8;
                    (r, g, b)
                }
                None => (0, 0, 0),
            };

            let (fg, bg, ch) = find_best_combination_8([r, g, b]);
            print!("{}{}{}\x1b[0m", ansi_fg(fg), ansi_bg(bg), ch);
        }
        println!();
    }
    print!("\x1b[0m");
}