use super::super::Attributes;

/// Вершина геометрии на плоскости.
///
/// Координаты и глубина — структурные поля, дискретизатор работает
/// с ними напрямую. Всё остальное — в `attr`.
#[derive(Copy, Clone, Debug)]
pub struct Vertex<A: Attributes> {
    pub x: f32,
    pub y: f32,
    pub depth: f32,
    pub attr: A,
}

impl<A: Attributes> Vertex<A> {
    /// Вершина на глубине 0.
    pub fn new(x: f32, y: f32, attr: A) -> Self {
        Vertex { x, y, depth: 0.0, attr }
    }

    /// Вершина с заданной глубиной.
    pub fn with_depth(x: f32, y: f32, depth: f32, attr: A) -> Self {
        Vertex { x, y, depth, attr }
    }
}