/// Слой отрисовки: связывает одну плоскость сцены с набором пар
/// (индекс View, индекс Port). Каждая пара — один проход дискретизации.
///
/// `capacity` задаёт максимальное число элементов в клетке `(x, y, layer)`
/// этого слоя. Обычно для непрозрачных объектов достаточно 1, для
/// прозрачных — 4–16, в зависимости от того, сколько перекрытий
/// интерпретатор собирается смешивать.
#[derive(Clone, Debug)]
pub struct Layer {
    /// Индекс плоскости в `Scene::planes`
    pub plane_index: usize,
    /// Пары `(view_index, port_index)`.
    pub bindings: Vec<(usize, usize)>,
    /// Максимум элементов в клетке этого слоя.
    pub capacity: usize,
}

impl Layer {
    /// Создаёт новый слой.
    /// # Panics
    /// Если `bindings` пуст или `capacity == 0`.
    pub fn new(plane_index: usize, bindings: Vec<(usize, usize)>, capacity: usize) -> Self {
        assert!(!bindings.is_empty(), "bindings must not be empty");
        assert!(capacity > 0, "capacity must be > 0");
        Self { plane_index, bindings, capacity }
    }
}