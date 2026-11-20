#[derive(Debug)]
pub struct DataWindow {
    pub position: (f32, f32),
    pub size: (f32, f32),
    pub texts: Vec<String>,
    pub font_size: u16,
    pub height: u16,
}

pub struct Rom {
    pub windows: Vec<DataWindow>,
}

impl Rom {
    pub fn new() -> Self {
        Self {
            windows: vec![
                DataWindow {
                    position: (120.0, 20.0),
                    size: (36.0 * 5.0, 200.0),
                    texts: vec![
                        "じゅもん".to_string(),
                        "どうぐ".to_string(),
                        "つよさ".to_string(),
                        "さくせん".to_string(),
                    ],
                    font_size: 36,
                    height: 40,
                },
                DataWindow {
                    position: (228.0, 20.0),
                    size: (36.0 * 7.0, 350.0),
                    texts: vec![
                        "ウィト".to_string(),
                        "ウイデア".to_string(),
                        "トレパー".to_string(),
                        "スキリト".to_string(),
                        "ブレスア".to_string(),
                        "タスパオ".to_string(),
                        "ラトクス".to_string(),
                    ],
                    font_size: 36,
                    height: 40,
                },
            ],
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Scene {
    Map,
    Menu,
}
pub struct Ram {
    scene: Scene,
    pub next_scene: Option<Scene>,
    pub start_menu_index: Option<u32>,
    pub current_menu_index: u32,
}

impl Ram {
    pub fn new() -> Self {
        Self {
            scene: Scene::Map,
            next_scene: None,
            start_menu_index: None,
            current_menu_index: 0,
        }
    }

    pub fn scene(&self) -> Scene {
        self.scene
    }

    pub fn apply_next_scene(&mut self) {
        if let Some(scene) = self.next_scene {
            self.scene = scene;
            self.next_scene = None;
        }
    }
}
