/// ストローク付きで出力
use std::{iter, sync::Arc};

use femtovg::{Canvas, Color, Paint, renderer};
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
    canvas: Canvas<renderer::WGPURenderer>, // 追加
    font_ids: Vec<femtovg::FontId>,         // 追加
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
            canvas,
            font_ids,
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
        }
    }

    pub fn draw_text(&mut self) {
        // canvasへの設定は描画するたびに必要
        // font1
        {
            let stroke_paint = Paint::color(Color::black())
                .with_font(&[self.font_ids[0]])
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

            let x = 5.0;
            let start_y = 5.0;
            let line_height = 40.0; // 1行の高さ
            // \nまでしか描画されないので改行ごとにループして描画
            for (i, line) in text.lines().enumerate() {
                let y = start_y + i as f32 * line_height;
                let _ = self.canvas.stroke_text(x, y, line, &stroke_paint);
                let _ = self.canvas.fill_text(x, y, line, &fill_paint);
            }
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
    }

    fn resize_text(&mut self) {
        // Canvasの解像度・スケールを更新
        self.canvas
            .set_size(self.config.width, self.config.height, 1.0);

        // 初期論理サイズに対するスケール倍率を計算
        let scale_x = self.config.width as f32 / self.target_logical_size.0;
        let scale_y = self.config.height as f32 / self.target_logical_size.1;
        //self.canvas.save();
        self.canvas.reset_transform(); // 累積を防ぐためスケールをリセット
        self.canvas.scale(scale_x, scale_y); // 全体をスケーリング
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

        // 描画コマンドを記録するためのコマンドエンコーダを作成
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            // レンダーパス開始
            // 背景を塗りつぶしているだけ
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        }

        self.draw_text();

        let clear_cmd = encoder.finish();
        let femtovg_cmd = self.canvas.flush_to_output(&output.texture);

        // コマンドを実行
        self.queue.submit(iter::once(clear_cmd).chain(femtovg_cmd));
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
