// src/internal_datatypes.rs

/// Предвычисленные параметры viewport-преобразования.
#[derive(Clone)]
pub(crate) struct ViewportTransform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub has_rotation: bool,
    pub cos: f32,
    pub sin: f32,
    /// Центр вращения
    pub cx: f32,
    pub cy: f32,
    /// Множитель Y для компенсации неквадратной дискреты
    pub y_aspect: f32,
}