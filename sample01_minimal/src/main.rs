use sample01_minimal::{
    run_depth_buffer, run_font_femtovg, run_font_femtovg_off_screen, run_font_glyphon_custom,
    run_font_glyphon_system, run_off_screen_render, run_polygon_simple, run_sprite_tilemap,
    run_texture_bindless, run_texture_filter_blur_screen, run_texture_filter_color,
    run_texture_filter_noise, run_texture_introduction, run_texture_multi_2d_array,
    run_texture_multi_bind_group, run_texture_uniform, run_viewport, run_viewport_stretch,
    run_window,
};

fn main() -> anyhow::Result<()> {
    let default_run_id: i32 = 99;

    // 第1引数（args[1]）を取得。引数が渡されていない場合は None
    let run_id: i32 = match std::env::args().nth(1) {
        Some(arg) => {
            // 数値への変換を試みる
            if let Ok(num) = arg.parse::<i32>() {
                num
            } else {
                // 特定の文字列の場合の判定
                match arg.as_str() {
                    "polygon_simple" => 0,            // 三角形の描画
                    "texture_introduction" => 10,     // 単一のテクスチャ表示
                    "texture_uniform" => 11,          // 同一テクスチャから複数回描画
                    "texture_multi_bind_group" => 12, // BindGropuを複数登録し複数のテクスチャを表示
                    "texture_multi_2d_array" => 13,   // 1回描画で複数のテクスチャを表示
                    "texture_bindless" => 14,         // サイズの異なるテクスチャ配列
                    "viewport" => 20,                 // ビューポート
                    // ビューポートを設定して引き延ばし転送
                    // 作ってはみたもののぼやけて使い物にならない、オフスクリーンに描画して転送する必要がある
                    "viewport_strech" => 21,
                    "off_screen_render" => 30,       // オフスクリーン描画
                    "font_glyphon_system" => 40,     // glyphonでシステムフォント
                    "font_glyphon_custom" => 41,     // glyphonで外部フォント
                    "font_femtovg" => 42,            // femtovgでストローク付き外部フォント
                    "font_femtovg_off_screen" => 43, // femtovgでオフスクリーンにフォントを描画
                    "window" => 50,                  // フレームテクスチャを使ってウィンドウを作る
                    "depth_buffer" => 51,            // 深度バッファを使ってウィンドウの重ね合わせ
                    "texture_filter_noise" => 60,    // テクスチャフィルター：ノイズ
                    "texture_filter_color" => 61,    // テクスチャフィルター：カラー
                    "texture_filter_blur_screen" => 62, // テクスチャフィルター：ぼかし
                    "sprite_tilemap" => 70, // スプライトをタイルマップのようにならべだけのもの
                    // それ以外（数値でも特定の文字列でもない）ならデフォルト値
                    _ => default_run_id,
                }
            }
        }
        // 引数自体が指定されていない場合もデフォルト値
        None => default_run_id,
    };

    match run_id {
        0 => run_polygon_simple::run(),
        10 => run_texture_introduction::run(),
        11 => run_texture_uniform::run(),
        12 => run_texture_multi_bind_group::run(),
        13 => run_texture_multi_2d_array::run(),
        14 => run_texture_bindless::run(),
        20 => run_viewport::run(),
        21 => run_viewport_stretch::run(),
        30 => run_off_screen_render::run(),
        40 => run_font_glyphon_system::run(),
        41 => run_font_glyphon_custom::run(),
        42 => run_font_femtovg::run(),
        43 => run_font_femtovg_off_screen::run(),
        50 => run_window::run(),
        51 => run_depth_buffer::run(),
        60 => run_texture_filter_noise::run(),
        61 => run_texture_filter_color::run(),
        62 => run_texture_filter_blur_screen::run(),
        70 => run_sprite_tilemap::run(),
        _ => run_font_femtovg_off_screen::run(),
    }
}
