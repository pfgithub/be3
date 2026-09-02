const EYE_HEIGHT: f32 = 1.7;
const MOVE_UNITS_PER_SECOND: f32 = 4.0;
const LOOK_RADIANS_PER_PIXEL: f32 = 0.0025;
const MAX_PITCH: f32 = 1.5;
const FOV_Y_RADIANS: f32 = 1.309;
const NEAR: f32 = 0.05;
const FAR: f32 = 200.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Camera {
    position: [f32; 3],
    yaw: f32,
    pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: [0.0, EYE_HEIGHT, 8.0],
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

impl Camera {
    pub(crate) fn look(&mut self, delta: [f32; 2]) {
        self.yaw += delta[0] * LOOK_RADIANS_PER_PIXEL;
        self.pitch = (self.pitch - delta[1] * LOOK_RADIANS_PER_PIXEL).clamp(-MAX_PITCH, MAX_PITCH);
    }

    pub(crate) fn walk(&mut self, strafe: f32, forward: f32, dt: f32) {
        let length = (strafe * strafe + forward * forward).sqrt();
        if length < f32::EPSILON {
            return;
        }
        let (strafe, forward) = (strafe / length, forward / length);
        let (sin_yaw, cos_yaw) = (self.yaw.sin(), self.yaw.cos());
        let distance = MOVE_UNITS_PER_SECOND * dt;
        self.position[0] += (sin_yaw * forward + cos_yaw * strafe) * distance;
        self.position[2] += (-cos_yaw * forward + sin_yaw * strafe) * distance;
    }

    pub(crate) fn view_projection(&self, aspect: f32) -> [[f32; 4]; 4] {
        let forward = [
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            -self.pitch.cos() * self.yaw.cos(),
        ];
        let view = look_at(self.position, add(self.position, forward), [0.0, 1.0, 0.0]);
        let projection = perspective(FOV_Y_RADIANS, aspect, NEAR, FAR);
        mat4_mul(projection, view)
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let length = dot(a, a).sqrt();
    [a[0] / length, a[1] / length, a[2] / length]
}

fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let forward = normalize(sub(center, eye));
    let side = normalize(cross(forward, up));
    let up = cross(side, forward);
    [
        [side[0], up[0], -forward[0], 0.0],
        [side[1], up[1], -forward[1], 0.0],
        [side[2], up[2], -forward[2], 0.0],
        [-dot(side, eye), -dot(up, eye), dot(forward, eye), 1.0],
    ]
}

fn perspective(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let focal_length = 1.0 / (fov_y_radians * 0.5).tan();
    [
        [focal_length / aspect, 0.0, 0.0, 0.0],
        [0.0, focal_length, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ]
}

fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0f32; 4]; 4];
    for (col, result_col) in result.iter_mut().enumerate() {
        for (row, cell) in result_col.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[k][row] * b[col][k]).sum();
        }
    }
    result
}
