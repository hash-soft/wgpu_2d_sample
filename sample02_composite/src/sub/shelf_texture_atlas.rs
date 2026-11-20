use ahash::AHashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AtlasHandle(Rect);

impl AtlasHandle {
    pub fn x(&self) -> u16 {
        self.0.x
    }

    pub fn y(&self) -> u16 {
        self.0.y
    }

    pub fn width(&self) -> u16 {
        self.0.width
    }

    pub fn height(&self) -> u16 {
        self.0.height
    }
}

#[derive(Debug, Clone, Copy)]
struct FreeSlot {
    x: u16,
    width: u16,
}

#[derive(Debug, Clone)]
struct Shelf {
    y: u16,
    height: u16,
    free_slots: Vec<FreeSlot>,
    allocated_items: AHashSet<AtlasHandle>,
}

impl Shelf {
    fn new(y: u16, height: u16, total_width: u16) -> Self {
        Self {
            y,
            height,
            free_slots: vec![FreeSlot {
                x: 0,
                width: total_width,
            }],
            allocated_items: AHashSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.allocated_items.is_empty()
    }

    pub fn can_fit_width(&self, width: u16) -> bool {
        self.free_slots.iter().any(|slot| slot.width >= width)
    }

    pub fn allocate(&mut self, width: u16, height: u16) -> Option<AtlasHandle> {
        if height > self.height {
            return None;
        }

        // First-Fit: 収まる最初の空きスロットを探す
        for i in 0..self.free_slots.len() {
            let slot = &self.free_slots[i];
            if slot.width >= width {
                let rect = Rect {
                    x: slot.x,
                    y: self.y,
                    width,
                    height,
                };
                if slot.width == width {
                    self.free_slots.remove(i);
                } else {
                    self.free_slots[i].x += width;
                    self.free_slots[i].width = self.free_slots[i].width.saturating_sub(width);
                }
                let handle = AtlasHandle(rect);
                self.allocated_items.insert(handle.clone());
                return Some(handle);
            }
        }

        None
    }

    pub fn free(&mut self, handle: &AtlasHandle) {
        let exist = self.allocated_items.remove(handle);
        if exist {
            self.insert_or_marge_free_slot(FreeSlot {
                x: handle.0.x,
                width: handle.0.width,
            });
        }
    }

    /// 空きスロットを追加するか統合する
    /// 左右両方調べて統合できなければ空きを追加する
    /// todo: x座標で判断しているからソートしてなくてもいい気がするな
    fn insert_or_marge_free_slot(&mut self, slot: FreeSlot) {
        // 左を統合
        // 空きを更新したら削除して統合したものを返す、しなかったらもともとの空きを返す
        let left = self.free_slots.iter().position(|s| s.x + s.width == slot.x);
        let slot = if let Some(i) = left {
            let mut marge_slot = self.free_slots.remove(i);
            marge_slot.width += slot.width;
            marge_slot
        } else {
            slot
        };
        // 右を統合
        let right = self
            .free_slots
            .iter()
            .position(|s| s.x == slot.x + slot.width);
        if let Some(i) = right {
            self.free_slots[i].x = slot.x;
            self.free_slots[i].width += slot.width;
        } else {
            self.free_slots.push(slot);
        };
    }
}

#[derive(Debug, Clone)]
pub struct ShelfAtlas {
    y: u16, // 使用開始する高さ x は 0 で固定
    width: u16,
    height: u16,
    shelves: Vec<Shelf>,
}

impl ShelfAtlas {
    pub fn new(y: u16, width: u16, height: u16) -> Self {
        Self {
            y: y,
            width,
            height,
            shelves: vec![],
        }
    }

    #[allow(unused)]
    pub fn clear(&mut self) {
        self.shelves.clear();
    }

    #[allow(unused)]
    pub fn shelves_len(&self) -> usize {
        self.shelves.len()
    }

    #[allow(unused)]
    pub fn item_len(&self) -> usize {
        self.shelves.iter().map(|s| s.allocated_items.len()).sum()
    }

    #[allow(unused)]
    pub fn get(&self, handle: &AtlasHandle) -> Option<AtlasHandle> {
        for shelf in &self.shelves {
            if let Some(handle) = shelf.allocated_items.get(handle) {
                return Some(handle.clone());
            }
        }
        None
    }

    pub fn allocate(&mut self, width: u16, height: u16) -> Option<AtlasHandle> {
        if width > self.width || height > self.height {
            return None;
        }
        let mut found: Option<usize> = None;
        let mut suggest: Option<usize> = None;
        let mut suggest_height = 0;
        for (i, shelf) in &mut self.shelves.iter_mut().enumerate() {
            if shelf.can_fit_width(width) {
                // shelfの高さが半分を超えていたら確定
                if height > shelf.height / 2 {
                    found = Some(i);
                    break;
                }
                // 今までより高いshelfがあれば候補を更新
                if shelf.height > suggest_height {
                    suggest = Some(i);
                    suggest_height = shelf.height;
                }
            }
        }

        if let Some(i) = found {
            return self.shelves[i].allocate(width, height);
        }

        // 新しいshelfを作って追加する
        let mut current_y = self.y;
        for (i, shelf) in &mut self.shelves.iter_mut().enumerate() {
            let next_y = shelf.y;
            let empty_height = next_y.saturating_sub(current_y);
            current_y = next_y + shelf.height;
            if empty_height >= height {
                self.shelves
                    .insert(i, Shelf::new(current_y, height, self.width));
                return self.shelves[i].allocate(width, height);
            }
        }
        // 最後の下の部分を確認
        let last_empty_height = self.height.saturating_sub(current_y);
        if last_empty_height >= height {
            self.shelves.push(Shelf::new(current_y, height, self.width));
            if let Some(shelf) = self.shelves.last_mut() {
                return shelf.allocate(width, height);
            }
        }

        // 追加できなければ追加可能な場所に追加する
        if let Some(i) = suggest {
            return self.shelves[i].allocate(width, height);
        }

        None
    }

    /// shelfを空にして空になっていれば削除するので統合は不要
    /// indexが合わなくなるからインデックスでの取得はできないな
    /// 統合したところでインデックスずれるからインデックス以外を取得条件にする必要がある
    pub fn free(&mut self, handle: &AtlasHandle) {
        if let Some(i) = self
            .shelves
            .iter()
            .position(|s| s.allocated_items.contains(handle))
        {
            self.shelves[i].free(handle);
            if self.shelves[i].is_empty() {
                self.shelves.remove(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn test_shelf_allocate() {
        let mut atlas = Shelf::new(0, 100, 100);
        assert_eq!(
            atlas.allocate(10, 10),
            Some(AtlasHandle(Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10
            }))
        );
        assert_eq!(
            atlas.allocate(10, 10),
            Some(AtlasHandle(Rect {
                x: 10,
                y: 0,
                width: 10,
                height: 10
            }))
        );
        assert_eq!(
            atlas.allocate(10, 8),
            Some(AtlasHandle(Rect {
                x: 20,
                y: 0,
                width: 10,
                height: 8
            }))
        );
        assert_eq!(atlas.allocate(80, 10), None);
    }

    #[test]
    fn test_shelf_free() {
        let mut atlas = Shelf::new(0, 100, 100);
        let handle = atlas.allocate(10, 10);
        atlas.allocate(10, 10);
        atlas.allocate(10, 8);
        atlas.free(&handle.unwrap());
        assert_eq!(atlas.allocated_items.len(), 2);
    }

    #[test]
    fn test_shelf_free2() {
        let mut atlas = Shelf::new(0, 100, 100);
        let handle1 = atlas.allocate(10, 10).unwrap();
        let handle2 = atlas.allocate(10, 10).unwrap();
        let handle3 = atlas.allocate(10, 8).unwrap();
        atlas.free(&handle1);
        assert_eq!(atlas.allocated_items.len(), 2);
        atlas.free(&handle2);
        assert_eq!(atlas.allocated_items.len(), 1);
        atlas.free(&handle3);
        assert_eq!(atlas.allocated_items.len(), 0);
    }

    #[test]
    fn test_shelf_altas_allocate() {
        let mut atlas = ShelfAtlas::new(0, 100, 100);
        assert_eq!(
            atlas.allocate(10, 10),
            Some(AtlasHandle(Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 10
            }))
        );
        assert_eq!(
            atlas.allocate(10, 10),
            Some(AtlasHandle(Rect {
                x: 10,
                y: 0,
                width: 10,
                height: 10
            }))
        );
        assert_eq!(
            atlas.allocate(10, 8),
            Some(AtlasHandle(Rect {
                x: 20,
                y: 0,
                width: 10,
                height: 8
            }))
        );
        // shelfの数
        assert_eq!(atlas.shelves.len(), 1);
        assert_eq!(
            atlas.allocate(80, 10),
            Some(AtlasHandle(Rect {
                x: 0,
                y: 10,
                width: 80,
                height: 10
            }))
        );
        assert_eq!(atlas.shelves.len(), 2);
    }

    /// 100*100のSelfAtlasを15*15のサイズで埋めて使いきり、埋まったら2つ目のSelfAtlasを作る
    /// 2つ目が埋まったらそこで終了
    /// - 埋まったかどうかは戻り値で判断する
    #[test]
    fn test_shelf_altas_allocate_full() {
        let start = Instant::now();
        let mut atlases = vec![];
        atlases.push(ShelfAtlas::new(0, 100, 100));
        let mut i = 0;
        loop {
            let result = atlases[i].allocate(15, 15);
            if result.is_none() {
                i += 1;
                if i == 2 {
                    break;
                }
                atlases.push(ShelfAtlas::new(0, 100, 100));
            }
        }
        println!("rust 実行時間: {:?}", start.elapsed());
        assert_eq!(atlases[0].shelves.len(), 100 / 15);
        assert_eq!(atlases[1].shelves.len(), 100 / 15);
    }

    #[test]
    fn test_shelf_altas_free() {
        let start = Instant::now();
        let mut atlas = ShelfAtlas::new(0, 100, 100);
        let handle1 = atlas.allocate(10, 10).unwrap();
        let handle2 = atlas.allocate(10, 10).unwrap();
        let handle3 = atlas.allocate(10, 8).unwrap();
        let handle4 = atlas.allocate(80, 10).unwrap();
        atlas.free(&handle1);
        assert_eq!(atlas.shelves.len(), 2);
        atlas.free(&handle2);
        assert_eq!(atlas.shelves.len(), 2);
        atlas.free(&handle3);
        assert_eq!(atlas.shelves.len(), 1);
        atlas.free(&handle4);
        assert_eq!(atlas.shelves.len(), 0);
        println!("rust 実行時間: {:?}", start.elapsed());
    }
}
