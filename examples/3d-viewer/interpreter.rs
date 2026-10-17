use discretizer::{Attributes, ElementBuffer};

/// ASCII-интерпретатор `ElementBuffer<A>`.
///
/// Заполняет символьную сетку. Ничего не знает о выводе в терминал —
/// для вывода используйте `terminalio::Screen`.
pub struct AsciiInterpreter {
    pub ramp: Vec<char>,
    pub background: char,
    /// Минимальная яркость (ambient). 0.0 — тёмный минимум рампы.
    pub min_intensity: f32,
    /// Слой, с которого читать (0 — самый верхний).
    pub layer: usize,
}

impl Default for AsciiInterpreter {
    fn default() -> Self {
        Self {
            ramp: " .:-=+*#%@".chars().collect(),
            background: ' ',
            min_intensity: 0.0,
            layer: 0,
        }
    }
}

impl AsciiInterpreter {
    pub fn new() -> Self { Self::default() }

    pub fn with_ramp(mut self, ramp: impl AsRef<str>) -> Self {
        self.ramp = ramp.as_ref().chars().collect();
        assert!(!self.ramp.is_empty(), "ramp must not be empty");
        self
    }

    pub fn with_background(mut self, c: char) -> Self {
        self.background = c;
        self
    }

    pub fn with_min_intensity(mut self, v: f32) -> Self {
        self.min_intensity = v.clamp(0.0, 1.0);
        self
    }

    pub fn with_layer(mut self, layer: usize) -> Self {
        self.layer = layer;
        self
    }

    /// Заполняет сетку символов из буфера дискрет.
    ///
    /// `grid` — плоский массив `width * height`, индекс `y * width + x`.
    /// `shade` — как из атрибутов элемента получить яркость `0..1`.
    pub fn fill<A: Attributes>(
        &self,
        grid: &mut [char],
        width: usize,
        height: usize,
        buffer: &ElementBuffer<A>,
        shade: impl Fn(&A) -> f32,
    ) {
        assert_eq!(grid.len(), width * height, "grid size mismatch");
        for y in 0..height {
            for x in 0..width {
                grid[y * width + x] = self.cell_char(buffer, x, y, &shade);
            }
        }
    }

    fn cell_char<A: Attributes>(
        &self,
        buffer: &ElementBuffer<A>,
        x: usize,
        y: usize,
        shade: &impl Fn(&A) -> f32,
    ) -> char {
        let Some(stack) = buffer.stack(x, y, self.layer) else {
            return self.background;
        };
        let Some(elem) = stack.first() else {
            return self.background;
        };
        let intensity = shade(&elem.attr);
        self.intensity_to_char(intensity)
    }

    fn intensity_to_char(&self, intensity: f32) -> char {
        let t = intensity.clamp(0.0, 1.0);
        let t = self.min_intensity + (1.0 - self.min_intensity) * t;
        let idx = (t * (self.ramp.len() - 1) as f32).round() as usize;
        self.ramp[idx.min(self.ramp.len() - 1)]
    }
}