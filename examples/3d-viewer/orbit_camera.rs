use glam::{Vec3, Quat};
use crate::projector::scene::{Scene};

pub struct OrbitCamera {
    node: usize,
    target: Vec3,
    radius: f32,
    yaw: f32,
    pitch: f32,
}

impl OrbitCamera {
    pub fn new(node: usize, target: Vec3, radius: f32) -> Self {
        Self {node, target, radius, yaw: 0f32, pitch: 0f32}
    }

    pub fn position(&self) -> Vec3 {
        let x = self.radius * self.pitch.cos() * self.yaw.sin();
        let y = self.radius * self.pitch.sin();
        let z = self.radius * self.pitch.cos() * self.yaw.cos();
        self.target + glam::Vec3::new(x, y, z)
    }

    pub fn apply(&self, scene: &mut Scene) {
        let node = scene.node_mut(self.node);
        let pos = self.position();
        node.transform.translation = pos;

        // Строим Camera-to-World матрицу напрямую через векторы
        // Ось Z смотрит ОТ цели К камере (т.к. OpenGL смотрит в -Z)
        let z_axis = (pos - self.target).normalize();
        
        // Ось X (вправо) — это векторное произведение мировой оси Y и Z
        let x_axis = Vec3::Y.cross(z_axis).normalize();
        
        // Ось Y (вверх) — это произведение Z и X
        let y_axis = z_axis.cross(x_axis);

        // Собираем матрицу поворота из колонок (X, Y, Z) и делаем из неё Quat
        let rotation_mat = glam::Mat3::from_cols(x_axis, y_axis, z_axis);
        node.transform.rotation = Quat::from_mat3(&rotation_mat);
    }
    pub fn increase_yaw(&mut self, value: f32) {
        self.yaw += value;
    }

    pub fn decrease_yaw(&mut self, value: f32) {
        self.yaw -= value;
    }

    pub fn increase_pitch(&mut self, value: f32) {
        self.pitch += value;
        self.pitch = self.pitch.min(1.4);
    }

    pub fn decrease_pitch(&mut self, value: f32) {
        self.pitch -= value;
        self.pitch = self.pitch.max(-1.4);
    }

    pub fn zoom_in(&mut self, amount: f32) {
        self.radius = (self.radius - amount).max(0.01); // не даём уйти в 0 или минус
    }

    pub fn zoom_out(&mut self, amount: f32) {
        self.radius += amount;
    }
}