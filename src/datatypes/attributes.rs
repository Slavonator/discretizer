/// Набор атрибутов, интерполируемых по поверхности примитива.
///
/// Реализуется пользователем для своего типа данных. Дискретизатор
/// не знает, что именно лежит в `A`, — он знает только, как это
/// интерполировать вдоль линии и внутри треугольника.
///
/// Требование `Copy` — сознательное: атрибуты лежат прямо в `Element`,
/// без индирекции и без аллокаций. Если атрибутов много и они тяжёлые,
/// стоит хранить в них индексы, а сами данные — отдельно.
pub trait Attributes: Copy {
    /// Значение для незанятых слотов буфера. Не «ноль» в арифметическом
    /// смысле, а именно заполнитель — поэтому у типа могут быть
    /// осмысленные ненулевые значения по умолчанию.
    fn placeholder() -> Self;

    /// Барицентрическая интерполяция: `w[0] + w[1] + w[2] == 1`.
    /// `w[0]` — вес против `a`, `w[1]` — против `b`, `w[2]` — против `c`.
    fn bary(a: &Self, b: &Self, c: &Self, w: [f32; 3]) -> Self;
}

impl Attributes for () {
    #[inline(always)] fn placeholder() -> Self {}
    #[inline(always)] fn bary(_: &Self, _: &Self, _: &Self, _: [f32; 3]) -> Self {}
}

impl Attributes for f32 {
    #[inline(always)] fn placeholder() -> Self { 0.0 }
    #[inline(always)]
    fn bary(a: &Self, b: &Self, c: &Self, w: [f32; 3]) -> Self {
        a * w[0] + b * w[1] + c * w[2]
    }
}

impl<const N: usize> Attributes for [f32; N] {
    #[inline(always)] fn placeholder() -> Self { [0.0; N] }
    #[inline(always)]
    fn bary(a: &Self, b: &Self, c: &Self, w: [f32; 3]) -> Self {
        let mut r = [0.0; N];
        for i in 0..N { r[i] = a[i] * w[0] + b[i] * w[1] + c[i] * w[2]; }
        r
    }
}