use sample02_composite::{
    run_load_tilemap, run_move_character, run_tilemap_instanced, run_tilemap_multi_indices,
};

fn main() -> anyhow::Result<()> {
    let default_run_id: i32 = 99;

    // 追加予定
    // タイルマップを背景にしてマルチウィンドウ処理を行う

    // 第1引数（args[1]）を取得。引数が渡されていない場合は None
    let run_id: i32 = match std::env::args().nth(1) {
        Some(arg) => {
            // 数値への変換を試みる
            if let Ok(num) = arg.parse::<i32>() {
                num
            } else {
                // 特定の文字列の場合の判定
                match arg.as_str() {
                    "move_character" => 0, // キャラチップ移動
                    "load_tilemap" => 1,   // タイルマップ読み込み

                    "tilemap_multi_indices" => 10, // 未精査
                    "tilemap_instanced" => 11,     // 未精査
                    // それ以外（数値でも特定の文字列でもない）ならデフォルト値
                    _ => default_run_id,
                }
            }
        }
        // 引数自体が指定されていない場合もデフォルト値
        None => default_run_id,
    };

    println!("run_id: {}", run_id);

    match run_id {
        0 => run_move_character::run(),
        1 => run_load_tilemap::run(),

        10 => run_tilemap_multi_indices::run(),
        11 => run_tilemap_instanced::run(),
        _ => run_move_character::run(),
    }
}
