use crate::tiled_map::TiledMap;
use anyhow::Result;
use std::fs;

#[macro_export]
macro_rules! load_map_static {
    ($path:expr) => {{
        // コンパイル時にJSON文字列をバイナリに埋め込む
        // サンプル時はこちらを使う
        let json_str = include_str!($path);

        // serde_jsonでデシリアライズ
        let tiled_map: crate::tiled_map::TiledMap = serde_json::from_str(json_str)?;
        anyhow::Ok(tiled_map)
    }};
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
