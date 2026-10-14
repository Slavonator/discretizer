use super::{Scene, Configuration};
use super::super::Attributes;

/// Задача дискретизации: чистая геометрия сцены и конфигурация процесса.
#[derive(Clone, Debug)]
pub struct Task<A: Attributes> {
    pub scene: Scene<A>,
    pub config: Configuration,
}

impl<A: Attributes> Task<A> {
    /// Создаёт задачу с проверкой ссылочной целостности между сценой
    /// и конфигурацией.
    ///
    /// # Panics
    /// Если `config.layers` пуст, либо любой из индексов внутри `Layer`
    /// выходит за границы соответствующих коллекций конфигурации
    /// или за границы `scene.planes`.
    pub fn new(scene: Scene<A>, config: Configuration) -> Self {
        assert!(
            !config.layers.is_empty(),
            "Configuration must contain at least one layer"
        );

        for layer in &config.layers {
            assert!(
                layer.plane_index < scene.planes.len(),
                "Layer plane_index {} out of bounds (planes count: {})",
                layer.plane_index, scene.planes.len()
            );
            for &(view_idx, port_idx) in &layer.bindings {
                assert!(
                    view_idx < config.views.len(),
                    "view index {} out of bounds (views count: {})",
                    view_idx, config.views.len()
                );
                assert!(
                    port_idx < config.ports.len(),
                    "port index {} out of bounds (ports count: {})",
                    port_idx, config.ports.len()
                );
            }
        }

        Task { scene, config }
    }
}