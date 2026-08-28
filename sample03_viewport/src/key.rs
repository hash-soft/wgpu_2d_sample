use winit::keyboard::KeyCode;

#[derive(Default)]
pub struct InputState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
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
