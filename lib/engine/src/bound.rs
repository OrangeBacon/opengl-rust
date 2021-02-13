use nalgebra_glm as glm;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Bounds {
    pub min_x: f32,
    pub min_y: f32,
    pub min_z: f32,

    pub max_x: f32,
    pub max_y: f32,
    pub max_z: f32,
}

impl Bounds {
    pub fn new_nan() -> Self {
        Self {
            min_x: f32::NAN,
            min_y: f32::NAN,
            min_z: f32::NAN,

            max_x: f32::NAN,
            max_y: f32::NAN,
            max_z: f32::NAN,
        }
    }

    pub fn from_slice(min: &[f32], max: &[f32]) -> Self {
        Self {
            min_x: min[0],
            min_y: min[1],
            min_z: min[2],

            max_x: max[0],
            max_y: max[1],
            max_z: max[2],
        }
    }

    pub fn new_from(&self, bound: &Bounds) -> Bounds {
        Bounds {
            min_x: self.min_x.min(bound.min_x),
            min_y: self.min_y.min(bound.min_y),
            min_z: self.min_z.min(bound.min_z),

            max_x: self.max_x.max(bound.max_x),
            max_y: self.max_y.max(bound.max_y),
            max_z: self.max_z.max(bound.max_z),
        }
    }

    pub fn merge(&mut self, bound: &Bounds) {
        *self = self.new_from(bound);
    }

    pub fn apply_mat(&self, mat: &glm::Mat4) -> Self {
        let min = mat * glm::vec4(self.min_x, self.min_y, self.min_z, 0.0);
        let max = mat * glm::vec4(self.max_x, self.max_y, self.max_z, 0.0);

        Self::from_slice(min.as_slice(), max.as_slice())
    }
}
