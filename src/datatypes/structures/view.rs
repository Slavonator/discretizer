/// Вид (снимок) плоскости: определяет прямоугольную область и поворот.
#[derive(Clone, Debug)]
pub struct View {
    /// Координата X левого верхнего угла области на плоскости
    pub x: f32,
    /// Координата Y левого верхнего угла области на плоскости
    pub y: f32,
    /// Ширина области (может быть отрицательной для отражения по горизонтали)
    pub width: f32,
    /// Высота области (может быть отрицательной для отражения по вертикали)
    pub height: f32,
    /// Угол поворота области вокруг её центра (в радианах)
    pub rotation_angle: f32,
}

impl Default for View {
    fn default() -> Self {
        View {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            rotation_angle: 0.0,
        }
    }
}

impl View {
    /// Создаёт вид с заданными границами.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        assert!(width != 0.0, "View width must be non-zero");
        assert!(height != 0.0, "View height must be non-zero");
        View { x, y, width, height, rotation_angle: 0.0 }
    }

    /// Задаёт угол поворота в радианах.
    pub fn with_rotation(mut self, angle: f32) -> Self {
        self.rotation_angle = angle;
        self
    }

}