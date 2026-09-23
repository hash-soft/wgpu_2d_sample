/// この方法をベースに拡張していく
/// todo
/// ・表示順にソート
/// ・色合成
/// ・フィルター
/// memo
/// ・ブレンド方法を変えるにはレンダーパイプライン変えないといけないからdrawを分割する必要がある
///
use std::{iter, sync::Arc};

use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{key::InputState, sprite::SpriteCharacterInstance, texture::Texture};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
}

pub struct State {
    #[allow(dead_code)]
    target_logical_size: (f32, f32),
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    // textureとbind_groupをvecにする
    #[allow(dead_code)]
    empty_texture: Texture,
    #[allow(dead_code)]
    textures: Vec<Texture>,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    texture_bind_group: wgpu::BindGroup,
    sampler_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    uniform_buffer: wgpu::Buffer, // 保持しているだけ
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    sprites: Vec<SpriteCharacterInstance>,
    direction: u32,
    pattern_count: u32,
    input: InputState,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        // 初期サイズを変換する
        let target_logical_size = (size.width as f32, size.height as f32);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            // WindowsなのでDX12 Vulkanだとinstance作成に成功しても
            // 古いintel内蔵gpuの環境だと次のrequest_device()でアクセス違反になることがある
            // （たまに起動する）ので素直にDX12
            // 消費メモリがめちゃくちゃ増えるがWGPUがバージョンアップしたらまたVulkan試してみる
            backends: wgpu::Backends::DX12,
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

        // 必要なものだけログに出す
        println!("=== WGPU Debug Info ===");
        let limits = adapter.limits();
        println!(
            "Max sampled textures per shader stage: {}",
            limits.max_sampled_textures_per_shader_stage
        );
        println!("Max bind groups: {}", limits.max_bind_groups);
        println!(
            "Max bindings per bind group: {}",
            limits.max_bindings_per_bind_group
        );

        let max_sampled_textures = Texture::request_max_textures(&limits, 32);

        // 探したAdapterからDeviceとQueueを作る
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sample02"),                    // デバイス特定のためつけておく
                required_features: wgpu::Features::empty(), // 拡張機能不要なのでempty
                experimental_features: wgpu::ExperimentalFeatures::disabled(), // 実験的機能は不要
                required_limits: wgpu::Limits {
                    // テクスチャエントリー数だけ指定
                    max_sampled_textures_per_shader_stage: max_sampled_textures,
                    ..wgpu::Limits::default()
                },
                memory_hints: Default::default(), // メモリ割り当て方法は標準
                trace: wgpu::Trace::Off,          // 通常はOff
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
        let dragon_bytes: &[u8] = include_bytes!("image/characters/pipo-enemy021.png");
        let oni_bytes: &[u8] = include_bytes!("image/characters/pipo-enemy019.png");
        let purin_bytes: &[u8] = include_bytes!("image/characters/cm_001.png");
        let c_set_bytes = include_bytes!("image/characters/c_set_001.png");
        let array_bytes = vec![dragon_bytes, oni_bytes, purin_bytes, c_set_bytes];

        let mut textures = Vec::with_capacity(array_bytes.len());
        for bytes in array_bytes.iter() {
            let texture = Texture::from_bytes(&device, &queue, bytes, "Texture 2D")?;
            textures.push(texture);
        }

        // 静的シェーダーを使っている関係で使う数だけにする
        let texture_count = array_bytes.len() as u32;
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

        // テクスチャグループレイアウト
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

        // テクスチャバインドグループ
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &group_entries,
            label: Some("texture_bind_group"),
        });

        let sampler = Texture::create_sampler(&device);
        let mut sampler_layout_entries = Vec::with_capacity(1);
        sampler_layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        });

        let sampler_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &sampler_layout_entries,
                label: Some("sampler_bind_group_layout"),
            });

        let mut sampler_group_entries = Vec::with_capacity(1);
        sampler_group_entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Sampler(&sampler),
        });

        let sampler_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sampler_bind_group_layout,
            entries: &sampler_group_entries,
            label: Some("sampler_bind_group"),
        });

        // ユニフォームグループレイアウト
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX, // 頂点シェーダーで参照するため
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        // テクスチャパイプラインレイアウトの作成
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&texture_bind_group_layout),
                    Some(&sampler_bind_group_layout),
                ], // グループレイアウトをバインド
                immediate_size: 0,
            });

        // shader_multi_entry.wgsl
        let uniform_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Uniform Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_multi_entry.wgsl").into()),
        });

        // Uniformを使用したパイプライン
        let uniform_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &uniform_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(SpriteCharacterInstance::desc())],
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

        // ユニフォームデータ
        let uniform: Uniforms = Uniforms {
            screen_size: [target_logical_size.0, target_logical_size.1],
        };

        // uniformバッファ
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // uniformバインドグループ
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let sprites = vec![
            // スプライト 0（キャラセット1枚目 / 矢印キーで移動・左上原点回転）
            SpriteCharacterInstance {
                position: [400.0, 200.0],
                size: [32.0, 32.0],
                uv_offset: [0.0, 0.0],
                uv_size: [
                    32.0 / textures[3].texture.width() as f32,
                    32.0 / textures[3].texture.height() as f32,
                ],
                texture_index: 3,
                opacity: 1.0,
                scale: [1.0, 1.0],
                rotation: 0.0,
                pivot: [0.0, 0.0], // 左上原点で回転
            },
            // スプライト 1（ドラゴン / 中心を軸に自動回転）
            SpriteCharacterInstance {
                position: [400.0, 300.0],
                size: [
                    textures[0].texture.width() as f32,
                    textures[0].texture.height() as f32,
                ],
                uv_offset: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                texture_index: 0,
                opacity: 1.0,
                scale: [1.0, 1.0],
                rotation: 0.0,
                pivot: [0.5, 0.5], // 中心を軸に回転
            },
            // スプライト 2（鬼 / 0.5倍縮小・半透明）
            SpriteCharacterInstance {
                position: [100.0, 100.0],
                size: [
                    textures[1].texture.width() as f32,
                    textures[1].texture.height() as f32,
                ],
                uv_offset: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                texture_index: 1,
                opacity: 0.5,
                scale: [0.5, 0.5], // 0.5倍縮小
                rotation: 0.0,
                pivot: [0.5, 0.5],
            },
            // スプライト 3（プリン / 45度回転・中心軸）
            SpriteCharacterInstance {
                position: [100.0, 400.0],
                size: [
                    textures[2].texture.width() as f32,
                    textures[2].texture.height() as f32,
                ],
                uv_offset: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                texture_index: 2,
                opacity: 1.0,
                scale: [1.0, 1.0],
                rotation: std::f32::consts::FRAC_PI_4, // 45度
                pivot: [0.5, 0.5],
            },
            // スプライト 4（プリン / 2倍拡大・右端を軸に回転）
            SpriteCharacterInstance {
                position: [200.0, 400.0],
                size: [
                    textures[2].texture.width() as f32,
                    textures[2].texture.height() as f32,
                ],
                uv_offset: [0.0, 0.0],
                uv_size: [1.0, 1.0],
                texture_index: 2,
                opacity: 1.0,
                scale: [2.0, 2.0], // 2倍拡大
                rotation: 0.0,
                pivot: [1.0, 0.5], // 右端中央を軸
            },
        ];

        // instanceバッファ
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&sprites),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Ok(Self {
            target_logical_size,
            surface,
            device,
            queue,
            config,
            empty_texture,
            textures,
            sampler,
            texture_bind_group,
            sampler_bind_group,
            render_pipeline: uniform_render_pipeline,
            uniform_buffer,
            uniform_bind_group,
            instance_buffer,
            sprites,
            direction: 0,
            pattern_count: 0,
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

            // 以下のコメントアウト部分を有効にすると位置とサイズが追従しない
            // let uniform = Uniforms {
            //     screen_size: [width as f32, height as f32], // ここを新しい幅・高さに更新
            //     position: [400.0, 300.0],                   // 任意の固定位置 (x, y)
            //     image_size: [
            //         self.diffuse_texture.texture.width() as f32,
            //         self.diffuse_texture.texture.height() as f32,
            //     ],
            // };

            // // 2. queue.write_buffer で GPU 上のバッファデータを書き換える
            // self.queue
            //     .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniform]));
        }
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        match (key, pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {
                self.input.process_key(key, pressed);
            }
        }
    }

    fn update(&mut self) {
        let speed = 2.0;
        let [dx, dy] = self.input.direction();

        self.pattern_count = (self.pattern_count + 1) % 60;
        if dx != 0.0 || dy != 0.0 {
            // スプライト 0: 矢印キーで移動
            self.sprites[0].position[0] += dx * speed;
            self.sprites[0].position[1] += dy * speed;
            // スプライトの方向やアニメーションなどの座標はcpu側で設定する
            self.direction = if dx > 0.0 {
                2
            } else if dx < 0.0 {
                1
            } else if dy > 0.0 {
                0
            } else if dy < 0.0 {
                3
            } else {
                self.direction
            };
            self.sprites[0].uv_offset[1] =
                32.0 * self.direction as f32 / self.textures[3].texture.height() as f32;
        }
        let pattern = (self.pattern_count / 15) as usize;
        let pattern_table: [u32; 4] = [1, 2, 1, 0];
        self.sprites[0].uv_offset[0] =
            (32.0 * pattern_table[pattern] as f32) / self.textures[3].texture.width() as f32;

        // スプライト 1（ドラゴン）: 中心軸で自動回転
        self.sprites[1].rotation += 0.02;
        if self.sprites[1].rotation >= std::f32::consts::TAU {
            self.sprites[1].rotation -= std::f32::consts::TAU;
        }

        // スプライト 4（プリン）: 右端軸で自動回転
        self.sprites[4].rotation += 0.01;
        if self.sprites[4].rotation >= std::f32::consts::TAU {
            self.sprites[4].rotation -= std::f32::consts::TAU;
        }

        // 毎フレームGPUバッファを更新
        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.sprites),
        );
    }

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

        // 描画コマンドを記録するためのコマンドエンコーダを作成
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            // レンダーパス開始
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

            // パイプライン設定
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
            render_pass.set_bind_group(2, &self.sampler_bind_group, &[]);
            // ここでキャラ全部のintanceを流し込む
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.sprites.len() as u32);
        }

        // コマンドを実行
        self.queue.submit(iter::once(encoder.finish()));
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

        self.state = Some(pollster::block_on(State::new(window)).unwrap());
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
