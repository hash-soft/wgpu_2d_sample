use std::ops;

use ahash::AHashMap;
use femtovg::{Canvas, Color, FontId, Paint, renderer};
use wgpu::util::DeviceExt;

use crate::sub::{
    shelf_texture_atlas::{AtlasHandle, ShelfAtlas},
    texture::Texture,
};

/// # どういう使い方をするか
/// - drawで指定した文字や位置をそのまま表示できる
/// - 同じ情報を渡せば削除も可能
/// - システム文字は固定で作成しておきインデックスでdrawできる
/// - こちらは削除不可
/// - draw_text、draw_indexのような感じ
/// - 転送範囲を指定することもできる（文字の一部分だけ表示）
/// - drawでパラメータ指定が必要
/// - 最大と現在値を区切る斜め線や横線も必要
/// - 必須ではないがルビもあればいい
/// - 文章を小分けにしないと位置がわからないがそれをどうするか
///
/// # draw_text
/// - テクスチャアトラスに描画＋インスタンスに追加
/// - 位置情報は描画情報をキーにしたmapに保持する
/// - mapにヒットした場合はインスタンスの追加だけを行う
///
/// # draw_index
/// - 事前にテクスチャアトラスに描画しておく必要がある
/// - 位置情報はvecで保持しておく
/// - テクスチャアトラスの位置を取得＋インスタンスに追加
///
/// # instanceの使い方
/// - 追加はpushすればいいだけなので問題なし
/// - 問題は不要になったinstanceは削除か描画されない値にする必要があるがどう見つけるか
/// - instanceは描画情報しかないので不要かどうかわからない
/// - drawの時にinstanceのインデックスをOptionで返すようにするか（失敗ならNone）
/// - instanceはvec形式でしか持てないので順次検索をする必要があるが毎回先頭からだと効率が悪い
/// - 削除インデックスリストで管理するか
///
/// # テクスチャアトラス
/// - システム文字、動的文字で分けない
/// - 動的文字はシステム文字の後ろに追加する
/// - アトラス管理システムが必要
/// - これの削除とインスタンスは連動させない（使用側で管理）
/// - 領域がなくなった場合はテクスチャを増やす
/// - 同じサイズを増やすので2d配列のシェーダーが使用できると思ったが拡張できなくて全テクスチャ作り直しになるので実質できない
///
/// # まずは毎回draw方式で実装してみる ⇒ これはできた
/// - なのでフレーム事にtextsをclearから始める
/// - clear ⇒ 描画分draw ⇒ write_buffer
///
/// # todo textだけじゃなくてウィンドウに限らない項目を構成するものだからwindow_itemに名前を変える
///
/// # todo 色を塗る機能も追加
/// - color系はインデックスで持ち texture_index でテクスチャか色塗りかを分ける
/// - テクスチャは mat4とoffset、色塗りはvec4 のインデックスから値を引く
///
/// # todo 薄い色をベースに濃い色を上昇させる機能も追加
/// - colorと同様 texture_index で分ける
/// - テクスチャと色塗に比べて情報量が多いのでどうするか
/// - インデックスで持つにしてもオフセットは別で必要だから最低1つは変数の追加が必要
///

#[derive(Debug)]
pub struct TextPaint {
    pub font_index: u16,
    pub font_size: u16,
    pub font_weight: u16,
    pub line_width: u8,
    pub color: [u8; 4],
    pub line_color: [u8; 4],
}

/// ahashのため整数と文字列に限定
#[derive(Hash, Eq, PartialEq, Debug, Clone)]
struct CacheKey {
    pub text: String,
    pub font_index: u16,
    pub font_size: u16,
    pub font_weight: u16,
    pub line_width: u8,
    pub color: [u8; 4],
    pub line_color: [u8; 4],
}

#[derive(Debug)]
struct Glyph {
    atlas_handle: AtlasHandle,
    pub texture_index: u32,
    pub uv_offset: [f32; 2],
    pub uv_size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextUniforms {
    logical_size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextInstance {
    pub position: [f32; 2],  // 表示ピクセル位置
    pub size: [f32; 2],      // 表示ピクセルサイズ
    pub uv_offset: [f32; 2], // 切り出しUVオフセット
    pub uv_size: [f32; 2],   // 切り出しUVサイズ
    pub texture_index: u32,
    pub opacity: f32, // 透過度
    pub z: f32,
    pub color_multiply: [f32; 4],
    pub color_add: [f32; 4],
}

impl TextInstance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // location(0): position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(1): size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(2): uv_offset
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(3): uv_size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // location(4): texture_index
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
                // location(5): opacity
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
                // location(6): z
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32,
                },
                // location(7): color_multiply
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // location(8): color_add
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 15]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[derive(Debug)]
struct Atlas {
    scale: (f32, f32),
    atlas_index: usize,
    shelf_atlases: Vec<ShelfAtlas>,
}

impl Default for Atlas {
    fn default() -> Self {
        Self {
            scale: (1.0, 1.0),
            atlas_index: 0,
            shelf_atlases: Vec::new(),
        }
    }
}

impl Atlas {
    pub fn set_scale(&mut self, scale_x: f32, scale_y: f32) {
        self.scale = (scale_x, scale_y);
    }

    pub fn reset(&mut self, texture_dimension: u32) {
        let texture_dimension = texture_dimension as f32;
        let logical_texture_width = (texture_dimension / self.scale.0).floor();
        let logical_texture_height = (texture_dimension / self.scale.1).floor();
        self.shelf_atlases.clear();
        self.shelf_atlases.push(ShelfAtlas::new(
            0,
            logical_texture_width as u16,
            logical_texture_height as u16,
        ));
    }

    pub fn draw_canvas(
        &mut self,
        canvas: &mut Canvas<renderer::WGPURenderer>,
        texture_dimension: u32,
        font_id: FontId,
        text: &str,
        paint: &TextPaint,
    ) -> Option<Glyph> {
        // 描画対象のアトラスが用意されていない場合は終了
        if self.atlas_index >= self.shelf_atlases.len() {
            return None;
        }
        let line_width = paint.line_width as f32;
        let fill_paint = Paint::color(Color::rgba(
            paint.color[0],
            paint.color[1],
            paint.color[2],
            paint.color[3],
        ))
        .with_font(&[font_id])
        .with_font_weight(paint.font_weight as f32)
        .with_font_size(paint.font_size as f32)
        .with_line_join(femtovg::LineJoin::Round) // トゲ飛び出し防止
        .with_text_baseline(femtovg::Baseline::Top)
        .with_text_align(femtovg::Align::Left);
        // todo: top -> 文字によって縦幅が違うから調整が必要かもしれない
        // left -> 転送位置で調整すればいいからLeftだけでいいが左の位置は切り上げ前のものを使う必要がある
        // それぞれglyphに持たせておくか

        let pad = line_width / 2.0;
        let texture_dimension = texture_dimension as f32;
        // 描画位置の更新
        let handle = if let Ok(metrics) = canvas.measure_text(0.0, 0.0, text, &fill_paint) {
            // 小数点以下を切り上げて整数にする
            let (width, height) = (
                metrics.width().ceil() + line_width,
                metrics.height().ceil() + line_width,
            );

            if let Some(handle) =
                self.shelf_atlases[self.atlas_index].allocate(width as u16, height as u16)
            {
                handle
            } else {
                return None;
            }
        } else {
            return None;
        };

        let new_x = handle.x() as f32;
        let new_y = handle.y() as f32;
        let new_width = handle.width() as f32;
        let new_height = handle.height() as f32;

        // {
        //     // 背景を塗りつぶしたいとき
        //     let mut path = femtovg::Path::new();
        //     path.rect(self.last_x, self.last_y, self.last_width, self.last_height);
        //     let back_paint = Paint::color(Color::rgb(0, 255, 0)); // 背景色
        //     canvas.fill_path(&path, &back_paint);
        // }
        let x = new_x + pad;
        let y = new_y + pad;
        // ストロークがあれば先に描画する
        if line_width > 0.0 {
            let stroke_paint = fill_paint
                .clone()
                .with_color(Color::rgba(
                    paint.line_color[0],
                    paint.line_color[1],
                    paint.line_color[2],
                    paint.line_color[3],
                ))
                .with_line_width(line_width);

            let _ = canvas.stroke_text(x, y, text, &stroke_paint);
        }
        let _ = canvas.fill_text(x, y, text, &fill_paint);

        let tex_actual_x = new_x * self.scale.0;
        let tex_actual_y = new_y * self.scale.1;
        let tex_actual_w = new_width * self.scale.0;
        let tex_actual_h = new_height * self.scale.1;
        let uv_offset = [
            tex_actual_x / texture_dimension,
            tex_actual_y / texture_dimension,
        ];
        let uv_size = [
            tex_actual_w / texture_dimension,
            tex_actual_h / texture_dimension,
        ];
        Some(Glyph {
            atlas_handle: handle,
            texture_index: 0, // 参照atlas_shelfのインデックスになる
            uv_offset,
            uv_size,
        })
    }

    pub fn clear_canvas(&mut self, canvas: &mut Canvas<renderer::WGPURenderer>, glyph: &Glyph) {
        let x = glyph.atlas_handle.x() as u32;
        let y = glyph.atlas_handle.y() as u32;
        let width = glyph.atlas_handle.width() as u32;
        let height = glyph.atlas_handle.height() as u32;
        let index = glyph.texture_index as usize;
        self.shelf_atlases[index].free(&glyph.atlas_handle);
        canvas.clear_rect(x, y, width, height, Color::rgba(0, 0, 0, 0));
    }
}

static MAX_TEXT: usize = 100;
/// 後でクレート依存を分離する
#[derive(Debug)]
pub struct TextRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline, // 転送用パイプライン
    texture_dimension: u32,
    canvas: Canvas<renderer::WGPURenderer>, // femtovg
    uniform_bind_group: wgpu::BindGroup,
    textures: Vec<Texture>,
    texture_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    sampler_bind_group: wgpu::BindGroup,
    texts: Vec<TextInstance>,
    text_instance_buffer: wgpu::Buffer,
    max_text: usize,
    font_ids: Vec<FontId>,
    cache: AHashMap<CacheKey, Glyph>,
    atlas: Atlas,
}

impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        logical_size: (f32, f32),
        texture_dimension: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        // rendererに渡したdeviceとqueueはrenderer内部でしか使えないので別に保持しておく必要がある
        let renderer = renderer::WGPURenderer::new(device.clone(), queue.clone());
        let canvas = Canvas::new(renderer).expect("Cannot create canvas");
        let logical_size = [logical_size.0, logical_size.1];

        // グローバル
        let uniform_bind_group_layout = Self::create_uniform_bind_group_layout(&device);
        let uniform_buffer = Self::create_uniform_buffer(&device, &logical_size);
        let uniform_bind_group =
            Self::create_uniform_bind_group(&device, &uniform_bind_group_layout, &uniform_buffer);

        // テクスチャー
        let texture_bind_group_layout = Self::create_texture_bind_group_layout(&device);
        // ２のべき乗にする
        let texture_dimension = texture_dimension.next_power_of_two();
        // バックバッファを１枚つくる（あふれる場合は後で拡張）
        // bind_groupの作り直しも必要になる
        let texture = Self::create_backbuffer(&device, texture_dimension, format);
        let textures: Vec<Texture> = vec![texture];
        let texture_bind_group =
            Self::create_texture_bind_group(&device, &texture_bind_group_layout, &textures);

        // サンプラー
        let sampler_bind_group_layout = Self::create_sampler_bind_group_layout(&device);
        let sampler = Texture::create_sampler(&device);
        let sampler_bind_group =
            Self::create_sampler_bind_group(&device, &sampler_bind_group_layout, &sampler);

        // パイプライン
        let render_pipeline = Self::create_pipeline(
            &device,
            format,
            &texture_bind_group_layout,
            &sampler_bind_group_layout,
        );

        // テキストのインスタンス
        let max_text = MAX_TEXT; // とりあえず100個
        let texts = Vec::new();
        let text_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Instance Buffer"),
            size: (std::mem::size_of::<TextInstance>() * max_text) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false, // 作成時には書き込まない
        });

        Self {
            device: device.clone(),
            queue: queue.clone(),
            render_pipeline,
            texture_dimension,
            canvas,
            uniform_bind_group,
            textures,
            texture_bind_group,
            sampler,
            sampler_bind_group,
            texts,
            max_text,
            text_instance_buffer,
            font_ids: Vec::new(),
            cache: AHashMap::new(),
            atlas: Atlas::default(),
        }
    }

    #[allow(dead_code)]
    pub fn print_text_size(&self, text: &str, paint: &TextPaint) {
        let fill_paint = Paint::color(Color::rgba(
            paint.color[0],
            paint.color[1],
            paint.color[2],
            paint.color[3],
        ))
        .with_font(&[self.font_ids[paint.font_index as usize]])
        .with_font_weight(paint.font_weight as f32)
        .with_font_size(paint.font_size as f32)
        .with_line_join(femtovg::LineJoin::Round) // トゲ飛び出し防止
        .with_text_baseline(femtovg::Baseline::Top)
        .with_text_align(femtovg::Align::Left);
        if let Ok(metrics) = self.canvas.measure_text(0.0, 0.0, text, &fill_paint) {
            println!(
                "font_size:{} width:{} height:{}",
                paint.font_size,
                metrics.width(),
                metrics.height()
            );
        } else {
            println!("print_text_size {} error", text);
        }
    }

    pub fn texture_dimension(&self) -> u32 {
        self.texture_dimension
    }

    pub fn textures(&self) -> &Vec<Texture> {
        &self.textures
    }

    pub fn texts_len(&self) -> usize {
        self.texts.len()
    }

    /// フォントを追加
    /// 現在外部フォントしか使えない
    pub fn add_font(&mut self, font_bytes: &[u8]) {
        let font_id = self
            .canvas
            .add_font_mem(font_bytes)
            .expect("Cannot add font");
        self.font_ids.push(font_id);
    }

    pub fn render(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        ranges: Option<&Vec<ops::Range<u32>>>,
    ) {
        let range = if let Some(ranges) = ranges {
            if let Some(last) = ranges.last() {
                ranges[0].start..last.end
            } else {
                return;
            }
        } else {
            0..self.texts.len() as u32
        };
        // rangeで確認すればtextsの空を確認する必要はない
        if range.is_empty() {
            return;
        }

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
        render_pass.set_bind_group(2, &self.sampler_bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.text_instance_buffer.slice(..));
        render_pass.draw(0..4, range);
    }

    pub fn resize(&mut self, width: u32, height: u32, logical_size: (f32, f32)) {
        // 初期論理サイズに対するスケール倍率を計算
        let scale_x = width as f32 / logical_size.0;
        let scale_y = height as f32 / logical_size.1;

        self.canvas.reset_transform(); // 累積を防ぐためスケールをリセット
        self.canvas.scale(scale_x, scale_y); // 全体をスケーリング

        // flush_to_targetに合わせたサイズ
        self.canvas
            .set_size(self.texture_dimension, self.texture_dimension, 1.0);

        self.atlas.set_scale(scale_x, scale_y);
        self.atlas.reset(self.texture_dimension);
    }

    pub fn clear(&mut self) {
        self.texts.clear();
        self.cache.clear();
        self.atlas.reset(self.texture_dimension);
    }

    pub fn draw_text(&mut self, x: f32, y: f32, z: f32, text: &str, paint: &TextPaint) {
        if self.texts.len() >= self.max_text {
            return;
        }
        // キャッシュキーを組み立てる
        let key = CacheKey {
            text: text.to_string(),
            font_index: paint.font_index,
            font_size: paint.font_size,
            font_weight: paint.font_weight,
            line_width: paint.line_width,
            color: paint.color,
            line_color: paint.line_color,
        };

        let glyph = match self.cache.entry(key) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                // ヒットしなかった場合、drawを行う
                let glyph = self.atlas.draw_canvas(
                    &mut self.canvas,
                    self.texture_dimension,
                    self.font_ids[paint.font_index as usize],
                    text,
                    paint,
                );
                if let Some(glyph) = glyph {
                    entry.insert(glyph)
                } else {
                    return;
                }
            }
        };

        let width = glyph.atlas_handle.width() as f32;
        let height = glyph.atlas_handle.height() as f32;
        // last_width, last_heightの部分
        let pad = paint.line_width as f32 / 2.0;
        // instanceを追加する
        let instance = TextInstance {
            position: [x - pad, y - pad],
            size: [width, height],
            uv_offset: glyph.uv_offset,
            uv_size: glyph.uv_size,
            texture_index: glyph.texture_index,
            opacity: 1.0,
            z,
            color_multiply: [1.0, 1.0, 1.0, 1.0],
            color_add: [0.0, 0.0, 0.0, 0.0],
        };
        self.texts.push(instance);
    }

    pub fn remove_cache(&mut self, text: &str, paint: &TextPaint) {
        let key = CacheKey {
            text: text.to_string(),
            font_index: paint.font_index,
            font_size: paint.font_size,
            font_weight: paint.font_weight,
            line_width: paint.line_width,
            color: paint.color,
            line_color: paint.line_color,
        };
        if let Some(glyph) = self.cache.remove(&key) {
            self.atlas.clear_canvas(&mut self.canvas, &glyph);
        }
    }

    /// canvasにため込んだコマンドをGPUに送る
    pub fn submit(&mut self, clear: bool) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Clear Offscreen Encoder"),
            });

        let load = if clear {
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
        } else {
            wgpu::LoadOp::Load
        };
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Clear Offscreen Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.textures[0].view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        let cmd = self.canvas.flush_to_output(&self.textures[0].texture);
        self.queue
            .submit(std::iter::once(encoder.finish()).chain(cmd));

        // todo: offsetの位置も指定できるようにする
        // キャッシュ削除の場合は不要、というかこれ別で分けたほうがいい
        // instance更新
        self.queue.write_buffer(
            &self.text_instance_buffer,
            0,
            bytemuck::cast_slice(&self.texts),
        );
    }

    fn create_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("text_renderer_uniform_bind_group_layout"),
        })
    }

    fn create_uniform_buffer(device: &wgpu::Device, logical_size: &[f32; 2]) -> wgpu::Buffer {
        let uniform = TextUniforms {
            logical_size: *logical_size,
        };
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Filter Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    fn create_uniform_bind_group(
        device: &wgpu::Device,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
        uniform_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        })
    }

    fn create_texture_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let mut texture_entries = Vec::with_capacity(1);
        for i in 0..1 {
            texture_entries.push(wgpu::BindGroupLayoutEntry {
                binding: i as u32,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                },
                count: None,
            });
        }

        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &texture_entries,
            label: Some("text_renderer_texture_bind_group_layout"),
        })
    }

    fn create_texture_bind_group(
        device: &wgpu::Device,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        textures: &Vec<Texture>,
    ) -> wgpu::BindGroup {
        let mut group_entries = Vec::with_capacity(1);
        for (i, texture) in textures.iter().enumerate() {
            group_entries.push(wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            });
        }
        // 共通で持っている空テクスチャを入れる
        // while group_entries.len() < 1 {
        //     group_entries.push(wgpu::BindGroupEntry {
        //         binding: group_entries.len() as u32,
        //         resource: wgpu::BindingResource::TextureView(&empty_texture.view),
        //     });
        // }

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &group_entries,
            label: Some("texture_bind_group"),
        })
    }

    fn create_sampler_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering), // Filteringにしてけばsamplerのほうを使ってくれるらしいが未確認
                count: None,
            }],
            label: Some("sampler_bind_group_layout"),
        })
    }

    fn create_sampler_bind_group(
        device: &wgpu::Device,
        sampler_bind_group_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: sampler_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(sampler),
            }],
            label: Some("sampler_bind_group"),
        })
    }

    /// オフスクリーンに描画したテキストを転送するだけものもの
    /// todo シェーダーバッファは外から渡すようにする
    fn create_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        sampler_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader_buffer = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader_text.wgsl").into()),
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("text_renderer_uniform_bind_group_layout"),
            });

        let rende_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Text Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(texture_bind_group_layout),
                    Some(sampler_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Render Pipeline"),
            layout: Some(&rende_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_buffer,
                entry_point: Some("vs_main"),
                buffers: &[Some(TextInstance::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_buffer,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false), // 書き込みは行わない
                depth_compare: Some(wgpu::CompareFunction::Greater), // 手前（大きい値）にあるピクセルを描画
                stencil: wgpu::StencilState::default(),              // ステンシルは使用しない
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        render_pipeline
    }

    /// キャッシュ用バックバッファ
    fn create_backbuffer(
        device: &wgpu::Device,
        texture_dimension: u32,
        format: wgpu::TextureFormat,
    ) -> Texture {
        let texture =
            Texture::create_texture(device, texture_dimension, texture_dimension, format, true);
        texture
    }
}
