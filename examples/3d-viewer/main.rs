pub mod input;
pub mod orbit_camera;
pub mod interpreter;
pub mod projector;

use std::f32::consts::PI;
use std::thread::sleep;
use std::time::Duration;

use crossterm::event::{MouseButton, MouseEventKind};
use glam::{Vec3, Vec2};
use tobj;

use discretizer as ds;
use crate::projector::{Projector, CameraContext, scene::{Camera, Node, Transform, Vertex}};
use crate::interpreter::AsciiInterpreter;
use crate::input::{Event, Input, KeyCode, KeyEventKind, KeyModifiers, Screen};
use crate::orbit_camera::OrbitCamera;

fn load_scene(path: &str) -> crate::projector::scene::Scene {
    use crate::projector::scene::{Mesh, Node, Primitive, Scene, Vertex};

    let (models, _) = tobj::load_obj(path, &tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    })
    .expect("Failed to load OBJ");

    let mut scene = Scene::new();
    for model in models {
        let m = &model.mesh;
        let n = m.positions.len() / 3;
        let mut vertices = Vec::with_capacity(n);
        for i in 0..n {
            let pos = Vec3::new(m.positions[i * 3], m.positions[i * 3 + 1], m.positions[i * 3 + 2]);
            let normal = if !m.normals.is_empty() {
                Vec3::new(m.normals[i * 3], m.normals[i * 3 + 1], m.normals[i * 3 + 2])
            } else {
                Vec3::ZERO
            };
            let uv = if !m.texcoords.is_empty() {
                Vec2::new(m.texcoords[i * 2], m.texcoords[i * 2 + 1])
            } else {
                Vec2::ZERO
            };
            vertices.push(Vertex::new(pos).with_normal(normal).with_uv(uv));
        }

        let prim = Primitive::new(vertices, m.indices.clone());
        let mesh = Mesh { name: None, primitives: vec![prim] };
        let mesh_idx = scene.add_mesh(mesh);

        // model.name ещё не тронут — используем его только тут
        scene.add_node(
            Node::new()
                .with_name(model.name)
                .with_mesh(mesh_idx),
        );
    }
    scene
}

#[derive(Copy, Clone, Debug)]
struct Attrs { normal: Vec3 }

impl ds::Attributes for Attrs {
    fn placeholder() -> Self {
        Attrs { normal: Vec3::Z }
    }

    fn bary(a: &Self, b: &Self, c: &Self, w: [f32; 3]) -> Self {
        Attrs {
            normal: Vec3::new(
                a.normal.x * w[0] + b.normal.x * w[1] + c.normal.x * w[2],
                a.normal.y * w[0] + b.normal.y * w[1] + c.normal.y * w[2],
                a.normal.z * w[0] + b.normal.z * w[1] + c.normal.z * w[2],
            ).normalize(),
        }
    }
}

struct MouseState {
    last_row: u16,
    last_col: u16
}

impl MouseState {

    fn new() -> Self {
        Self { 
            last_row: 0, 
            last_col: 0 
        }
    }

}

fn build_attrs(_v: &Vertex, _wp: Vec3, wn: Vec3) -> Attrs {
    Attrs { normal: wn }
}

fn lerp_attrs(a: &Attrs, b: &Attrs, t: f32) -> Attrs {
    Attrs {
        normal: a.normal.lerp(b.normal, t).normalize(),
    }
}

fn shade(a: &Attrs) -> f32 {
    let light = Vec3::new(1.0, 1.0, 1.0).normalize();
    let diff = a.normal.dot(light).max(0.0);
    (0.2 + 0.8 * diff).clamp(0.0, 1.0)
}

const SENSITIVITY: f32 = 0.01;
const ZOOM_STEP: f32 = 0.5;

fn main() -> std::io::Result<()> {
    // --- 1. Аргументы ---
    let args: Vec<String> = std::env::args().collect();
    let path = match args.get(1) {
        Some(p) => p,
        None => {
            eprintln!("Usage: viewer-3d <model.obj>");
            return Ok(());
        }
    };

    // --- 2. Размер терминала ---
    let (out_w, out_h) = terminal_size::terminal_size()
        .map(|(w, h)| (w.0 as usize, h.0 as usize))
        .unwrap_or((80, 24));

    // --- 3. Сцена: модель + камера ---
    let mut scene = load_scene(path);

    // Найти центр и радиус модели, чтобы поставить камеру осмысленно
    let mesh_idx = scene.nodes[0].mesh.expect("model node has no mesh");
    let prim = &scene.meshes[mesh_idx].primitives[0];
    let initial_world_matrices = scene.compute_world_matrices();
    let world_center = initial_world_matrices[0].transform_point3(prim.local_center);
    let radius = prim.bounding_radius;

    let cam_idx = scene.add_camera(Camera::new(PI / 3.0, 0.1, radius * 1000.0));
    let cam_node = scene.add_node(
        Node::new()
            .with_name("camera")
            .with_camera(cam_idx)
            .with_transform(Transform::from_translation(
                world_center + Vec3::new(0.0, 0.0, radius * 2.5)
            )),
    );
    let mut camera = OrbitCamera::new(cam_node, world_center, radius * 2.5);

    // --- 4. Конфигурация плоскости и буфера ---
    let aspect_ratio = 0.5;
    let plane_h = out_h as f32;
    let plane_w = out_w as f32 * aspect_ratio;
    let aspect = plane_w / plane_h; // Камера теперь знает реальные пропорции!

    // --- 5. Интерпретатор ---
    let renderer = AsciiInterpreter::new()
        .with_ramp(" .:~=+*#%@");
    // --- 6. Ввод и вывод ---
    let input = Input::new()?;
    let mut screen = Screen::new();
    screen.enter()?;

    // --- 7. Главный цикл ---
    let dt = Duration::from_millis(33);
    let mut running = true;
    let mut mouse_state = MouseState::new();

    while running {
        // 7a. События
        while let Some(event) = input.poll() {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => running = false,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            running = false;
                        }
                        _ => {}
                    }
                }
            }
            if let Event::Mouse(mouse_event) = event {
                match mouse_event.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        mouse_state.last_col = mouse_event.column;
                        mouse_state.last_row = mouse_event.row
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        
                        let horisontal_move: i32 =  mouse_event.column as i32 - mouse_state.last_col as i32;
                        let vertical_move: i32 = mouse_event.row as i32 - mouse_state.last_row as i32;
                        mouse_state.last_col = mouse_event.column;
                        mouse_state.last_row = mouse_event.row;
                        camera.increase_yaw(horisontal_move as f32 * SENSITIVITY);
                        camera.decrease_pitch(vertical_move as f32 * SENSITIVITY);
                    }
                    MouseEventKind::ScrollUp => {
                        camera.zoom_in(ZOOM_STEP);
                    }
                    MouseEventKind::ScrollDown => {
                        camera.zoom_out(ZOOM_STEP);
                    }
                    _ => {}
                }

            }
        }

        // 7b. Пайплайн: сцена → discretizer::Scene → буфер
        camera.apply(&mut scene);
        let world_matrices = scene.compute_world_matrices();
        let ctx = CameraContext::from_node(&scene, cam_node, aspect);
        let projector = Projector::new(ctx, plane_w, plane_h);

        let mut ds_scene = ds::Scene::<Attrs>::new();
        let plane = projector.project_scene(
            &mut ds_scene,
            &scene,
            &world_matrices,
            build_attrs,
            lerp_attrs,
        );
        let plane_idx = ds_scene.add_plane(plane);
        let mut config = ds::Configuration::new(out_w, out_h);
        let view_idx = config.add_view(ds::View::new(0.0, 0.0, plane_w, plane_h));
        let port = ds::Port::new(ds::ScalingMode::Contain).with_aspect_ratio(0.5);
            
        let port_idx = config.add_port(port);
        config.add_layer(ds::Layer::new(plane_idx, vec![(view_idx, port_idx)], 1));

        let task = ds::Task::new(ds_scene, config);
        let buffer = ds::discretize(&task);

        // 7c. Отрисовка
        screen.resize(buffer.width, buffer.height);
        screen.clear_back(' ');
        renderer.fill(screen.back_mut(), buffer.width, buffer.height, &buffer, shade);
        screen.flush()?;

        sleep(dt);
    }

    Ok(())
}