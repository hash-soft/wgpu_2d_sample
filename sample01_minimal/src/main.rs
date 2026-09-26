use sample01_minimal::{run_polygon_simple, run_viewport, run_viewport_stretch};

fn main() -> anyhow::Result<()> {
    let default_run_id: i32 = 0;

    // 第1引数（args[1]）を取得。引数が渡されていない場合は None
    let run_id: i32 = match std::env::args().nth(1) {
        Some(arg) => {
            // 1. 数値への変換を試みる
            if let Ok(num) = arg.parse::<i32>() {
                num
            } else {
                // 2. 特定の文字列の場合の判定
                match arg.as_str() {
                    "polygon_simple" => 0,
                    "viewport" => 1,
                    "viewport_strech" => 1, // 作ってはみたもののぼやけて使い物にならない、オフスクリーンに描画して転送する必要がある
                    // 3. それ以外（数値でも特定の文字列でもない）ならデフォルト値
                    _ => default_run_id,
                }
            }
        }
        // 引数自体が指定されていない場合もデフォルト値
        None => default_run_id,
    };

    match run_id {
        0 => run_polygon_simple::run(),
        1 => run_viewport::run(),
        2 => run_viewport_stretch::run(),
        _ => run_polygon_simple::run(),
    }
}
