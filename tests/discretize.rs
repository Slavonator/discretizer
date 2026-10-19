use discretizer::{
    discretize, Configuration, Layer, Plane, Port, ScalingMode, Scene, Task, Triangle, Vertex,
    View,
};

fn empty_task() -> Task<()> {
    let mut scene = Scene::<()>::new();
    let plane = scene.add_plane(Plane { triangles: vec![] });

    let mut config = Configuration::new(8, 8);
    let view = config.add_view(View::new(0.0, 0.0, 8.0, 8.0));
    let port = config.add_port(Port::new(ScalingMode::Stretch));
    config.add_layer(Layer::new(plane, vec![(view, port)], 1));

    Task::new(scene, config)
}

#[test]
fn empty_scene_produces_buffer_with_correct_dimensions() {
    let buf = discretize(&empty_task());
    assert_eq!(buf.width, 8);
    assert_eq!(buf.height, 8);
    assert_eq!(buf.layers, 1);
    assert_eq!(buf.capacity(0), 1);
    assert_eq!(buf.cells().count(), 0);
}

#[test]
fn single_triangle_produces_cells_with_its_id() {
    let mut scene = Scene::<()>::new();
    let v0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, ()));
    let v1 = scene.add_vertex(Vertex::with_depth(10.0, 0.0, 0.0, ()));
    let v2 = scene.add_vertex(Vertex::with_depth(0.0, 10.0, 0.0, ()));
    let tri = scene.add_triangle(Triangle { id: 42, vertices: [v0, v1, v2] });
    let plane = scene.add_plane(Plane { triangles: vec![tri] });

    let mut config = Configuration::new(10, 10);
    let view = config.add_view(View::new(0.0, 0.0, 10.0, 10.0));
    let port = config.add_port(Port::new(ScalingMode::Stretch));
    config.add_layer(Layer::new(plane, vec![(view, port)], 1));

    let buf = discretize(&Task::new(scene, config));
    let cells: Vec<_> = buf.cells().collect();
    assert!(!cells.is_empty(), "triangle should cover some cells");
    for (_, _, _, stack) in cells {
        assert_eq!(stack[0].object_id, 42);
    }
}

#[test]
fn capacity_one_keeps_only_nearest() {
    let mut scene = Scene::<()>::new();
    // Near triangle
    let a0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.5, ()));
    let a1 = scene.add_vertex(Vertex::with_depth(10.0, 0.0, 0.5, ()));
    let a2 = scene.add_vertex(Vertex::with_depth(0.0, 10.0, 0.5, ()));
    let tri_a = scene.add_triangle(Triangle { id: 1, vertices: [a0, a1, a2] });
    // Far triangle
    let b0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 5.0, ()));
    let b1 = scene.add_vertex(Vertex::with_depth(10.0, 0.0, 5.0, ()));
    let b2 = scene.add_vertex(Vertex::with_depth(0.0, 10.0, 5.0, ()));
    let tri_b = scene.add_triangle(Triangle { id: 2, vertices: [b0, b1, b2] });
    let plane = scene.add_plane(Plane { triangles: vec![tri_a, tri_b] });

    let mut config = Configuration::new(10, 10);
    let view = config.add_view(View::new(0.0, 0.0, 10.0, 10.0));
    let port = config.add_port(Port::new(ScalingMode::Stretch));
    config.add_layer(Layer::new(plane, vec![(view, port)], 1));

    let buf = discretize(&Task::new(scene, config));
    for (_, _, _, stack) in buf.cells() {
        assert_eq!(stack.len(), 1, "capacity 1 holds only one fragment");
        assert_eq!(stack[0].object_id, 1, "nearest triangle should win");
    }
}

#[test]
fn two_layers_are_independent() {
    let mut scene = Scene::<()>::new();
    let v0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, ()));
    let v1 = scene.add_vertex(Vertex::with_depth(10.0, 0.0, 0.0, ()));
    let v2 = scene.add_vertex(Vertex::with_depth(0.0, 10.0, 0.0, ()));
    let tri = scene.add_triangle(Triangle { id: 1, vertices: [v0, v1, v2] });
    let plane = scene.add_plane(Plane { triangles: vec![tri] });

    let mut config = Configuration::new(10, 10);
    let view = config.add_view(View::new(0.0, 0.0, 10.0, 10.0));
    let port = config.add_port(Port::new(ScalingMode::Stretch));
    config.add_layer(Layer::new(plane, vec![(view, port)], 1));
    config.add_layer(Layer::new(plane, vec![(view, port)], 8));

    let buf = discretize(&Task::new(scene, config));
    assert_eq!(buf.layers, 2);
    assert_eq!(buf.capacity(0), 1);
    assert_eq!(buf.capacity(1), 8);

    let layer0: std::collections::HashSet<_> = buf
        .cells()
        .filter(|(_, _, l, _)| *l == 0)
        .map(|(x, y, _, _)| (x, y))
        .collect();
    let layer1: std::collections::HashSet<_> = buf
        .cells()
        .filter(|(_, _, l, _)| *l == 1)
        .map(|(x, y, _, _)| (x, y))
        .collect();
    assert_eq!(layer0, layer1, "same triangle covers same cells in both layers");
}

#[test]
fn view_origin_shifts_triangle() {
    // Треугольник в [0..2], но view начинается с 5 → он уедет в отрицательную
    // область буфера и обрежется, оставив меньше клеток, чем без сдвига.
    let make = |view_x: f32| {
        let mut scene = Scene::<()>::new();
        let v0 = scene.add_vertex(Vertex::with_depth(0.0, 0.0, 0.0, ()));
        let v1 = scene.add_vertex(Vertex::with_depth(2.0, 0.0, 0.0, ()));
        let v2 = scene.add_vertex(Vertex::with_depth(0.0, 2.0, 0.0, ()));
        let tri = scene.add_triangle(Triangle { id: 1, vertices: [v0, v1, v2] });
        let plane = scene.add_plane(Plane { triangles: vec![tri] });

        let mut config = Configuration::new(10, 10);
        let view = config.add_view(View::new(view_x, 0.0, 10.0, 10.0));
        let port = config.add_port(Port::new(ScalingMode::Stretch));
        config.add_layer(Layer::new(plane, vec![(view, port)], 1));
        discretize(&Task::new(scene, config))
    };

    let no_shift = make(0.0);
    let shifted = make(5.0);
    assert!(
        shifted.cells().count() < no_shift.cells().count(),
        "shifted view should lose cells"
    );
}