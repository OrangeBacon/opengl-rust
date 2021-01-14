use nalgebra_glm as glm;
use sdl2::keyboard::Scancode;

use crate::EngineState;

/// Simple camera object
/// uses euler angles, prevents looking up so that it doesn't gimble lock
/// Allows flying in any direction, is not bound to any plane.
/// No roll implemented. Scroll wheel to zoom in, wasd to move, mouse to chenge
/// direction
pub struct Camera {

    /// current camera position
    pos: glm::Vec3,

    /// vector facing forwards
    front: glm::Vec3,

    /// vector facing upwards, relative to the current look direction
    up: glm::Vec3,

    /// vector pointing right of the current look direction
    right: glm::Vec3,

    /// global up direction
    world_up: glm::Vec3,

    /// rotation around the y (vertical) axis
    yaw: f32,

    /// rotation around the x (horizontal) axis
    pitch: f32,

    /// zoom level from 1x to 10x
    zoom: f32,

    /// how fast to move the camera per update
    movement_speed: f32,

    /// how fast to move the camera per mouse movement
    mouse_sensitivity: f32,

    /// mow fast does the zoom level change when the scroll wheel is used
    zoom_speed: f32,
}

impl Camera {

    /// create a new camera using the default settings
    pub fn new() -> Self {
        let up = glm::vec3(0.0, 1.0, 0.0);
        let mut ret = Self {
            pos: glm::vec3(0.0, 0.0, 3.0),
            front: glm::vec3(0.0, 0.0, -1.0),
            right: glm::vec3(0.0, 0.0, 0.0),
            up: up,
            world_up: up,
            yaw: -90.0,
            pitch: 0.0,
            zoom: 1.0,
            movement_speed: 2.5,
            mouse_sensitivity: 0.2,
            zoom_speed: 0.1,
        };

        ret.update_vectors();

        ret
    }

    /// get the current view matrix from the camera
    pub fn get_view(&self) -> glm::Mat4 {
        glm::look_at(&self.pos, &(self.pos + self.front), &self.up)
    }

    /// get the current field of view
    pub fn get_fov(&self) -> f32 {
        (45.0 / self.zoom).to_radians()
    }

    /// update the camera's location from user input
    pub fn update(&mut self, state: &EngineState, dt: f32) {

        // move the camera with wasd
        let camera_speed = self.movement_speed * dt;
        if state.inputs.is_key_pressed(Scancode::W) {
            self.pos += camera_speed * self.front;
        }
        if state.inputs.is_key_pressed(Scancode::S) {
            self.pos -= camera_speed * self.front;
        }
        if state.inputs.is_key_pressed(Scancode::A) {
            self.pos -= camera_speed * self.right;
        }
        if state.inputs.is_key_pressed(Scancode::D) {
            self.pos += camera_speed * self.right;
        }
        if state.inputs.is_key_pressed(Scancode::Space) {
            self.pos += camera_speed * self.world_up;
        }
        if state.inputs.is_key_pressed(Scancode::LShift) {
            self.pos -= camera_speed * self.world_up;
        }

        // move the camera based upon mouse movement
        let x_offset = state.inputs.delta_x as f32 * self.mouse_sensitivity;
        let y_offset = state.inputs.delta_y as f32 * self.mouse_sensitivity;

        self.yaw += x_offset;
        self.pitch += y_offset;

        if self.pitch > 89.0 {
            self.pitch = 89.0
        }
        if self.pitch < -89.0 {
            self.pitch = -89.0
        }

        // change the zoom level depending upon scroll wheel
        self.zoom += self.zoom_speed * state.inputs.wheel_delta_y as f32;
        if self.zoom < 1.0 {
            self.zoom = 1.0;
        }
        if self.zoom > 15.0 {
            self.zoom = 15.0;
        }

        self.update_vectors();
    }

    /// re-calculate internal vectors, should be run every time one of the
    /// vectors is changed, so the view methods work correctly
    fn update_vectors(&mut self) {
        let yaw = self.yaw.to_radians();
        let pitch = self.pitch.to_radians();

        // euler angle rotation woo!  todo: replace
        let direction = glm::vec3(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        );
        self.front = glm::normalize(&direction);

        // update the direction vectors based upon the new camera location
        self.right = glm::normalize(&glm::cross(&self.front, &self.world_up));
        self.up = glm::normalize(&glm::cross(&self.right, &self.front));
    }
}
