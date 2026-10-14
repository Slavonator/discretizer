use overwhelement::*;
use std::f32::consts::PI;
use std::thread::sleep;
use std::time::Duration;
use terminal_size::terminal_size;

// ---------- Атрибуты вершины ----------

#[derive(Copy, Clone, Debug)]
struct Attrs {
    uv: [f32; 2],
    normal: [f32; 3],
}

impl Attributes for Attrs {
    fn placeholder() -> Self {
        Attrs { uv: [0.0; 2], normal: [0.0; 3] }
    }
    fn bary(a: &Self, b: &Self, c: &Self, w: [f32; 3]) -> Self {
        Attrs {
            uv: [
                a.uv[0] * w[0] + b.uv[0] * w[1] + c.uv[0] * w[2],
                a.uv[1] * w[0] + b.uv[1] * w[1] + c.uv[1] * w[2],
            ],
            normal: [
                a.normal[0] * w[0] + b.normal[0] * w[1] + c.normal[0] * w[2],
                a.normal[1] * w[0] + b.normal[1] * w[1] + c.normal[1] * w[2],
                a.normal[2] * w[0] + b.normal[2] * w[1] + c.normal[2] * w[2],
            ],
        }
    }
}

// ---------- Вектор и матрица ----------

#[derive(Copy, Clone)]
struct Vec3 { x: f32, y: f32, z: f32 }

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
    fn dot(self, o: Self) -> f32 { self.x * o.x + self.y * o.y + self.z * o.z }
    fn normalize(self) -> Self {
        let l = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        Self::new(self.x / l, self.y / l, self.z / l)
    }
}

#[derive(Copy, Clone)]
struct Mat4 { m: [f32; 16] } // row-major

impl Mat4 {
    fn mul(self, o: Mat4) -> Mat4 {
        let mut r = [0.0; 16];
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += self.m[i * 4 + k] * o.m[k * 4 + j];
                }
                r[i * 4 + j] = s;
            }
        }
        Mat4 { m: r }
    }

    fn transform(self, v: [f32; 4]) -> [f32; 4] {
        let mut r = [0.0; 4];
        for i in 0..4 {
            r[i] = self.m[i * 4] * v[0]
                 + self.m[i * 4 + 1] * v[1]
                 + self.m[i * 4 + 2] * v[2]
                 + self.m[i * 4 + 3] * v[3];
        }
        r
    }

    fn rotation_x(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        Mat4 { m: [
            1.0, 0.0, 0.0, 0.0,
            0.0, c, -s, 0.0,
            0.0, s,  c, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ]}
    }

    fn rotation_y(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        Mat4 { m: [
             c,  0.0, s, 0.0,
             0.0, 1.0, 0.0, 0.0,
            -s,  0.0, c, 0.0,
             0.0, 0.0, 0.0, 1.0,
        ]}
    }

    fn translation(x: f32, y: f32, z: f32) -> Mat4 {
        Mat4 { m: [
            1.0, 0.0, 0.0, x,
            0.0, 1.0, 0.0, y,
            0.0, 0.0, 1.0, z,
            0.0, 0.0, 0.0, 1.0,
        ]}
    }

    fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y / 2.0).tan();
        Mat4 { m: [
            f / aspect, 0.0, 0.0, 0.0,
            0.0, f, 0.0, 0.0,
            0.0, 0.0, (far + near) / (near - far), (2.0 * far * near) / (near - far),
            0.0, 0.0, -1.0, 0.0,
        ]}
    }
}

fn transform_normal(m: Mat4, v: Vec3) -> Vec3 {
    Vec3::new(
        m.m[0] * v.x + m.m[1] * v.y + m.m[2] * v.z,
        m.m[4] * v.x + m.m[5] * v.y + m.m[6] * v.z,
        m.m[8] * v.x + m.m[9] * v.y + m.m[10] * v.z,
    )
}

/// NDC [-1,1] → координаты плоскости [0, plane_w] × [0, plane_h].
/// Возвращает (x, y, depth), где depth — z_ndc.
fn ndc_to_plane(clip: [f32; 4], plane_w: f32, plane_h: f32) -> [f32; 3] {
    let inv_w = 1.0 / clip[3];
    let x = clip[0] * inv_w;
    let y = clip[1] * inv_w;
    let z = clip[2] * inv_w;
    [
        (x + 1.0) * 0.5 * plane_w,
        (1.0 - y) * 0.5 * plane_h, // Y переворачиваем: NDC «вверх», буфер «вниз»
        z,
    ]
}

// ---------- Провайдер: 3D-куб → Scene<Attrs> ----------

fn build_cube_scene(plane_w: f32, plane_h: f32, angle_x: f32, angle_y: f32) -> Scene<Attrs> {
    let corners: [[f32; 3]; 8] = [
        [-1.0, -1.0, -1.0],
        [ 1.0, -1.0, -1.0],
        [ 1.0,  1.0, -1.0],
        [-1.0,  1.0, -1.0],
        [-1.0, -1.0,  1.0],
        [ 1.0, -1.0,  1.0],
        [ 1.0,  1.0,  1.0],
        [-1.0,  1.0,  1.0],
    ];

    // (a, b, c, d, нормаль в модельном пространстве)
    let faces: [(usize, usize, usize, usize, [f32; 3]); 6] = [
        (0, 3, 2, 1, [ 0.0,  0.0, -1.0]), // -Z
        (4, 5, 6, 7, [ 0.0,  0.0,  1.0]), // +Z
        (0, 4, 7, 3, [-1.0,  0.0,  0.0]), // -X
        (1, 2, 6, 5, [ 1.0,  0.0,  0.0]), // +X
        (0, 1, 5, 4, [ 0.0, -1.0,  0.0]), // -Y
        (3, 7, 6, 2, [ 0.0,  1.0,  0.0]), // +Y
    ];

    let model = Mat4::rotation_y(angle_y).mul(Mat4::rotation_x(angle_x));
    let view = Mat4::translation(0.0, 0.0, -5.0);
    let proj = Mat4::perspective(PI / 3.0, 1.0, 0.1, 100.0);

    let mut scene = Scene::new();
    let mut tri_indices = Vec::with_capacity(12);

    for &(a, b, c, d, n_model) in &faces {
        let n = transform_normal(model, Vec3::new(n_model[0], n_model[1], n_model[2])).normalize();

        let local = [corners[a], corners[b], corners[c], corners[d]];
        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

        let mut vids = [0usize; 4];
        for j in 0..4 {
            let v = local[j];
            let world = model.transform([v[0], v[1], v[2], 1.0]);
            let view_sp = view.transform(world);
            let clip = proj.transform(view_sp);
            let [px, py, pz] = ndc_to_plane(clip, plane_w, plane_h);

            let attr = Attrs { uv: uvs[j], normal: [n.x, n.y, n.z] };
            vids[j] = scene.add_vertex(Vertex::with_depth(px, py, pz, attr));
        }

        tri_indices.push(scene.add_triangle(Triangle { id: 0, vertices: [vids[0], vids[1], vids[2]] }));
        tri_indices.push(scene.add_triangle(Triangle { id: 0, vertices: [vids[0], vids[2], vids[3]] }));
    }

    scene.add_plane(Plane { triangles: tri_indices });
    scene
}

// ---------- Интерпретатор: ASCII с простым освещением ----------

const RAMP: &[u8] = b" .:-=+*#%@";

fn render_ascii(buffer: &ElementBuffer<Attrs>, light: Vec3) {
    // Очистка экрана + курсор в левый верхний угол
    print!("\x1b[2J\x1b[H");

    let mut line = String::with_capacity(buffer.width);
    for y in 0..buffer.height {
        line.clear();
        for x in 0..buffer.width {
            let c = match buffer.stack(x, y, 0).and_then(|s| s.first()) {
                Some(elem) => {
                    let n = Vec3::new(
                        elem.attr.normal[0],
                        elem.attr.normal[1],
                        elem.attr.normal[2],
                    );
                    // Ламберт: max(0, dot(n, light)) + ambient
                    let diff = n.dot(light).max(0.0);
                    let intensity = (0.2 + 0.8 * diff).clamp(0.0, 1.0);
                    let idx = (intensity * (RAMP.len() - 1) as f32).round() as usize;
                    RAMP[idx] as char
                }
                None => ' ',
            };
            line.push(c);
        }
        println!("{}", line);
    }
}

// ---------- main ----------

fn main() {
    let (out_w, out_h) = terminal_size()
        .map(|(w, h)| (w.0 as usize, h.0 as usize))
        .unwrap_or((80, 24));

    let plane_w = 100.0f32;
    let plane_h = 100.0f32;

    let light = Vec3::new(1.0, 1.0, 1.0).normalize();

    let dt = Duration::from_millis(33); // ~30 fps
    let angle_x = 0.5;
    let mut angle_y: f32 = 0.0;

    loop {
        // Провайдер: 3D → Scene<Attrs> в координатах плоскости.
        let scene = build_cube_scene(plane_w, plane_h, angle_x, angle_y);

        // Конфигурация: View на всю плоскость, Port на весь буфер терминала.
        let mut config = Configuration::new(out_w, out_h);
        let view_idx = config.add_view(View::new(0.0, 0.0, plane_w, plane_h));

        let port = Port::new(ScalingMode::Contain).with_aspect_ratio(0.5);
        let port_idx = config.add_port(port);

        config.add_layer(Layer::new(0, vec![(view_idx, port_idx)]));

        let task = Task::new(scene, config);
        let buffer = discretize(&task);

        render_ascii(&buffer, light);

        angle_y += 0.03; // шаг поворота
        if angle_y > 2.0 * PI {
            angle_y -= 2.0 * PI;
        }

        sleep(dt);
    }
}