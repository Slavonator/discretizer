use super::super::{Attributes, EMPTY_OBJECT_ID, VOID_DEPTH};

/// Дискрета: элемент, попавший в клетку буфера.
///
/// `object_id` и `depth` — структурные поля, по ним работает сортировка
/// и выбор места. `attr` — пользовательские атрибуты. Слой, из которого
/// пришёл элемент, определяется позицией клетки в `ElementBuffer`.
#[derive(Copy, Clone, Debug)]
pub struct Element<A: Attributes> {
    /// Идентификатор объекта. `EMPTY_OBJECT_ID`, если объекта нет.
    pub object_id: u32,
    /// Удалённость от наблюдателя. `VOID_DEPTH`, если объекта нет.
    pub depth: f32,
    /// Пользовательские атрибуты.
    pub attr: A,
}

impl<A: Attributes> Element<A> {
    /// Заполнитель для незанятых слотов буфера.
    #[inline]
    pub fn placeholder() -> Self {
        Element {
            object_id: EMPTY_OBJECT_ID,
            depth: VOID_DEPTH,
            attr: A::placeholder(),
        }
    }
}