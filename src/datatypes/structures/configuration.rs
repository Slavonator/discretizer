use super::{Port, View, Layer};

/// Конфигурация дискретизации: размеры выходного буфера и все сущности,
/// описывающие отображение геометрии в буфер — виды, порты и слои.
#[derive(Clone, Debug)]
pub struct Configuration {
    pub output_width: usize,
    pub output_height: usize,
    pub views: Vec<View>,
    pub ports: Vec<Port>,
    pub layers: Vec<Layer>,
}

impl Configuration {
    pub fn new(width: usize, height: usize) -> Self {
        Configuration {
            output_width: width,
            output_height: height,
            views: Vec::new(),
            ports: Vec::new(),
            layers: Vec::new(),
        }
    }

    pub fn add_view(&mut self, view: View) -> usize {
        self.views.push(view);
        self.views.len() - 1
    }

    pub fn add_port(&mut self, port: Port) -> usize {
        self.ports.push(port);
        self.ports.len() - 1
    }

    pub fn add_layer(&mut self, layer: Layer) -> usize {
        self.layers.push(layer);
        self.layers.len() - 1
    }
}