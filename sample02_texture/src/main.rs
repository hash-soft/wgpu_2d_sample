mod key;
mod run_bind_group_multi_entry;
mod run_introduction;
mod run_multi_2d_array;
mod run_multi_bind_group;
mod run_uniform;
mod sprite;
mod texture;
fn main() -> anyhow::Result<()> {
    let default_run_id: i32 = 99;

    // 第1引数（args[1]）を取得。引数が渡されていない場合は None
    let run_id: i32 = match std::env::args().nth(1) {
        Some(arg) => {
            // 1. 数値への変換を試みる
            if let Ok(num) = arg.parse::<i32>() {
                num
            } else {
                // 2. 特定の文字列の場合の判定
                match arg.as_str() {
                    "introduction" => 0,           // 固定位置に表示
                    "uniform" => 1,                // 同一テクスチャから複数回描画
                    "multi_bind_group" => 2,       // BindGropuを複数登録し複数のテクスチャを表示
                    "2d_array" => 3,               // 1回描画で複数のテクスチャを表示
                    "bing_group_multi_entry" => 4, // 1つのBindGroup内で複数のテクスチャを登録
                    "tilemap" => 5,
                    // 3. それ以外（数値でも特定の文字列でもない）ならデフォルト値
                    _ => default_run_id,
                }
            }
        }
        // 引数自体が指定されていない場合もデフォルト値
        None => default_run_id,
    };

    match run_id {
        0 => run_introduction::run(),
        1 => run_uniform::run(),
        2 => run_multi_bind_group::run(),
        3 => run_multi_2d_array::run(),
        _ => run_bind_group_multi_entry::run(),
    }
}
