/// Плоскость, контейнер геометрии
#[derive(Clone, Debug)]
pub struct Plane {
    /// Список ссылок на треугольники на плоскости
    pub triangles: Vec<usize>,
}