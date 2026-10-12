use winit::keyboard::KeyCode;

#[derive(Default)]
pub struct InputState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    pub plus: bool,
    pub minus: bool,
    pub reset: bool,
    pub rotation_l: bool,
    pub rotation_r: bool,
}

impl InputState {
    pub fn process_key(&mut self, key: KeyCode, is_pressed: bool) -> bool {
        match key {
            KeyCode::ArrowUp | KeyCode::KeyW => {
                self.up = is_pressed;
                true
            }
            KeyCode::ArrowDown | KeyCode::KeyS => {
                self.down = is_pressed;
                true
            }
            KeyCode::ArrowLeft | KeyCode::KeyA => {
                self.left = is_pressed;
                true
            }
            KeyCode::ArrowRight | KeyCode::KeyD => {
                self.right = is_pressed;
                true
            }
            KeyCode::KeyQ => {
                self.plus = is_pressed;
                true
            }
            KeyCode::KeyE => {
                self.minus = is_pressed;
                true
            }
            KeyCode::Space => {
                self.reset = is_pressed;
                true
            }
            KeyCode::KeyL => {
                self.rotation_l = is_pressed;
                true
            }
            KeyCode::KeyR => {
                self.rotation_r = is_pressed;
                true
            }
            _ => false,
        }
    }

    /// 現在の入力に応じた移動ベクトル (dx, dy) を返す
    pub fn direction(&self) -> [f32; 2] {
        let mut dx = 0.0;
        let mut dy = 0.0;
        if self.left {
            dx -= 1.0;
        }
        if self.right {
            dx += 1.0;
        }
        if self.up {
            dy -= 1.0;
        }
        if self.down {
            dy += 1.0;
        }
        [dx, dy]
    }
}
