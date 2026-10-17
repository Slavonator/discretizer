use glam::{Vec2, Vec3, Quat, Mat4, camera::rh::proj::opengl::perspective};

/// Сцена: арена узлов, мешей, камер и источников света.
///
/// Связи между узлами — индексы. Меш и камера на узле — тоже индексы.
/// Один и тот же меш может быть использован несколькими узлами (инстансинг).
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub nodes: Vec<Node>,
    pub roots: Vec<usize>,
    pub meshes: Vec<Mesh>,
    pub cameras: Vec<Camera>,
}

impl Scene {
    pub fn new() -> Self { Self::default() }

    // ---------- Узлы ----------

    pub fn add_node(&mut self, node: Node) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        self.roots.push(idx);
        idx
    }

    /// Прикрепляет `child` к `parent`. Убирает `child` из корней
    /// и от предыдущего родителя, если он был.
    pub fn add_child(&mut self, parent: usize, child: usize) {
        assert_ne!(parent, child, "node cannot be its own parent");
        assert!(parent < self.nodes.len(), "parent index out of bounds");
        assert!(child < self.nodes.len(), "child index out of bounds");

        // Из корней
        if let Some(pos) = self.roots.iter().position(|&r| r == child) {
            self.roots.remove(pos);
        }
        // От старого родителя
        if let Some(old) = self.nodes[child].parent {
            if let Some(pos) = self.nodes[old].children.iter().position(|&c| c == child) {
                self.nodes[old].children.remove(pos);
            }
        }
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
    }

    pub fn node(&self, idx: usize) -> &Node { &self.nodes[idx] }
    pub fn node_mut(&mut self, idx: usize) -> &mut Node { &mut self.nodes[idx] }

    // ---------- Меши / камеры / свет ----------

    pub fn add_mesh(&mut self, mesh: Mesh) -> usize {
        let idx = self.meshes.len();
        self.meshes.push(mesh);
        idx
    }

    pub fn add_camera(&mut self, camera: Camera) -> usize {
        let idx = self.cameras.len();
        self.cameras.push(camera);
        idx
    }

    // ---------- Мировые матрицы ----------

    /// Считает мировую матрицу каждого узла. Результат индексируется
    /// индексом узла. Для оторванных от корней узлов — `Mat4::IDENTITY`.
    pub fn compute_world_matrices(&self) -> Vec<Mat4> {
        let mut out = vec![Mat4::IDENTITY; self.nodes.len()];
        for &root in &self.roots {
            self.compute_world_recursive(root, Mat4::IDENTITY, &mut out);
        }
        out
    }

    fn compute_world_recursive(&self, idx: usize, parent: Mat4, out: &mut [Mat4]) {
        let world = parent * self.nodes[idx].transform.to_matrix();
        out[idx] = world;
        for &child in &self.nodes[idx].children {
            self.compute_world_recursive(child, world, out);
        }
    }

    /// Итеративный вариант без рекурсии — на случай глубоких деревьев.
    pub fn compute_world_matrices_iter(&self) -> Vec<Mat4> {
        let mut out = vec![Mat4::IDENTITY; self.nodes.len()];
        let mut stack: Vec<(usize, Mat4)> =
            self.roots.iter().map(|&r| (r, Mat4::IDENTITY)).collect();

        while let Some((idx, parent)) = stack.pop() {
            let world = parent * self.nodes[idx].transform.to_matrix();
            out[idx] = world;
            for &child in &self.nodes[idx].children {
                stack.push((child, world));
            }
        }
        out
    }
}


/// Вершина 3D-меша.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

impl Vertex {
    pub fn new(position: Vec3) -> Self {
        Self { position, normal: Vec3::ZERO, uv: Vec2::ZERO }
    }

    pub fn with_normal(mut self, n: Vec3) -> Self { self.normal = n; self }
    pub fn with_uv(mut self, uv: Vec2) -> Self { self.uv = uv; self }
}

/// Геометрический примитив: вершины, индексы, материал.
///
/// Индексы — тройки вершин, каждые три подряд образуют треугольник.
#[derive(Clone, Debug)]
pub struct Primitive {
    pub name: Option<String>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,

    /// Цвет материала, RGB.
    pub color: [u8; 3],
    /// Альфа: 0 — прозрачный, 255 — непрозрачный.
    pub alpha: u8,
    /// Если `true` — треугольники рисуются с обеих сторон.
    pub double_sided: bool,

    /// Локальный центр ограничивающей сферы.
    pub local_center: Vec3,
    /// Радиус ограничивающей сферы.
    pub bounding_radius: f32,
}

impl Primitive {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u32>) -> Self {
        assert_eq!(indices.len() % 3, 0, "indices length must be a multiple of 3");
        let (local_center, bounding_radius) = compute_bounding_sphere(&vertices);
        Self {
            name: None,
            vertices,
            indices,
            color: [200, 200, 200],
            alpha: 255,
            double_sided: false,
            local_center,
            bounding_radius,
        }
    }

    pub fn triangle_count(&self) -> usize { self.indices.len() / 3 }

    pub fn triangle(&self, i: usize) -> [u32; 3] {
        [self.indices[i * 3], self.indices[i * 3 + 1], self.indices[i * 3 + 2]]
    }

    pub fn recompute_bounds(&mut self) {
        let (c, r) = compute_bounding_sphere(&self.vertices);
        self.local_center = c;
        self.bounding_radius = r;
    }
}

fn compute_bounding_sphere(vertices: &[Vertex]) -> (Vec3, f32) {
    if vertices.is_empty() {
        return (Vec3::ZERO, 0.0);
    }
    let mut center = Vec3::ZERO;
    for v in vertices { center += v.position; }
    center /= vertices.len() as f32;

    let mut max_sq = 0.0f32;
    for v in vertices {
        let l = (v.position - center).length_squared();
        if l > max_sq { max_sq = l; }
    }
    (center, max_sq.sqrt())
}

/// Меш — контейнер примитивов. Может быть инстанциирован несколькими узлами.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
}

impl Mesh {
    pub fn new() -> Self { Self::default() }

    pub fn from_primitive(p: Primitive) -> Self {
        Self { name: None, primitives: vec![p] }
    }

    /// Куб со стороной `2 * half_size`, центрированный в начале координат.
    /// 24 вершины (по 4 на грань) — чтобы нормали не размазывались.
    /// Обход вершин CCW при взгляде снаружи.
    pub fn cube(half_size: f32) -> Self {
        let h = half_size;

        // Внутренний хелпер: строит 4 вершины грани.
        //   v0 = c - T1*h - T2*h
        //   v1 = c + T1*h - T2*h
        //   v2 = c + T1*h + T2*h
        //   v3 = c - T1*h + T2*h
        // где T1 × T2 = outward normal.
        // Треугольники: [0, 1, 2], [0, 2, 3].
        fn face(
            c: Vec3,
            t1: Vec3,
            t2: Vec3,
            n: Vec3,
            h: f32,
            verts: &mut Vec<Vertex>,
            indices: &mut Vec<u32>,
        ) {
            let base = verts.len() as u32;
            let mk = |u: f32, v: f32| {
                let pos = c + t1 * (u * h) + t2 * (v * h);
                Vertex::new(pos).with_normal(n).with_uv(Vec2::new((u + 1.0) * 0.5, (v + 1.0) * 0.5))
            };
            verts.push(mk(-1.0, -1.0));
            verts.push(mk( 1.0, -1.0));
            verts.push(mk( 1.0,  1.0));
            verts.push(mk(-1.0,  1.0));
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let mut verts = Vec::with_capacity(24);
        let mut idx = Vec::with_capacity(36);

        // +X: T1 = +Y, T2 = +Z   (Y × Z = X)
        face(Vec3::new( h, 0.0, 0.0), Vec3::Y, Vec3::Z,  Vec3::X, h, &mut verts, &mut idx);
        // -X: T1 = +Z, T2 = +Y   (Z × Y = -X)
        face(Vec3::new(-h, 0.0, 0.0), Vec3::Z, Vec3::Y, -Vec3::X, h, &mut verts, &mut idx);
        // +Y: T1 = +Z, T2 = +X   (Z × X = Y)
        face(Vec3::new(0.0,  h, 0.0), Vec3::Z, Vec3::X,  Vec3::Y, h, &mut verts, &mut idx);
        // -Y: T1 = +X, T2 = +Z   (X × Z = -Y)
        face(Vec3::new(0.0, -h, 0.0), Vec3::X, Vec3::Z, -Vec3::Y, h, &mut verts, &mut idx);
        // +Z: T1 = +X, T2 = +Y   (X × Y = Z)
        face(Vec3::new(0.0, 0.0,  h), Vec3::X, Vec3::Y,  Vec3::Z, h, &mut verts, &mut idx);
        // -Z: T1 = +Y, T2 = +X   (Y × X = -Z)
        face(Vec3::new(0.0, 0.0, -h), Vec3::Y, Vec3::X, -Vec3::Z, h, &mut verts, &mut idx);

        let prim = Primitive::new(verts, idx);
        Self { name: Some("Cube".into()), primitives: vec![prim] }
    }
}

/// Узел графа сцены.
#[derive(Clone, Debug, Default)]
pub struct Node {
    pub name: Option<String>,
    pub transform: Transform,

    pub parent: Option<usize>,
    pub children: Vec<usize>,

    /// Индекс меша в `Scene::meshes`.
    pub mesh: Option<usize>,
    /// Индекс камеры в `Scene::cameras`.
    pub camera: Option<usize>,
}

impl Node {
    pub fn new() -> Self { Self::default() }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_transform(mut self, t: Transform) -> Self {
        self.transform = t;
        self
    }

    pub fn with_mesh(mut self, mesh_idx: usize) -> Self {
        self.mesh = Some(mesh_idx);
        self
    }

    pub fn with_camera(mut self, cam_idx: usize) -> Self {
        self.camera = Some(cam_idx);
        self
    }
}

/// TRS-трансформация узла.
///
/// Порядок применения к точке: сначала `scale`, потом `rotation`,
/// потом `translation`. Матрица: `T * R * S`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub fn new() -> Self { Self::IDENTITY }

    pub fn from_translation(t: Vec3) -> Self {
        Self { translation: t, ..Self::IDENTITY }
    }

    pub fn from_rotation(r: Quat) -> Self {
        Self { rotation: r, ..Self::IDENTITY }
    }

    pub fn from_scale(s: Vec3) -> Self {
        Self { scale: s, ..Self::IDENTITY }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_translation(self.translation)
            * Mat4::from_quat(self.rotation)
            * Mat4::from_scale(self.scale)
    }
}

impl Default for Transform {
    fn default() -> Self { Self::IDENTITY }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn identity_is_identity() {
        assert_eq!(Transform::IDENTITY.to_matrix(), Mat4::IDENTITY);
    }

    #[test]
    fn order_is_scale_then_rotate_then_translate() {
        let t = Transform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            rotation: Quat::from_axis_angle(Vec3::Z, FRAC_PI_2),
            scale: Vec3::new(2.0, 2.0, 2.0),
        };
        // Точка (1, 0, 0):
        //   scale   -> (2, 0, 0)
        //   rotate  -> (0, 2, 0)
        //   translate -> (10, 2, 0)
        let p = t.to_matrix().transform_point3(Vec3::new(1.0, 0.0, 0.0));
        assert!((p - Vec3::new(10.0, 2.0, 0.0)).length() < 1e-5, "{p:?}");
    }
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub name: Option<String>,
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn new(fov_y: f32, near: f32, far: f32) -> Self {
        Self { name: None, fov_y, near, far }
    }

    pub fn default_perspective() -> Self {
        Self::new(std::f32::consts::FRAC_PI_3, 0.1, 100.0)
    }

    /// Перспективная проекция в NDC, OpenGL-стиль.
    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        perspective(self.fov_y, aspect, self.near, self.far)
    }

    /// View-матрица из мировой матрицы узла-камеры.
    ///
    /// Предполагается, что узел смотрит вдоль локальной `-Z` —
    /// та же конвенция, что в OpenGL, glTF и твоём C#-коде.
    /// View = inverse(world), потому что мы переходим из мира в систему камеры.
    pub fn view_matrix(camera_world: Mat4) -> Mat4 {
        camera_world.inverse()
    }

    pub fn validate(&self) {
        assert!(self.fov_y > 0.0, "fov_y must be positive");
        assert!(self.near > 0.0, "near must be positive");
        assert!(self.far > self.near, "far must be greater than near");
    }
}

impl Default for Camera {
    fn default() -> Self { Self::default_perspective() }
}


