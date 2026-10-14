/// Треугольник, примитивный объект геометрии, имеющий площадь
#[derive(Clone, Debug)]
pub struct Triangle {
    /// Идентификатор объекта
    pub id: u32,
    /// Вершины объекта
    pub vertices: [usize; 3]
}
