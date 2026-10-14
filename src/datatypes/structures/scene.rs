use super::{Plane, Vertex, Triangle};
use super::super::Attributes;

/// Чистая геометрия сцены. Не содержит информации об отображении.
#[derive(Clone, Debug)]
pub struct Scene<A: Attributes> {
    pub planes: Vec<Plane>,
    pub vertices: Vec<Vertex<A>>,
    pub triangles: Vec<Triangle>,
}

impl<A: Attributes> Scene<A> {
    pub fn new() -> Self {
        Self {
            planes: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
        }
    }

    pub fn add_plane(&mut self, plane: Plane) -> usize {
        self.planes.push(plane);
        self.planes.len() - 1
    }

    pub fn add_vertex(&mut self, vertex: Vertex<A>) -> usize {
        self.vertices.push(vertex);
        self.vertices.len() - 1
    }

    pub fn add_triangle(&mut self, triangle: Triangle) -> usize {
        self.triangles.push(triangle);
        self.triangles.len() - 1
    }
}