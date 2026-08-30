//! Stateful integer pointer tracking for desktop profiles.

use crate::config::IrTracking;

pub const POINTER_TICK_INTERVAL_US: u64 = 16_000;
pub const POINTER_LEFT: u8 = 1 << 0;
pub const POINTER_RIGHT: u8 = 1 << 1;
pub const POINTER_UP: u8 = 1 << 2;
pub const POINTER_DOWN: u8 = 1 << 3;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrPoint {
    pub valid: bool,
    pub x: i32,
    pub y: i32,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IrFrame {
    pub points: [IrPoint; 4],
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerDelta {
    pub dx: i32,
    pub dy: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerState {
    pointer_keys: u8,
    speed: i32,
    dx: i32,
    dy: i32,
    ir_active: bool,
    ir_x: i32,
    ir_y: i32,
    ir_speed: i32,
    ir_deadzone: i32,
    ir_smoothing: i32,
    tracking: IrTracking,
}
impl Default for PointerState {
    fn default() -> Self {
        Self::new(16, 8, 0, 0, IrTracking::Dual)
    }
}
impl PointerState {
    pub const fn new(
        speed: i32,
        ir_speed: i32,
        ir_deadzone: i32,
        ir_smoothing: i32,
        tracking: IrTracking,
    ) -> Self {
        Self {
            pointer_keys: 0,
            speed,
            dx: 0,
            dy: 0,
            ir_active: false,
            ir_x: 0,
            ir_y: 0,
            ir_speed,
            ir_deadzone,
            ir_smoothing,
            tracking,
        }
    }
    pub fn speed(&self) -> i32 {
        self.speed
    }
    pub fn set_speed(&mut self, speed: i32) {
        self.speed = speed;
        self.refresh_velocity();
    }
    pub fn set_ir_options(
        &mut self,
        speed: i32,
        deadzone: i32,
        smoothing: i32,
        tracking: IrTracking,
    ) {
        self.ir_speed = speed;
        self.ir_deadzone = deadzone;
        self.ir_smoothing = smoothing;
        self.tracking = tracking;
    }
    pub fn pointer_keys(&self) -> u8 {
        self.pointer_keys
    }
    pub fn velocity(&self) -> PointerDelta {
        PointerDelta {
            dx: self.dx,
            dy: self.dy,
        }
    }
    pub fn update_key(&mut self, bit: u8, pressed: bool) -> PointerDelta {
        if pressed {
            self.pointer_keys |= bit;
        } else {
            self.pointer_keys &= !bit;
        }
        self.refresh_velocity();
        self.velocity()
    }
    fn refresh_velocity(&mut self) {
        self.dx = 0;
        self.dy = 0;
        if self.pointer_keys & POINTER_LEFT != 0 {
            self.dx -= self.speed;
        }
        if self.pointer_keys & POINTER_RIGHT != 0 {
            self.dx += self.speed;
        }
        if self.pointer_keys & POINTER_UP != 0 {
            self.dy -= self.speed;
        }
        if self.pointer_keys & POINTER_DOWN != 0 {
            self.dy += self.speed;
        }
    }
    /// One fixed-rate D-pad tick. It returns no event when all keys are up.
    pub fn tick(&self) -> PointerDelta {
        self.velocity()
    }
    pub fn reset(&mut self) {
        self.pointer_keys = 0;
        self.dx = 0;
        self.dy = 0;
        self.reset_ir();
    }
    pub fn reset_ir(&mut self) {
        self.ir_active = false;
        self.ir_x = 0;
        self.ir_y = 0;
    }
    pub fn ir_active(&self) -> bool {
        self.ir_active
    }

    pub fn select_ir(&self, frame: &IrFrame) -> Option<IrPoint> {
        select_ir_point(frame, self.tracking)
    }
    /// Updates the IR relative tracker. Acquisition establishes a baseline and
    /// emits no jump; loss clears it so reacquisition also establishes a baseline.
    pub fn update_ir(&mut self, point: Option<IrPoint>) -> PointerDelta {
        let Some(point) = point.filter(|point| point.valid) else {
            self.reset_ir();
            return PointerDelta::default();
        };
        let mut out = PointerDelta::default();
        if self.ir_active {
            let x = smooth_axis(self.ir_x, point.x, self.ir_smoothing);
            let y = smooth_axis(self.ir_y, point.y, self.ir_smoothing);
            out.dx = apply_deadzone(scaled_delta(self.ir_x, x, self.ir_speed), self.ir_deadzone);
            out.dy = apply_deadzone(scaled_delta(self.ir_y, y, self.ir_speed), self.ir_deadzone);
            self.ir_x = x;
            self.ir_y = y;
        } else {
            self.ir_x = point.x;
            self.ir_y = point.y;
        }
        self.ir_active = true;
        out
    }
    pub fn update_ir_frame(&mut self, frame: &IrFrame) -> PointerDelta {
        self.update_ir(self.select_ir(frame))
    }
}

pub fn scaled_delta(from: i32, to: i32, speed: i32) -> i32 {
    (i64::from(to.wrapping_sub(from)) * i64::from(speed) / 64) as i32
}
pub fn apply_deadzone(delta: i32, deadzone: i32) -> i32 {
    if deadzone != 0 && delta.abs() < deadzone {
        0
    } else {
        delta
    }
}
pub fn smooth_axis(previous: i32, current: i32, smoothing: i32) -> i32 {
    if smoothing == 0 {
        current
    } else {
        (i64::from(previous) * i64::from(smoothing)
            + i64::from(current) * i64::from(100 - smoothing)) as i32
            / 100
    }
}
pub fn select_ir_point(frame: &IrFrame, tracking: IrTracking) -> Option<IrPoint> {
    let mut points = [IrPoint::default(); 4];
    let mut count = 0usize;
    for point in frame.points {
        if point.valid {
            points[count] = point;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    match tracking {
        IrTracking::First => Some(points[0]),
        IrTracking::Centroid => {
            let (mut x, mut y) = (0i64, 0i64);
            for point in points.iter().take(count) {
                x += i64::from(point.x);
                y += i64::from(point.y);
            }
            Some(IrPoint {
                valid: true,
                x: (x / count as i64) as i32,
                y: (y / count as i64) as i32,
            })
        }
        IrTracking::Dual => {
            if count == 1 {
                return Some(points[0]);
            }
            let (mut a, mut b, mut best) = (0usize, 1usize, -1i64);
            for i in 0..count {
                for j in (i + 1)..count {
                    let dx = i64::from(points[i].x) - i64::from(points[j].x);
                    let dy = i64::from(points[i].y) - i64::from(points[j].y);
                    let distance = dx * dx + dy * dy;
                    if distance > best {
                        best = distance;
                        a = i;
                        b = j;
                    }
                }
            }
            Some(IrPoint {
                valid: true,
                x: (points[a].x + points[b].x) / 2,
                y: (points[a].y + points[b].y) / 2,
            })
        }
    }
}
