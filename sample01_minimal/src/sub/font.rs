use glyphon::FontSystem;

/// フォント読み込み
/// 存在しないweightを指定した場合、フォントがデフォルトになるので
/// weightリストを作る
/// また、フォントのloadが成功したかも追加された数もわからないので
/// 読み込み前後の数の差で判断する
/// wghtに対応しているかは簡単にはわからないので対応はあきらめる
#[derive(Debug)]
pub struct CustomFont {
    pub family: String,
    // 複数のWeightを持つ可能性があるため配列で保持する
    pub available_weights: Vec<glyphon::Weight>,
}
impl CustomFont {
    pub fn load_font(font_system: &mut FontSystem, data: Vec<u8>) -> Option<Self> {
        // 読み込む前のフォントの総数を記憶
        let faces_before = font_system.db().faces().count();

        // フォントデータを登録（複数ウェイトがあれば複数追加される）
        font_system.db_mut().load_font_data(data);

        // 増えた分（faces_before 以降）のイテレータを取得
        let new_faces = font_system.db().faces().skip(faces_before);

        let mut family = String::new();
        let mut available_weights = Vec::new();
        for face in new_faces {
            if family.is_empty() {
                // 単一ファミリー前提
                if let Some((name, _)) = face.families.first() {
                    family = name.to_string();
                }
            }
            // 含まれるウェイトをすべてリストアップ
            available_weights.push(face.weight);
        }
        if family.is_empty() {
            return None; // 読み込み失敗
        }

        // ソートだけしておく
        available_weights.sort();

        Some(Self {
            family,
            available_weights,
        })
    }

    pub fn load_system_font(font_system: &FontSystem, target_family: &str) -> Option<Self> {
        let mut family = String::new();
        let mut available_weights = Vec::new();

        for face in font_system.db().faces() {
            // ファミリー名の中に目的のフォント名が含まれているか確認
            let matches = face.families.iter().any(|(name, _)| name == target_family);
            if matches {
                if family.is_empty() {
                    family = target_family.to_string();
                }
                available_weights.push(face.weight);
            }
        }

        if family.is_empty() {
            return None; // 該当するシステムフォントが見つからない
        }

        // 重複するウェイトをソート＆排除
        available_weights.sort();
        available_weights.dedup();

        Some(CustomFont {
            family,
            available_weights,
        })
    }

    /// 指定した Weight に最も近い利用可能な Weight を返す
    pub fn get_closest_weight(&self, target: glyphon::Weight) -> glyphon::Weight {
        // fontdb::Weight は内部に u16 の数値（0〜1000など）を持つため .0 でアクセス可能
        let target_val = target.0 as i32;

        // 差の絶対値が最小のものを探す
        self.available_weights
            .iter()
            .min_by_key(|&&w| (w.0 as i32 - target_val).abs())
            .copied()
            .unwrap_or(glyphon::Weight::default())
    }
}
