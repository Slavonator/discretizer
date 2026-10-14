use super::Element;
use super::super::Attributes;

/// Буфер дискрет: `width × height × layers × capacity`.
///
/// Клетка `(x, y, layer)` хранит до `capacity` элементов, отсортированных
/// по `depth` от ближнего к дальнему. При переполнении вытесняется самый
/// дальний элемент клетки. При равных `depth` побеждает тот, кто пришёл раньше.
#[derive(Clone)]
pub struct ElementBuffer<A: Attributes> {
    pub width: usize,
    pub height: usize,
    pub layers: usize,
    pub capacity: usize,

    cell_count: usize,
    /// Плоский массив: индекс = `((y * width + x) * layers + layer) * capacity + slot`
    elements: Vec<Element<A>>,
    /// Число занятых слотов в клетке: индекс = `(y * width + x) * layers + layer`
    counts: Vec<u32>,
}

impl<A: Attributes> ElementBuffer<A> {
    /// Создаёт пустой буфер заданных размеров.
    ///
    /// # Panics
    /// Если любой из размеров равен нулю, `capacity > u32::MAX`,
    /// или произведение размеров переполняет `usize`.
    pub fn new(width: usize, height: usize, layers: usize, capacity: usize) -> Self {
        assert!(width > 0, "width must be > 0");
        assert!(height > 0, "height must be > 0");
        assert!(layers > 0, "layers must be > 0");
        assert!(capacity > 0, "capacity must be > 0");
        assert!(capacity <= u32::MAX as usize, "capacity must fit in u32");

        let cell_count = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(layers))
            .expect("ElementBuffer dimensions overflow");
        let total = cell_count
            .checked_mul(capacity)
            .expect("ElementBuffer total size overflow");

        Self {
            width,
            height,
            layers,
            capacity,
            cell_count,
            elements: vec![Element::placeholder(); total],
            counts: vec![0; cell_count],
        }
    }

    #[inline(always)]
    fn cell_index(&self, x: usize, y: usize, layer: usize) -> Option<usize> {
        if x < self.width && y < self.height && layer < self.layers {
            Some((y * self.width + x) * self.layers + layer)
        } else {
            None
        }
    }

    /// Занятые элементы клетки `(x, y, layer)` — от ближнего к дальнему.
    /// `None`, если координаты вне буфера; пустая клетка даёт пустой срез.
    #[inline]
    pub fn stack(&self, x: usize, y: usize, layer: usize) -> Option<&[Element<A>]> {
        let cell = self.cell_index(x, y, layer)?;
        let count = self.counts[cell] as usize;
        let base = cell * self.capacity;
        Some(&self.elements[base..base + count])
    }

    /// Добавляет элемент в клетку, сохраняя сортировку по `depth`.
    ///
    /// Если клетка полна и новый элемент дальше самого дальнего — игнорируется.
    /// Иначе самый дальний вытесняется.
    ///
    /// Возвращает `false`, если координаты вне буфера.
    ///
    /// # Panics
    /// Если `element.depth` не конечно (NaN, ±∞).
    pub fn push(
        &mut self,
        x: usize,
        y: usize,
        layer: usize,
        element: Element<A>,
    ) -> bool {
        assert!(
            element.depth.is_finite(),
            "element.depth must be finite, got {}",
            element.depth
        );

        let cell = match self.cell_index(x, y, layer) {
            Some(c) => c,
            None => return false,
        };

        let cap = self.capacity;
        let base = cell * cap;
        let count = self.counts[cell] as usize;

        // Полная клетка и новый элемент дальше самого дальнего — отбрасываем.
        if count == cap && element.depth >= self.elements[base + cap - 1].depth {
            return true;
        }

        // Позиция вставки: первый элемент, который строго дальше нового.
        let insert_at = self.elements[base..base + count]
            .iter()
            .position(|e| e.depth > element.depth)
            .unwrap_or(count);

        if count == cap {
            // Хвост сдвигается вправо, последний (самый дальний) теряется.
            self.elements
                .copy_within(base + insert_at..base + cap - 1, base + insert_at + 1);
        } else {
            self.elements
                .copy_within(base + insert_at..base + count, base + insert_at + 1);
            self.counts[cell] = (count + 1) as u32;
        }
        self.elements[base + insert_at] = element;
        true
    }

    /// Очищает буфер, сохраняя память.
    pub fn clear(&mut self) {
        for c in &mut self.counts { *c = 0; }
    }

    /// Итерация по всем непустым клеткам: `(x, y, layer, stack)`.
    /// `stack` отсортирован от ближнего к дальнему.
    pub fn cells(&self) -> impl Iterator<Item = (usize, usize, usize, &[Element<A>])> + '_ {
        let w = self.width;
        let l = self.layers;
        let cap = self.capacity;
        let elements = &self.elements;
        let counts = &self.counts;

        (0..self.cell_count).filter_map(move |cell| {
            let count = counts[cell] as usize;
            if count == 0 { return None; }
            let layer = cell % l;
            let xy = cell / l;
            let x = xy % w;
            let y = xy / w;
            let base = cell * cap;
            Some((x, y, layer, &elements[base..base + count]))
        })
    }

    /// Прямой доступ ко всем слотам, включая незанятые.
    #[inline]
    pub fn raw_elements(&self) -> &[Element<A>] { &self.elements }

    /// Прямой доступ к счётчикам занятости.
    #[inline]
    pub fn raw_counts(&self) -> &[u32] { &self.counts }
}