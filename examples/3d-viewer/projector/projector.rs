use glam::{Mat4, Vec3, Mat3, Vec4};
use super::scene::{Primitive, Scene, Vertex};
use discretizer;

/// Проектор: превращает 3D-примитивы из сцены в набор вершин и треугольников
/// в аренах Overwhelement.
///
/// Живёт один кадр. Хранит контекст камеры и размеры плоскости.
pub struct Projector {
    pub camera: CameraContext,
    /// Ширина плоскости в координатах Overwhelement.
    pub plane_width: f32,
    /// Высота плоскости в координатах Overwhelement.
    pub plane_height: f32,
}

impl Projector {
    pub fn new(camera: CameraContext, plane_width: f32, plane_height: f32) -> Self {
        assert!(plane_width > 0.0 && plane_height > 0.0);
        Self { camera, plane_width, plane_height }
    }

    /// Проецирует один примитив с заданной мировой матрицей.
    ///
    /// Возвращает индексы треугольников, добавленных в `discretizer_scene`.
    /// Их можно сложить в `Plane::triangles`.
    pub fn project_primitive<A, BF, LF>(
        &self,
        discretizer_scene: &mut discretizer::Scene<A>,
        primitive: &Primitive,
        world: Mat4,
        object_id: u32,
        build_attrs: BF,
        lerp_attrs: LF,
    ) -> Vec<usize>
    where
        A: discretizer::Attributes,
        BF: Fn(&Vertex, Vec3, Vec3) -> A,
        LF: Fn(&A, &A, f32) -> A,
    {
        let normal_matrix = Mat3::from_mat4(world).inverse().transpose();
        let mut out = Vec::with_capacity(primitive.triangle_count());

        for tri_idx in 0..primitive.triangle_count() {
            let [i0, i1, i2] = primitive.triangle(tri_idx);
            let v0 = primitive.vertices[i0 as usize];
            let v1 = primitive.vertices[i1 as usize];
            let v2 = primitive.vertices[i2 as usize];

            // Мировые позиции
            let w0 = world.transform_point3(v0.position);
            let w1 = world.transform_point3(v1.position);
            let w2 = world.transform_point3(v2.position);

            // View space
            let vp0 = self.camera.world_to_view(w0);
            let vp1 = self.camera.world_to_view(w1);
            let vp2 = self.camera.world_to_view(w2);

            // Backface culling
            if !primitive.double_sided && !is_front_facing_view(vp0, vp1, vp2) {
                continue;
            }

            // Мировые нормали: через normal_matrix, потом нормализация
            let wn0 = (normal_matrix * v0.normal).normalize_or_zero();
            let wn1 = (normal_matrix * v1.normal).normalize_or_zero();
            let wn2 = (normal_matrix * v2.normal).normalize_or_zero();

            // Пользовательские атрибуты
            let a0 = build_attrs(&v0, w0, wn0);
            let a1 = build_attrs(&v1, w1, wn1);
            let a2 = build_attrs(&v2, w2, wn2);

            // Клиппинг по near/far
            let clipped = clip_triangle_near_far(
                [
                    ViewVertex::new(vp0, a0),
                    ViewVertex::new(vp1, a1),
                    ViewVertex::new(vp2, a2),
                ],
                self.camera.near,
                self.camera.far,
                &lerp_attrs,
            );

            // Проекция и запись в арены Overwhelement
            for tri in clipped {
                let indices = [
                    discretizer_scene.add_vertex(self.project_view_vertex(tri[0])),
                    discretizer_scene.add_vertex(self.project_view_vertex(tri[1])),
                    discretizer_scene.add_vertex(self.project_view_vertex(tri[2])),
                ];
                out.push(discretizer_scene.add_triangle(discretizer::Triangle {
                    id: object_id,
                    vertices: indices,
                }));
            }
        }

        out
    }

    /// Проецирует все узлы с мешами в одну плоскость Overwhelement.
    ///
    /// `world_matrices` — результат `Scene::compute_world_matrices()`.
    /// `object_id` для треугольника — индекс узла; если нужны другие id,
    /// используй `project_primitive` напрямую.
    pub fn project_scene<A, BF, LF>(
        &self,
        discretizer_scene: &mut discretizer::Scene<A>,
        scene: &Scene,
        world_matrices: &[Mat4],
        build_attrs: BF,
        lerp_attrs: LF,
    ) -> discretizer::Plane
    where
        A: discretizer::Attributes,
        BF: Fn(&Vertex, Vec3, Vec3) -> A,
        LF: Fn(&A, &A, f32) -> A,
    {
        let mut all_triangles = Vec::new();

        for (node_idx, node) in scene.nodes.iter().enumerate() {
            let Some(mesh_idx) = node.mesh else { continue };
            let world = world_matrices[node_idx];
            let mesh = &scene.meshes[mesh_idx];

            for prim in &mesh.primitives {
                let tris = self.project_primitive(
                    discretizer_scene,
                    prim,
                    world,
                    node_idx as u32,
                    &build_attrs,
                    &lerp_attrs,
                );
                all_triangles.extend(tris);
            }
        }

        discretizer::Plane { triangles: all_triangles }
    }

    /// View space → плоскость Overwhelement.
    /// Предполагается, что вершина уже прошла клиппинг: `w > 0`, `z` в `[-far, -near]`.
    fn project_view_vertex<A: discretizer::Attributes>(&self, vv: ViewVertex<A>) -> discretizer::Vertex<A> {
        let clip = self.camera.proj * vv.position.extend(1.0);
        debug_assert!(clip.w > 0.0, "clip.w must be positive after near-clipping");

        let inv_w = 1.0 / clip.w.max(1e-6);
        let ndc_x = clip.x * inv_w;
        let ndc_y = clip.y * inv_w;
        let ndc_z = clip.z * inv_w;

        let x = (ndc_x + 1.0) * 0.5 * self.plane_width;
        let y = (1.0 - ndc_y) * 0.5 * self.plane_height;

        discretizer::Vertex::with_depth(x, y, ndc_z, vv.attributes)
    }
}

/// Вершина в view space вместе с атрибутами.
/// Позиция — реальная 3D-точка в системе камеры, ещё без проекции.
#[derive(Copy, Clone, Debug)]
pub struct ViewVertex<A: Copy> {
    pub position: Vec3,
    pub attributes: A,
}

impl<A: Copy> ViewVertex<A> {
    pub fn new(position: Vec3, attributes: A) -> Self {
        Self { position, attributes }
    }
}

/// Что осталось от исходного треугольника после клиппинга.
/// Это выпуклый многоугольник из 0, 3 или 4 вершин.
/// 0 — треугольник полностью снаружи, дальше рендерить нечего.
/// 3 — треугольник целиком внутри или тривиально отсечён до одного.
/// 4 — потребуется разбить на два треугольника.
#[derive(Debug)]
pub struct ClippedPolygon<A: Copy> {
    pub vertices: Vec<ViewVertex<A>>,
}

impl<A: Copy> ClippedPolygon<A> {
    pub fn is_empty(&self) -> bool { self.vertices.is_empty() }
    pub fn len(&self) -> usize { self.vertices.len() }
}

/// Отсекает треугольник по near-плоскости (`z = -near`) в view space.
///
/// Камера смотрит в `-Z`, поэтому «внутри» — это `z <= -near`.
/// Атрибуты интерполируются линейно вдоль ребра.
///
/// `lerp_attrs(a, b, t)` — линейная интерполяция атрибутов.
/// Для большинства типов это `a + (b - a) * t`, но клиппинг
/// не навязывает конкретную реализацию.
pub fn clip_triangle_near<A: Copy>(
    tri: [ViewVertex<A>; 3],
    near: f32,
    lerp_attrs: impl Fn(&A, &A, f32) -> A,
) -> ClippedPolygon<A> {
    debug_assert!(near > 0.0);
    clip_polygon(
        &tri,
        // inside: z <= -near
        |v| v.position.z <= -near,
        // t при пересечении ребра a→b с плоскостью z = -near
        |a, b| {
            let dz = b.position.z - a.position.z;
            if dz.abs() < 1e-6 { 
                0.0 // Избегаем деления на ноль
            } else { 
                (-near - a.position.z) / dz 
            }
        },
        lerp_attrs,
    )
}

/// Отсекает треугольник по far-плоскости (`z = -far`) в view space.
///
/// «Внутри» — это `z >= -far`. Всё, что дальше (z < -far), отсекается.
pub fn clip_triangle_far<A: Copy>(
    tri: [ViewVertex<A>; 3],
    far: f32,
    lerp_attrs: impl Fn(&A, &A, f32) -> A,
) -> ClippedPolygon<A> {
    debug_assert!(far > 0.0);
    clip_polygon(
        &tri,
        |v| v.position.z >= -far,
        |a, b| {
            let dz = b.position.z - a.position.z;
            debug_assert!(dz.abs() > 1e-12, "edge parallel to far plane");
            (-far - a.position.z) / dz
        },
        lerp_attrs,
    )
}

/// Клиппинг по near и far последовательно.
/// Возвращает 0, 1 или 2 треугольника, каждый из которых целиком
/// лежит в допустимом диапазоне глубин.
pub fn clip_triangle_near_far<A: Copy>(
    tri: [ViewVertex<A>; 3],
    near: f32,
    far: f32,
    lerp_attrs: impl Fn(&A, &A, f32) -> A + Copy,
) -> Vec<[ViewVertex<A>; 3]> {
    let after_near = clip_triangle_near(tri, near, lerp_attrs);
    if after_near.vertices.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();

    // after_near — выпуклый полигон 3 или 4 вершин.
    // Если 3 — это один треугольник.
    // Если 4 — два.
    // Прогоняем каждый треугольник через far-плоскость.
    let split = |poly: &ClippedPolygon<A>| -> Vec<[ViewVertex<A>; 3]> {
        if poly.vertices.len() == 3 {
            vec![[poly.vertices[0], poly.vertices[1], poly.vertices[2]]]
        } else if poly.vertices.len() == 4 {
            vec![
                [poly.vertices[0], poly.vertices[1], poly.vertices[2]],
                [poly.vertices[0], poly.vertices[2], poly.vertices[3]],
            ]
        } else {
            // 0 или что-то неожиданное — пропускаем
            Vec::new()
        }
    };

    for tri_after_near in split(&after_near) {
        let after_far = clip_triangle_far(tri_after_near, far, lerp_attrs);
        for tri_final in split(&after_far) {
            out.push(tri_final);
        }
    }
    out
}

// ---------- Общая логика Sutherland–Hodgman для одной плоскости ----------

fn clip_polygon<A: Copy>(
    input: &[ViewVertex<A>],
    inside: impl Fn(&ViewVertex<A>) -> bool,
    intersect_t: impl Fn(&ViewVertex<A>, &ViewVertex<A>) -> f32,
    lerp_attrs: impl Fn(&A, &A, f32) -> A,
) -> ClippedPolygon<A> {
    let mut out: Vec<ViewVertex<A>> = Vec::with_capacity(4);
    let n = input.len();

    for i in 0..n {
        let curr = &input[i];
        let next = &input[(i + 1) % n];
        let curr_inside = inside(curr);
        let next_inside = inside(next);

        if curr_inside {
            out.push(*curr);
        }
        if curr_inside != next_inside {
            let t = intersect_t(curr, next);
            let position = curr.position.lerp(next.position, t);
            let attributes = lerp_attrs(&curr.attributes, &next.attributes, t);
            out.push(ViewVertex { position, attributes });
        }
    }
    ClippedPolygon { vertices: out }
}

/// Проверяет, видна ли передняя сторона треугольника из начала координат.
/// Используется во view space — там камера всегда в начале.
///
/// Требует CCW-обхода при взгляде снаружи:
/// `normal = cross(b - a, c - a)` смотрит наружу,
/// и если этот нормаль указывает в сторону камеры — грань передняя.
#[inline]
pub fn is_front_facing_view(a: Vec3, b: Vec3, c: Vec3) -> bool {
    let normal = (b - a).cross(c - a);
    let center = (a + b + c) / 3.0;
    normal.dot(-center) > 0.0
}

/// То же, но в world space, с явной позицией камеры.
/// Полезно, если клиппинг идёт в мире, а не во view.
#[inline]
pub fn is_front_facing_world(a: Vec3, b: Vec3, c: Vec3, camera_pos: Vec3) -> bool {
    let normal = (b - a).cross(c - a);
    let center = (a + b + c) / 3.0;
    normal.dot(camera_pos - center) > 0.0
}

/// Всё, что нужно для проекции в одном кадре.
/// Собирается один раз, переиспользуется на все вершины кадра.
#[derive(Clone, Debug)]
pub struct CameraContext {
    pub camera_pos: Vec3,
    pub view: Mat4,
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub near: f32,
    pub far: f32,
    pub aspect: f32,
}

impl CameraContext {
    /// Собирает контекст из узла-камеры в сцене.
    ///
    /// # Panics
    /// Если индекс узла вне границ, у узла нет камеры, или параметры камеры некорректны.
    pub fn from_node(scene: &Scene, node_idx: usize, aspect: f32) -> Self {
        assert!(aspect > 0.0, "aspect must be positive");

        let node = scene
            .nodes
            .get(node_idx)
            .unwrap_or_else(|| panic!("node index {node_idx} out of bounds"));
        let cam_idx = node
            .camera
            .unwrap_or_else(|| panic!("node {node_idx} has no camera"));
        let cam = &scene.cameras[cam_idx];
        cam.validate();

        let world = node.transform.to_matrix();
        let view = world.inverse();
        let proj = cam.proj_matrix(aspect);
        let view_proj = proj * view;
        let camera_pos = world.transform_point3(Vec3::ZERO);

        Self {
            camera_pos,
            view,
            proj,
            view_proj,
            near: cam.near,
            far: cam.far,
            aspect,
        }
    }

    #[inline]
    pub fn world_to_view(&self, world: Vec3) -> Vec3 {
        self.view.transform_point3(world)
    }

    #[inline]
    pub fn world_to_clip(&self, world: Vec3) -> Vec4 {
        self.view_proj * world.extend(1.0)
    }
}