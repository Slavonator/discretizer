use discretizer::{Element, ElementBuffer, EMPTY_OBJECT_ID, VOID_DEPTH};

fn elem(id: u32, depth: f32) -> Element<()> {
    Element { object_id: id, depth, attr: () }
}

#[test]
fn new_sets_dimensions() {
    let buf = ElementBuffer::<()>::new(3, 4, vec![1, 2, 3]);
    assert_eq!(buf.width, 3);
    assert_eq!(buf.height, 4);
    assert_eq!(buf.layers, 3);
}

#[test]
#[should_panic]
fn new_panics_on_zero_width() {
    let _ = ElementBuffer::<()>::new(0, 4, vec![1]);
}

#[test]
#[should_panic]
fn new_panics_on_zero_height() {
    let _ = ElementBuffer::<()>::new(3, 0, vec![1]);
}

#[test]
#[should_panic]
fn new_panics_on_zero_capacity() {
    let _ = ElementBuffer::<()>::new(3, 4, vec![1, 0]);
}

#[test]
fn capacity_matches_input() {
    let buf = ElementBuffer::<()>::new(2, 2, vec![1, 4, 8]);
    assert_eq!(buf.capacity(0), 1);
    assert_eq!(buf.capacity(1), 4);
    assert_eq!(buf.capacity(2), 8);
}

#[test]
fn stack_returns_none_out_of_bounds() {
    let buf = ElementBuffer::<()>::new(2, 2, vec![1]);
    assert!(buf.stack(2, 0, 0).is_none());
    assert!(buf.stack(0, 2, 0).is_none());
    assert!(buf.stack(0, 0, 1).is_none());
}

#[test]
fn empty_cell_returns_empty_slice() {
    let buf = ElementBuffer::<()>::new(2, 2, vec![1]);
    assert!(buf.stack(0, 0, 0).unwrap().is_empty());
}

#[test]
fn push_returns_true_for_valid_coordinates() {
    let mut buf = ElementBuffer::<()>::new(2, 2, vec![1]);
    assert!(buf.push(0, 0, 0, elem(1, 1.0)));
}

#[test]
fn push_returns_false_for_out_of_bounds() {
    let mut buf = ElementBuffer::<()>::new(2, 2, vec![1]);
    assert!(!buf.push(2, 0, 0, elem(1, 1.0)));
    assert!(!buf.push(0, 2, 0, elem(1, 1.0)));
    assert!(!buf.push(0, 0, 1, elem(1, 1.0)));
}

#[test]
fn push_stores_element() {
    let mut buf = ElementBuffer::<()>::new(2, 2, vec![1]);
    buf.push(1, 1, 0, elem(42, 3.5));
    let s = buf.stack(1, 1, 0).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].object_id, 42);
    assert_eq!(s[0].depth, 3.5);
}

#[test]
fn push_sorts_by_depth() {
    let mut buf = ElementBuffer::<()>::new(1, 1, vec![3]);
    buf.push(0, 0, 0, elem(1, 2.0));
    buf.push(0, 0, 0, elem(2, 1.0));
    buf.push(0, 0, 0, elem(3, 3.0));
    let s = buf.stack(0, 0, 0).unwrap();
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].depth, 1.0);
    assert_eq!(s[1].depth, 2.0);
    assert_eq!(s[2].depth, 3.0);
}

#[test]
fn push_evicts_farthest_on_overflow() {
    let mut buf = ElementBuffer::<()>::new(1, 1, vec![2]);
    buf.push(0, 0, 0, elem(1, 1.0));
    buf.push(0, 0, 0, elem(2, 2.0));
    // 0.5 ближе самого дальнего (2.0) → вытесняем 2.0, вставляем 0.5.
    buf.push(0, 0, 0, elem(3, 0.5));
    let s = buf.stack(0, 0, 0).unwrap();
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].object_id, 3); // 0.5 — теперь ближайший
    assert_eq!(s[1].object_id, 1); // 1.0 — второй
}
#[test]
fn push_ignores_farther_element_when_full() {
    let mut buf = ElementBuffer::<()>::new(1, 1, vec![2]);
    buf.push(0, 0, 0, elem(1, 1.0));
    buf.push(0, 0, 0, elem(2, 2.0));
    buf.push(0, 0, 0, elem(3, 5.0));
    let s = buf.stack(0, 0, 0).unwrap();
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].object_id, 1);
    assert_eq!(s[1].object_id, 2);
}

#[test]
#[should_panic]
fn push_panics_on_nan_depth() {
    let mut buf = ElementBuffer::<()>::new(1, 1, vec![1]);
    buf.push(0, 0, 0, elem(1, f32::NAN));
}

#[test]
#[should_panic]
fn push_panics_on_infinite_depth() {
    let mut buf = ElementBuffer::<()>::new(1, 1, vec![1]);
    buf.push(0, 0, 0, elem(1, f32::INFINITY));
}

#[test]
fn clear_removes_all_elements() {
    let mut buf = ElementBuffer::<()>::new(2, 2, vec![1]);
    buf.push(0, 0, 0, elem(1, 1.0));
    buf.clear();
    assert_eq!(buf.cells().count(), 0);
}

#[test]
fn clear_allows_push_again() {
    let mut buf = ElementBuffer::<()>::new(2, 2, vec![1]);
    buf.push(0, 0, 0, elem(1, 1.0));
    buf.clear();
    buf.push(0, 0, 0, elem(2, 2.0));
    assert_eq!(buf.stack(0, 0, 0).unwrap()[0].object_id, 2);
}

#[test]
fn cells_yields_correct_coordinates() {
    let mut buf = ElementBuffer::<()>::new(3, 3, vec![1, 1]);
    buf.push(0, 0, 0, elem(1, 1.0));
    buf.push(2, 1, 1, elem(2, 2.0));
    buf.push(1, 2, 0, elem(3, 3.0));

    let coords: std::collections::HashSet<_> =
        buf.cells().map(|(x, y, l, _)| (x, y, l)).collect();
    assert!(coords.contains(&(0, 0, 0)));
    assert!(coords.contains(&(2, 1, 1)));
    assert!(coords.contains(&(1, 2, 0)));
    assert_eq!(coords.len(), 3);
}

#[test]
fn raw_elements_length_equals_total_slots() {
    let buf = ElementBuffer::<()>::new(3, 4, vec![1, 2]);
    assert_eq!(buf.raw_elements().len(), 3 * 4 * (1 + 2));
}

#[test]
fn raw_counts_sum_matches_stored_elements() {
    let mut buf = ElementBuffer::<()>::new(2, 2, vec![2, 2]);
    assert_eq!(buf.raw_counts().iter().sum::<u32>(), 0);

    buf.push(0, 0, 0, elem(1, 1.0));
    buf.push(1, 1, 1, elem(2, 2.0));
    buf.push(0, 0, 0, elem(3, 3.0));
    assert_eq!(buf.raw_counts().iter().sum::<u32>(), 3);
}

#[test]
fn clone_preserves_content() {
    let mut buf = ElementBuffer::<()>::new(2, 2, vec![2]);
    buf.push(0, 0, 0, elem(42, 1.0));
    let clone = buf.clone();
    assert_eq!(clone.stack(0, 0, 0).unwrap()[0].object_id, 42);
}

#[test]
fn placeholder_has_sentinel_values() {
    let el: Element<()> = Element::placeholder();
    assert_eq!(el.object_id, EMPTY_OBJECT_ID);
    assert_eq!(el.depth, VOID_DEPTH);
}

#[test]
fn constants_match_documentation() {
    assert_eq!(EMPTY_OBJECT_ID, u32::MAX);
    assert_eq!(VOID_DEPTH, f32::INFINITY);
}