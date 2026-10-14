use super::{Port, View, Layer};

/// Конфигурация дискретизации: размеры выходного буфера, лимит перекрытий
/// и все сущности, описывающие отображение геометрии в буфер —
/// виды, порты и слои.
#[derive(Clone, Debug)]
pub struct Configuration {
    /// Ширина выходного буфера в пикселях
    pub output_width: usize,
    /// Высота выходного буфера в пикселях
    pub output_height: usize,
    /// Максимум элементов в одной клетке `(x, y, layer)`.
    /// Определяет `capacity` буфера дискрет.
    pub max_overlap: usize,
    /// Все виды, используемые слоями
    pub views: Vec<View>,
    /// Все порты, используемые слоями
    pub ports: Vec<Port>,
    /// Слои дискретизации, задающие порядок обработки
    pub layers: Vec<Layer>,
}

impl Configuration {
    /// Создаёт конфигурацию с заданными размерами буфера
    /// и лимитом перекрытий по умолчанию (1).
    pub fn new(width: usize, height: usize) -> Self {
        Configuration {
            output_width: width,
            output_height: height,
            max_overlap: 1,
            views: Vec::new(),
            ports: Vec::new(),
            layers: Vec::new(),
        }
    }

    /// Задаёт лимит перекрытий.
    /// # Panics
    /// Если `max_overlap == 0`.
    pub fn with_overlap_limit(mut self, max_overlap: usize) -> Self {
        assert!(max_overlap > 0, "max_overlap must be > 0");
        self.max_overlap = max_overlap;
        self
    }

    /// Добавляет вид и возвращает его индекс.
    pub fn add_view(&mut self, view: View) -> usize {
        self.views.push(view);
        self.views.len() - 1
    }

    /// Добавляет порт и возвращает его индекс.
    pub fn add_port(&mut self, port: Port) -> usize {
        self.ports.push(port);
        self.ports.len() - 1
    }

    /// Добавляет слой и возвращает его индекс.
    pub fn add_layer(&mut self, layer: Layer) -> usize {
        self.layers.push(layer);
        self.layers.len() - 1
    }
}