use std::ops;

use crate::sub::{
    frame_renderer::FrameRenderer,
    text_renderer::{TextPaint, TextRenderer},
};

/// frameとtextのrendererを使ってウィンドウを作る
#[derive(Debug)]
struct Window {
    item_id: u32,
    position: (f32, f32),
    size: (f32, f32),
}

// グループは外で管理する
pub struct WindowManager {
    windows: Vec<Window>,
    max: u32,
    item_ranges: Vec<ops::Range<u32>>, // フレームと違い描画順序は自由なので先頭から格納していく
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: vec![],
            max: 50,
            item_ranges: vec![],
        }
    }

    pub fn window_len(&self) -> u32 {
        self.windows.len() as u32
    }

    pub fn item_id(&self, index: u32) -> Option<u32> {
        if let Some(window) = self.windows.get(index as usize) {
            Some(window.item_id)
        } else {
            None
        }
    }

    pub fn item_ranges(&self) -> &Vec<ops::Range<u32>> {
        &self.item_ranges
    }

    pub fn resize_screen(
        &mut self,
        text_renderer: &mut TextRenderer,
        width: u32,
        height: u32,
        logical_size: (f32, f32),
    ) {
        text_renderer.resize(width, height, logical_size);
    }

    pub fn create_window(
        &mut self,
        item_id: u32,
        position: (f32, f32),
        size: (f32, f32),
    ) -> Option<u32> {
        if self.windows.len() >= self.max as usize {
            return None;
        }
        let index = self.windows.len() as u32;
        self.windows.push(Window {
            item_id,
            position,
            size,
        });
        Some(index)
    }

    pub fn pop_window(
        &mut self,
        frame_renderer: &mut FrameRenderer,
        _text_renderer: &mut TextRenderer,
    ) {
        // 必要になったときのために一応
        let _ = if let Some(pop) = self.windows.pop() {
            pop
        } else {
            return;
        };
        frame_renderer.pop_window();
        self.item_ranges.pop();
        // todo :text_rendererからpop分を取り除く
        // キャッシュなので別で行ったほうがいいかも
    }

    pub fn clear(&mut self, text_renderer: &mut TextRenderer) {
        text_renderer.clear();
        self.item_ranges.clear();
    }

    pub fn build_frame(&self, frame_renderer: &mut FrameRenderer, index: u32) {
        let window = if let Some(window) = self.windows.get(index as usize) {
            window
        } else {
            return;
        };
        // zの分子 テキストの分もあるので2倍かつ0.0より大きくないといけないので+1
        let n_num = (index + 1) as f32 * 2.0 - 1.0;
        frame_renderer.push_window(&window.position, &window.size, n_num / 256.0);
    }

    pub fn begin_build_text(&mut self, text_renderer: &TextRenderer) {
        let start = text_renderer.texts_len() as u32;
        self.item_ranges.push(start..start + 1);
    }

    pub fn end_build_text(&mut self, text_renderer: &TextRenderer) {
        if let Some(last) = self.item_ranges.last_mut() {
            // なにも描画しない場合もあるのでstartとendが同じでも追加する必要がある
            last.end = text_renderer.texts_len() as u32;
        }
    }

    pub fn build_text(
        &self,
        text_renderer: &mut TextRenderer,
        index: u32,
        x: f32,
        y: f32,
        text: &str,
        paint: &TextPaint,
    ) {
        let window = if let Some(window) = self.windows.get(index as usize) {
            window
        } else {
            return;
        };

        let left = window.position.0 + 8.0 + x;
        let top = window.position.1 + 8.0 + y;
        let z = (index + 1) as f32 * 2.0 / 256.0;
        text_renderer.draw_text(left, top, z, text, &paint);
    }
}
