use super::Element;
use super::super::Attributes;

/// Буфер дискрет: `width × height × layers × capacities[layer]`.
///
/// Клетка `(x, y, layer)` хранит до `capacities[layer]` элементов,
/// отсортированных по `depth` от ближнего к дальнему. При переполнении
/// вытесняется самый дальний элемент клетки.
///
/// Раскладка памяти — «рваная»: каждый слой получает свой непрерывный
/// блок в `elements`, размер которого зависит от его `capacity`.
/// Слою с `capacity = 1` нужно в `layers` раз меньше памяти, чем слою
/// с `capacity = 8`.
#[derive(Clone)]
pub struct ElementBuffer<A: Attributes> {
    pub width: usize,
    pub height: usize,
    pub layers: usize,

    /// `capacities[layer]` — ёмкость клетки слоя.
    capacities: Vec<usize>,
    /// `layer_offset[layer]` — начало блока слоя в `elements`.
    layer_offset: Vec<usize>,

    /// Плоский массив всех элементов буфера.
    elements: Vec<Element<A>>,
    /// Счётчики занятости: индекс = `(y * width + x) * layers + layer`.
    counts: Vec<u32>,
}

impl<A: Attributes> ElementBuffer<A> {
    /// Создаёт пустой буфер.
    ///
    /// `capacities` — по одному значению на слой. Длина вектора задаёт
    /// число слоёв.
    ///
    /// # Panics
    /// Если `width`, `height` или любая из `capacities` равны нулю,
    /// либо суммарный размер переполняет `usize`.
    pub fn new(width: usize, height: usize, capacities: Vec<usize>) -> Self {
        assert!(width > 0, "width must be > 0");
        assert!(height > 0, "height must be > 0");
        assert!(!capacities.is_empty(), "at least one layer is required");

        let layers = capacities.len();
        let wh = width
            .checked_mul(height)
            .expect("width * height overflow");

        // Считаем смещения слоёв и общий размер.
        let mut layer_offset = Vec::with_capacity(layers);
        let mut total = 0usize;
        for (i, &cap) in capacities.iter().enumerate() {
            assert!(cap > 0, "capacity for layer {} must be > 0", i);
            assert!(cap <= u32::MAX as usize, "capacity must fit in u32");
            layer_offset.push(total);
            let layer_size = wh
                .checked_mul(cap)
                .expect("layer size overflow");
            total = total
                .checked_add(layer_size)
                .expect("total buffer size overflow");
        }

        let cell_count = wh
            .checked_mul(layers)
            .expect("cell count overflow");

        Self {
            width,
            height,
            layers,
            capacities,
            layer_offset,
            elements: vec![Element::placeholder(); total],
            counts: vec![0; cell_count],
        }
    }

    /// Ёмкость клетки слоя `layer`.
    #[inline]
    pub fn capacity(&self, layer: usize) -> usize {
        self.capacities[layer]
    }

    /// Координаты клетки: `(индекс счётчика, база элементов, ёмкость)`.
    /// `None`, если координаты вне буфера.
    #[inline(always)]
    fn cell_coords(&self, x: usize, y: usize, layer: usize) -> Option<(usize, usize, usize)> {
        if x >= self.width || y >= self.height || layer >= self.layers {
            return None;
        }
        let xy = y * self.width + x;
        let counts_idx = xy * self.layers + layer;
        let cap = self.capacities[layer];
        let elem_base = self.layer_offset[layer] + xy * cap;
        Some((counts_idx, elem_base, cap))
    }

    /// Занятые элементы клетки — от ближнего к дальнему.
    /// `None` — координаты вне буфера; пустая клетка даёт пустой срез.
    #[inline]
    pub fn stack(&self, x: usize, y: usize, layer: usize) -> Option<&[Element<A>]> {
        let (counts_idx, elem_base, _) = self.cell_coords(x, y, layer)?;
        let count = self.counts[counts_idx] as usize;
        Some(&self.elements[elem_base..elem_base + count])
    }

    /// Добавляет элемент в клетку, сохраняя сортировку по `depth`.
    ///
    /// Если клетка полна и новый элемент дальше самого дальнего — игнорируется.
    /// Иначе самый дальний вытесняется.
    ///
    /// Возвращает `false`, если координаты вне буфера.
    ///
    /// # Panics
    /// Если `element.depth` не конечно.
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

        let (counts_idx, base, cap) = match self.cell_coords(x, y, layer) {
            Some(c) => c,
            None => return false,
        };

        let count = self.counts[counts_idx] as usize;

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
            self.elements
                .copy_within(base + insert_at..base + cap - 1, base + insert_at + 1);
        } else {
            self.elements
                .copy_within(base + insert_at..base + count, base + insert_at + 1);
            self.counts[counts_idx] = (count + 1) as u32;
        }
        self.elements[base + insert_at] = element;
        true
    }

    pub fn clear(&mut self) {
        for c in &mut self.counts { *c = 0; }
    }

    /// Итерация по всем непустым клеткам: `(x, y, layer, stack)`.
    pub fn cells(&self) -> impl Iterator<Item = (usize, usize, usize, &[Element<A>])> + '_ {
        let w = self.width;
        let l = self.layers;
        let elements = &self.elements;
        let counts = &self.counts;
        let layer_offset = &self.layer_offset;
        let capacities = &self.capacities;
        let cell_count = w * self.height * l;

        (0..cell_count).filter_map(move |cell| {
            let count = counts[cell] as usize;
            if count == 0 { return None; }
            let layer = cell % l;
            let xy = cell / l;
            let x = xy % w;
            let y = xy / w;
            let base = layer_offset[layer] + xy * capacities[layer];
            Some((x, y, layer, &elements[base..base + count]))
        })
    }

    /// Прямой доступ ко всем элементам буфера (включая незанятые слоты).
    /// Раскладка — послойная, с учётом индивидуальных `capacities`.
    #[inline]
    pub fn raw_elements(&self) -> &[Element<A>] { &self.elements }

    #[inline]
    pub fn raw_counts(&self) -> &[u32] { &self.counts }
}