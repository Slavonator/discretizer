use super::super::ScalingMode;
use super::Configuration;

/// Выходной порт: прямоугольная область в буфере и правила отображения в неё.
///
/// Нулевые `buffer_offset_*` означают отсутствие смещения.
/// Нулевые `buffer_width` / `buffer_height` означают «весь буфер».
#[derive(Clone, Debug)]
pub struct Port {
    /// Режим масштабирования вида в область порта
    pub scaling_mode: ScalingMode,
    /// Соотношение сторон одной выходной дискреты (пикселя)
    pub element_aspect_ratio: f32,
    /// Смещение области порта по X внутри выходного буфера (в пикселях)
    pub buffer_offset_x: u32,
    /// Смещение области порта по Y внутри выходного буфера (в пикселях)
    pub buffer_offset_y: u32,
    /// Ширина области порта в пикселях. `0` — весь буфер.
    pub buffer_width: u32,
    /// Высота области порта в пикселях. `0` — весь буфер.
    pub buffer_height: u32,
}

impl Default for Port {
    fn default() -> Self {
        Port {
            scaling_mode: ScalingMode::None,
            element_aspect_ratio: 1.0,
            buffer_offset_x: 0,
            buffer_offset_y: 0,
            buffer_width: 0,
            buffer_height: 0,
        }
    }
}

impl Port {
    /// Создаёт порт с заданным режимом масштабирования.
    pub fn new(scaling_mode: ScalingMode) -> Self {
        Port {
            scaling_mode,
            ..Default::default()
        }
    }

    /// Устанавливает смещение области порта в пикселях.
    pub fn with_buffer_offset(mut self, x: u32, y: u32) -> Self {
        self.buffer_offset_x = x;
        self.buffer_offset_y = y;
        self
    }

    /// Устанавливает размеры области порта в пикселях.
    pub fn with_buffer_size(mut self, width: u32, height: u32) -> Self {
        self.buffer_width = width;
        self.buffer_height = height;
        self
    }

    /// Задаёт соотношение сторон дискреты.
    /// # Panics
    /// Если `aspect <= 0.0` или не конечно.
    pub fn with_aspect_ratio(mut self, aspect: f32) -> Self {
        assert!(
            aspect.is_finite() && aspect > 0.0,
            "element_aspect_ratio must be positive and finite, got {}",
            aspect
        );
        self.element_aspect_ratio = aspect;
        self
    }

    /// Фактические границы области порта в буфере: `(offset_x, offset_y, width, height)`.
    /// Нулевые размеры разворачиваются в размеры выходного буфера.
    pub fn resolved_rect(&self, config: &Configuration) -> (u32, u32, u32, u32) {
        let w = if self.buffer_width == 0 {
            config.output_width as u32
        } else {
            self.buffer_width
        };
        let h = if self.buffer_height == 0 {
            config.output_height as u32
        } else {
            self.buffer_height
        };
        (self.buffer_offset_x, self.buffer_offset_y, w, h)
    }
}