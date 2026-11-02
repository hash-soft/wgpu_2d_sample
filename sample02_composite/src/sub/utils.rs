use anyhow::Result;
use std::fs;
use winit::window;

use crate::sub::tiled_map::TiledMap;

#[macro_export]
macro_rules! load_map_static {
    ($path:expr) => {{
        // コンパイル時にJSON文字列をバイナリに埋め込む
        // サンプル時はこちらを使う
        let json_str = include_str!($path);

        // serde_jsonでデシリアライズ
        let tiled_map: crate::sub::tiled_map::TiledMap = serde_json::from_str(json_str)?;
        anyhow::Ok(tiled_map)
    }};
}

pub struct DeviceParams {
    monitor_size: (u32, u32),
    max_texture_dimension_2d: u32,
    pub target_logical_size: (f32, f32),
}

impl DeviceParams {
    pub fn new(
        monitor_size: (u32, u32),
        max_texture_dimension_2d: u32,
        target_logical_size: (f32, f32),
    ) -> Self {
        Self {
            monitor_size,
            max_texture_dimension_2d,
            target_logical_size,
        }
    }

    pub fn monitor_size(&self) -> (u32, u32) {
        self.monitor_size
    }

    pub fn max_texture_dimension_2d(&self) -> u32 {
        self.max_texture_dimension_2d
    }
}

#[allow(dead_code)]
pub fn load_map_from_file(path: &str) -> Result<TiledMap> {
    // サンプルなのでtomlを基準にしているが実際は実行パスを基準にする
    // また、本関数でパスを補正しないこと
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir).join(String::from("src/") + path);
    println!("{}", path.display());

    // 実行時にファイルを読み込む
    let json_str = fs::read_to_string(path)?;

    // serde_jsonでデシリアライズ
    let tiled_map: TiledMap = serde_json::from_str(&json_str)?;
    Ok(tiled_map)
}

pub fn max_texture_dimension_2d(
    window: &window::Window,
    default_limit: u32,
    device_limit: u32,
) -> u32 {
    let mut limit = default_limit; // wgpuのデフォルト値を基準にする

    // モニターの最大解像度を取得して、大きいほうに合わせる
    // ほんとはウィンドウを監視してモニターが変わったら変化させないといけないけどとりあえず版
    // event_loop.available_monitors()を使用すると全モニター情報が取れる
    if let Some(size) = get_monitor_max_resolution(window) {
        let max = size.0.max(size.1);
        limit = limit.max(max); // 大きいほうに合わせる
    }

    // GPUの物理限界と比較し、小さいほうに合わせる
    limit = limit.min(device_limit);
    limit
}

pub fn get_monitor_max_resolution(window: &window::Window) -> Option<(u32, u32)> {
    if let Some(monitor) = window.current_monitor() {
        let size = monitor.size();
        Some((size.width, size.height))
    } else {
        None
    }
}
