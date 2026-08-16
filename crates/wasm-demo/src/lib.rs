#[cfg(target_arch = "wasm32")]
mod host {
    extern "C" {
        pub(super) fn clear(r: f32, g: f32, b: f32);
        pub(super) fn draw_triangle(
            x0: f32,
            y0: f32,
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
            r: f32,
            g: f32,
            b: f32,
        );
    }
}

#[cfg(target_arch = "wasm32")]
fn spin_triangle(time: f32, radius: f32, spin: f32, phase: f32, color: [f32; 3]) {
    let angle = time * spin + phase;
    let mut points = [[0.0f32; 2]; 3];
    for (index, point) in points.iter_mut().enumerate() {
        let corner_angle = angle + index as f32 * (core::f32::consts::TAU / 3.0);
        point[0] = corner_angle.cos() * radius;
        point[1] = corner_angle.sin() * radius;
    }
    unsafe {
        host::draw_triangle(
            points[0][0],
            points[0][1],
            points[1][0],
            points[1][1],
            points[2][0],
            points[2][1],
            color[0],
            color[1],
            color[2],
        );
    }
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn frame(time: f32) {
    unsafe {
        host::clear(0.06, 0.06, 0.09);
    }
    spin_triangle(time, 0.6, 0.8, 0.0, [0.90, 0.35, 0.35]);
    spin_triangle(time, 0.35, -1.3, 1.0, [0.35, 0.65, 0.90]);
    spin_triangle(time, 0.15, 2.1, 2.0, [0.95, 0.80, 0.30]);
}

#[cfg(not(target_arch = "wasm32"))]
#[no_mangle]
pub extern "C" fn frame(_time: f32) {}
