/// 転送にはz値つきのshader_texture_frame.wgslを使う
/// いちおう１フォントだけ転送したがstroke_textの品質が悪いので実用には使えない
/// 確認用にオフスクリーンテクスチャの保存を実装
use std::{iter, sync::Arc};

use femtovg::{Canvas, Color, Paint, renderer};
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::sub::{frame::FrameInstance, key::InputState, texture::Texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct VertexUniforms {
    screen_size: [f32; 2],
}

pub struct State {
    target_logical_size: (f32, f32),
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    textures: Vec<Texture>,
    texture_bind_group: wgpu::BindGroup,
    sampler_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    sprites: Vec<FrameInstance>,
    canvas: Canvas<renderer::WGPURenderer>,
    font_ids: Vec<femtovg::FontId>,
    input: InputState,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        // 初期サイズを論理サイズとして設定する
        let target_logical_size = (size.width as f32, size.height as f32);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::default(),
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: Default::default(),
        });
        // すべてデフォルトだがbackendsもデフォルトになってしまう
        //let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window.clone()).unwrap();

        // Adapterを探す
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(), // 電力志向
                compatible_surface: Some(&surface), // Surfaceと互換性のあるAdapterを探す
                force_fallback_adapter: false,      // trueなら強制ソフトウェア
                apply_limit_buckets: true,          // ハードウェア固有の制限値を丸める
            })
            .await
            .unwrap();

        println!("=== WGPU Debug Info ===");

        // 探したAdapterからDeviceとQueueを作る
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sample01"),                    // デバイス特定のためつけておく
                required_features: wgpu::Features::empty(), // 拡張機能不要なのでempty
                experimental_features: wgpu::ExperimentalFeatures::disabled(), // 実験的機能は不要
                required_limits: wgpu::Limits::default(),   // 限界値は標準の制限値
                memory_hints: Default::default(),           // メモリ割り当て方法は標準
                trace: wgpu::Trace::Off,                    // 通常はOff
            })
            .await
            .unwrap();

        // 利用可能モードを取得
        let surface_caps = surface.get_capabilities(&adapter);
        // ピクセルフォーマットの決定
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        // サーフェス情報
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, // 描画先（レンダリングターゲット）として使用
            format: surface_format,                        // surface_capsから決定したフォーマット
            width: size.width,                             // ウィンドウ内部の幅
            height: size.height,                           // ウィンドウ内部の高さ
            present_mode: wgpu::PresentMode::default(),    // 垂直同期
            alpha_mode: surface_caps.alpha_modes[0], // ウィンドウ背後との合成モード（利用可能な最初のモード）
            view_formats: vec![],                    // ビューフォーマットの追加設定（空）
            desired_maximum_frame_latency: 2,        // 最大フレームレイテンシ
            color_space: wgpu::SurfaceColorSpace::Auto, // カラー空間の自動設定
        };
        println!("=======================");

        let empty_texture = Texture::create_empty_texture(&device, &queue);
        let mut textures = Vec::new();
        // とりあえず4Kをカバーできるテクスチャを作成
        let texture = Texture::create_texture(&device, 4096, 4096, true);
        textures.push(texture);

        let texture_count = textures.len() as u32;
        let mut layout_entries = Vec::with_capacity(texture_count as usize);
        for i in 0..texture_count {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
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

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &layout_entries,
                label: Some("texture_bind_group_layout"),
            });

        let mut group_entries = Vec::with_capacity(texture_count as usize);
        for (i, texture) in textures.iter().enumerate() {
            group_entries.push(wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            });
        }
        while group_entries.len() < texture_count as usize {
            group_entries.push(wgpu::BindGroupEntry {
                binding: group_entries.len() as u32,
                resource: wgpu::BindingResource::TextureView(&empty_texture.view),
            });
        }

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &group_entries,
            label: Some("texture_bind_group"),
        });

        let sampler = Texture::create_sampler(&device);
        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                }],
                label: Some("sampler_bind_group_layout"),
            });

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sampler_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            }],
            label: Some("sampler_bind_group"),
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
                label: Some("uniform_bind_group_layout"),
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&texture_bind_group_layout),
                    Some(&sampler_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let uniform_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Filter Uniform Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_texture_frame.wgsl").into()),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Filter Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &uniform_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(FrameInstance::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &uniform_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let uniform: VertexUniforms = VertexUniforms {
            screen_size: [target_logical_size.0, target_logical_size.1],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Filter Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        // とりあえず6つ用意
        let sprites = vec![FrameInstance::default(); 6];

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Filter Instance Buffer"),
            contents: bytemuck::cast_slice(&sprites),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        // ここからフォント
        let renderer = renderer::WGPURenderer::new(device.clone(), queue.clone());
        let mut canvas = Canvas::new(renderer).expect("Cannot create canvas");
        canvas.set_size(size.width, size.height, 1.0);

        let font_data_yosu = canvas
            .add_font_mem(include_bytes!("../../fonts/yosugaraver1_2.ttf"))
            .expect("Cannot add font");

        let font_data_gen = canvas
            .add_font_mem(include_bytes!("../../fonts/GenEiPOPlePw-Bk.ttf"))
            .unwrap();

        let mut font_ids = Vec::new();
        font_ids.push(font_data_yosu);
        font_ids.push(font_data_gen);

        Ok(Self {
            target_logical_size,
            surface,
            device,
            queue,
            config,
            render_pipeline,
            textures,
            texture_bind_group,
            sampler_bind_group,
            uniform_buffer,
            uniform_bind_group,
            instance_buffer,
            sprites,
            canvas,
            font_ids,
            input: InputState::default(),
            window,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        // サーフェスは固定で画面サイズに合わせて伸縮したい場合はサイズ変更をやめるだけでOK
        // 縦横比率はviewportで制御する
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);

            self.resize_text();
            self.draw_text();
        }
    }

    pub fn draw_text(&mut self) {
        // オフスクリーンテクスチャを完全透明（RGBA 0,0,0,0）にクリアする
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Clear Offscreen Encoder"),
            });
        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Offscreen Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.textures[0].view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), // 完全透明でクリア
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
        }
        let clear_cmd = encoder.finish();

        // canvasへの設定は描画するたびに必要
        // font1
        {
            let line_width = 4.0;
            let stroke_paint = Paint::color(Color::rgba(0, 0, 0, 128))
                .with_font(&[self.font_ids[0]])
                .with_font_weight(Paint::FONT_WEIGHT_BOLD)
                .with_line_width(line_width)
                .with_letter_spacing(2.0) // ← 字間を広げることができる
                .with_font_size(36.0)
                .with_line_join(femtovg::LineJoin::Round) // トゲ飛び出し防止
                // Y座標の基準を「文字の上端」にする
                .with_text_baseline(femtovg::Baseline::Top)
                // X座標の基準を「文字の左端」にする（デフォルトもLeft）
                .with_text_align(femtovg::Align::Left);

            let fill_paint = stroke_paint.clone().with_color(Color::white());

            let text = "こんにちは, femtovg!\nand wgpu! こんにちは\nThis is a sample program.";

            // 画面（960x540論理空間）上の表示開始位置
            let disp_x = 0.0;
            let disp_y = 0.0;
            let line_height = 40.0;
            // テクスチャ（4096空間）内の配置開始位置
            let tex_x = 5.0;
            let tex_y = 5.0;
            let scale_x = self.config.width as f32 / self.target_logical_size.0;
            let scale_y = self.config.height as f32 / self.target_logical_size.1;
            // \nまでしか描画されないので改行ごとにループして描画
            // 同時にspriteの更新も行う
            // 戻り値から取得することも可能だが実用では空きを確認するので事前に取得しておく
            for (i, line) in text.lines().enumerate() {
                let cur_tex_y = tex_y + i as f32 * line_height;
                let cur_disp_y = disp_y + i as f32 * line_height;
                if let Ok(metrics) = self.canvas.measure_text(0.0, 0.0, line, &stroke_paint) {
                    // フチ取り分のマージン（片側2px）を考慮して切り出す
                    let pad = line_height / 2.0;
                    // 論理サイズ（960x540 基準）
                    let log_w = metrics.width() + pad * 2.0;
                    let log_h = metrics.height() + pad * 2.0;
                    // 1. 画面上の配置（シェーダーが自動拡大するので論理サイズのままでOK）
                    self.sprites[i].position = [disp_x - pad, cur_disp_y - pad];
                    self.sprites[i].size = [log_w, log_h];
                    // 2. テクスチャ上の実際のピクセルサイズ（scale が掛かっている！）
                    let tex_actual_x = (tex_x - pad) * scale_x;
                    let tex_actual_y = (cur_tex_y - pad) * scale_y;
                    let tex_actual_w = log_w * scale_x;
                    let tex_actual_h = log_h * scale_y;
                    // 3. テクスチャの切り出しUV（実際のピクセル範囲を 4096 で割る）
                    self.sprites[i].uv_offset = [tex_actual_x / 4096.0, tex_actual_y / 4096.0];
                    self.sprites[i].uv_size = [tex_actual_w / 4096.0, tex_actual_h / 4096.0];
                }

                // テクスチャ内の指定位置に描画
                let _ = self
                    .canvas
                    .stroke_text(tex_x, cur_tex_y, line, &stroke_paint);
                let _ = self.canvas.fill_text(tex_x, cur_tex_y, line, &fill_paint);
            }
            self.queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.sprites),
            );
        }

        // font2
        {
            let stroke_paint = Paint::color(Color::black())
                .with_font(&[self.font_ids[1]])
                .with_font_weight(Paint::FONT_WEIGHT_BOLD)
                .with_line_width(4.0)
                .with_letter_spacing(2.0) // ← 字間を広げることができる
                .with_font_size(36.0)
                .with_line_join(femtovg::LineJoin::Round) // トゲ飛び出し防止
                // Y座標の基準を「文字の上端」にする
                .with_text_baseline(femtovg::Baseline::Top)
                // X座標の基準を「文字の左端」にする（デフォルトもLeft）
                .with_text_align(femtovg::Align::Left);

            let fill_paint = stroke_paint.clone().with_color(Color::white());

            let text = "こんにちは, femtovg!\nand wgpu!\nThis is a sample program.";

            let x = 200.0;
            let start_y = 200.0;
            let line_height = 40.0; // 1行の高さ
            // \nまでしか描画されないので改行ごとにループして描画
            for (i, line) in text.lines().enumerate() {
                let y = start_y + i as f32 * line_height;
                let _ = self.canvas.stroke_text(x, y, line, &stroke_paint);
                let _ = self.canvas.fill_text(x, y, line, &fill_paint);
            }
        }
        // オフスクリーン描画
        let cmd = self.canvas.flush_to_output(&self.textures[0].texture);
        self.queue.submit(iter::once(clear_cmd).chain(cmd)); // GPU に投げて完了！
    }

    pub fn save_offscreen_png(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let width: u32 = 4096;
        let height: u32 = 4096;
        let row_bytes = width * 4; // RGBA8
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = row_bytes.div_ceil(alignment) * alignment;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("offscreen readback buffer"),
            size: u64::from(padded_bytes_per_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("offscreen readback encoder"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.textures[0].texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).expect("map callback receiver dropped");
            });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;

        let mapped = buffer.slice(..).get_mapped_range()?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in mapped
            .chunks_exact(padded_bytes_per_row as usize)
            .take(height as usize)
        {
            pixels.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();

        image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)?;

        Ok(())
    }

    fn resize_text(&mut self) {
        // Canvasの解像度・スケールを更新
        self.canvas
            .set_size(self.config.width, self.config.height, 1.0);

        // 初期論理サイズに対するスケール倍率を計算
        let scale_x = self.config.width as f32 / self.target_logical_size.0;
        let scale_y = self.config.height as f32 / self.target_logical_size.1;

        self.canvas.reset_transform(); // 累積を防ぐためスケールをリセット
        self.canvas.scale(scale_x, scale_y); // 全体をスケーリング
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        match (key, pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            (KeyCode::F2, true) => {
                self.save_offscreen_png(&std::path::Path::new("femtovg_offscreen.png"))
                    .unwrap();
            }
            (KeyCode::F4, true) => {
                let _ = self
                    .window
                    .request_inner_size(winit::dpi::PhysicalSize::new(960, 540));
            }
            _ => {
                self.input.process_key(key, pressed);
            }
        }
    }

    fn update(&mut self) {}

    fn render(&mut self) -> anyhow::Result<()> {
        // 次フレームのRedrawRequested要求
        // この処理が一定間隔のループを実現している
        self.window.request_redraw();

        // 画面に出力するsurfaceを取得
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                // SurfaceConfigの再構築
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };

        // アクセスするためのビューを作成
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // 描画コマンドを記録するためのコマンドエンコーダを作成
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            // レンダーパス開始
            // 背景を塗りつぶしているだけ
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // パスの開始時と終了時の操作
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None, // テクスチャにレンダリングを行うための設定
                })],
                depth_stencil_attachment: None, // 深度バッファやステンシルバッファの設定
                occlusion_query_set: None,      // 見えないオブジェクトをスキップするためのクエリ
                timestamp_writes: None,         // 処理計測のタイムスタンプの書き込み先
                multiview_mask: None,           // マルチビュー機能のためのマスク
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
            render_pass.set_bind_group(2, &self.sampler_bind_group, &[]);

            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.sprites.len() as u32);
        }

        //self.draw_text();

        let clear_cmd = encoder.finish();
        //let femtovg_cmd = self.canvas.flush_to_output(&output.texture);

        // コマンドを実行
        self.queue.submit(iter::once(clear_cmd));
        // サーフェスに反映
        self.queue.present(output);

        Ok(())
    }
}

pub struct App {
    state: Option<State>,
}

impl App {
    pub fn new() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_inner_size(winit::dpi::PhysicalSize::new(960, 540))
            .with_visible(false);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        window.set_visible(true);

        let state = pollster::block_on(State::new(window)).unwrap();
        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match (button, state.is_pressed()) {
                (MouseButton::Left, true) => {}
                (MouseButton::Left, false) => {}
                _ => {}
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => state.handle_key(event_loop, code, key_state.is_pressed()),
            _ => {}
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}
