use std::{iter, sync::Arc};

use glyphon::{
    Attrs, Cache, Family, FontSystem, Resolution, SwashCache, TextArea, TextAtlas, TextBounds,
    TextRenderer, Viewport,
};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

pub struct State {
    target_logical_size: (f32, f32),
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    font_system: FontSystem,
    text_renderer: TextRenderer,
    text_buffer: glyphon::Buffer,
    text_viewport: Viewport,
    text_atlas: TextAtlas,
    swash_cache: SwashCache,
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

        // Adapterの情報を出力
        let adapter_info = adapter.get_info();
        println!("=== WGPU Debug Info ===");
        println!("Adapter Info:\n {:#?}", adapter_info);
        let limits = adapter.limits();
        println!("Max 2D texture size: {}", limits.max_texture_dimension_2d);

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
        println!("Surface Capabilities:\n {:#?}", surface_caps);
        // ピクセルフォーマットの決定
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        println!("Surface Format:\n {:#?}", surface_format);
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
        // SurfaceConfigの出力
        println!("Surface Config:\n {:#?}", config);
        println!("=======================");

        let mut font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let mut cache = Cache::new(&device);
        let mut text_atlas = TextAtlas::new(&device, &queue, &mut cache, config.format);
        let text_renderer = TextRenderer::new(
            &mut text_atlas,
            &device,
            wgpu::MultisampleState::default(),
            None,
        );

        let mut text_buffer =
            glyphon::Buffer::new(&mut font_system, glyphon::Metrics::new(30.0, 42.0));
        text_buffer.set_size(Some(size.width as f32), Some(size.height as f32));
        text_buffer.set_text(
            "Hello, glyphon!\nand wgpu!\nThis is a sample program.",
            // Attrsを返せばいいので不要ならcolorを設定する必要はない
            // rgbaも使える
            &Attrs::new()
                .family(Family::SansSerif)
                .color(glyphon::Color::rgb(255, 255, 255)),
            glyphon::Shaping::Advanced,
            None,
        );
        // text_bufferを更新したら行う必要がある
        text_buffer.shape_until_scroll(&mut font_system, false);
        let mut text_viewport = Viewport::new(&device, &cache);
        text_viewport.update(
            &queue,
            Resolution {
                width: config.width,
                height: config.height,
            },
        );

        Ok(Self {
            target_logical_size,
            surface,
            device,
            queue,
            config,
            font_system,
            text_renderer,
            text_buffer,
            text_viewport,
            text_atlas,
            swash_cache,
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
            // サイズ変更に伴うテキストの再描画
            // そのまま拡大させたい（ギザギザ）場合は呼ばなければいい
            // （viewportを論理サイズのままにしておく）
            self.resize_text();
        }
    }

    pub fn text_renderer_prepare(&mut self) {
        // 実際は縦横の小さいほうに合わせてサイズをそろえるため
        // 縦横でスケールが異なることはない
        let scale_x = self.config.width as f32 / self.target_logical_size.0;
        let scale_y = self.config.height as f32 / self.target_logical_size.1;
        let uniform_scale = scale_x.min(scale_y);
        // 描画テキストの準備 (Prepare)
        self.text_renderer
            .prepare(
                &self.device,
                &self.queue,
                &mut self.font_system,
                &mut self.text_atlas,
                &self.text_viewport,
                [TextArea {
                    buffer: &self.text_buffer,
                    left: 250.0 * scale_x,
                    top: 150.0 * scale_y,
                    scale: uniform_scale,
                    // 現在の設定は画面全体なので問題ないが
                    // １部に設定したければscaleをかける必要がある
                    bounds: TextBounds {
                        left: 0,
                        top: 0,
                        right: self.config.width as i32,
                        bottom: self.config.height as i32,
                    },
                    default_color: glyphon::Color::rgb(0, 255, 255),
                    custom_glyphs: &[],
                }],
                &mut self.swash_cache,
            )
            .unwrap();
    }

    fn resize_text(&mut self) {
        self.text_buffer.set_size(
            Some(self.config.width as f32),
            Some(self.config.height as f32),
        );
        self.text_buffer
            .shape_until_scroll(&mut self.font_system, false);

        self.text_viewport.update(
            &self.queue,
            Resolution {
                width: self.config.width,
                height: self.config.height,
            },
        );
        self.text_renderer_prepare();
    }

    fn handle_key(&mut self, event_loop: &ActiveEventLoop, key: KeyCode, pressed: bool) {
        match (key, pressed) {
            (KeyCode::Escape, true) => event_loop.exit(),
            _ => {}
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

            self.text_renderer
                .render(&self.text_atlas, &self.text_viewport, &mut render_pass)
                .unwrap();
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

        let mut state = pollster::block_on(State::new(window)).unwrap();
        state.text_renderer_prepare();
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
