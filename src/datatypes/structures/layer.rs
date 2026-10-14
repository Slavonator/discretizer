/// Слой отрисовки: связывает одну плоскость сцены с набором пар
/// (индекс View, индекс Port). Каждая пара — один проход дискретизации:
/// геометрия плоскости, взятая через View, выводится через Port.
#[derive(Clone, Debug)]
pub struct Layer {
    /// Индекс плоскости в `Scene::planes`
    pub plane_index: usize,
    /// Пары `(view_index, port_index)`, индексы в `Configuration::views`
    /// и `Configuration::ports` соответственно.
    pub bindings: Vec<(usize, usize)>,
}

impl Layer {
    /// Создаёт новый слой.
    /// # Panics
    /// Если `bindings` пуст.
    pub fn new(plane_index: usize, bindings: Vec<(usize, usize)>) -> Self {
        assert!(!bindings.is_empty(), "bindings must not be empty");
        Self { plane_index, bindings }
    }
}