/// スプライトから不要なデータをそぎ落とし、最低限の情報をシェーダーに渡す
///
/// todo
/// ・回転の中心をずらせるようにする（優先低）
/// ・拡大、縮小時に中心が追従するようにする（優先低）
/// ・テクスチャとサンプラーの分離（優先中）
/// ・後方からの見下ろし（簡単なら）（優先最低）
/// ・実際にマップデータを読み込んでそれを表示する(作ったゲームのjson流用)（優先高）
use std::{iter, ops::Range, sync::Arc};
use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{key::InputState, texture::Texture, tile::TileInstance};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AtlasInfo {
    atlas_size: [u32; 2],
    tile_uv_size: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalUniforms {
    view_proj: [[f32; 4]; 4],
    tile_pixel_size: [f32; 2],
    pettern: [u32; 2],
    atlases: [AtlasInfo; 2],
}

pub struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    textures: Vec<Texture>,
    #[allow(dead_code)]
    sampler: wgpu::Sampler,
    texture_bind_group: wgpu::BindGroup,
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    global_unoform_buffer: wgpu::Buffer,
    camera_pos: [f32; 2],
    scale: f32,
    degress: f32,
    map_data: Vec<u32>,
    map_width: u32,
    map_height: u32,
    map_count: u32,
    tiles: Vec<TileInstance>,
    draw_ranges: Vec<Range<u32>>,
    input: InputState,
    window: Arc<Window>,
}

impl State {
    async fn new(window: Arc<Window>) -> anyhow::Result<State> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await
            .unwrap();

        let limits = adapter.limits();
        // サンプラー1枚+テクスチャ8枚
        // スプライトのほうが多いからスプライトのレイアウトを使いまわせばいいか
        let max_sampled_textures = Texture::request_max_sampled_textures(&limits, 9);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("tilemap_shader"),
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: wgpu::Limits {
                    max_sampled_textures_per_shader_stage: max_sampled_textures,
                    ..wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        println!(
            "device min_uniform_buffer_offset_aligment: {}",
            device.limits().min_uniform_buffer_offset_alignment
        );
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::default(),
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        // 複数テクスチャのロードと準備
        let empty_texture = Texture::create_empty_texture(&device, &queue);
        let world_bytes: &[u8] = include_bytes!("image/tilesets/world.png");
        let upper_bytes: &[u8] = include_bytes!("image/tilesets/upper.png");
        let array_bytes = vec![world_bytes, upper_bytes]; // テクスチャを増やす場合は要素を追加

        let mut textures = Vec::with_capacity(array_bytes.len());
        for bytes in array_bytes.iter() {
            let texture = Texture::from_bytes(&device, &queue, bytes, "Texture 2D")?;
            textures.push(texture);
        }
        let sampler = Texture::create_sampler(&device);

        let max_sampled_textures: u32 = 3;
        let texture_count = max_sampled_textures.saturating_sub(1);

        let mut layout_entries = Vec::with_capacity(max_sampled_textures as usize);
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
        layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: texture_count,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &layout_entries,
                label: Some("texture_bind_group_layout"),
            });

        let mut group_entries = Vec::with_capacity(max_sampled_textures as usize);
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
        group_entries.push(wgpu::BindGroupEntry {
            binding: texture_count,
            resource: wgpu::BindingResource::Sampler(&sampler),
        });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &group_entries,
            label: Some("texture_bind_group"),
        });

        let map_width = 100u32;
        let map_height = 100u32;
        let mut map_data = vec![0u32; (map_width * map_height * 2) as usize];
        // サンプルとして格子模様のマップを作成
        for y in 0..map_height {
            for x in 0..map_width {
                let idx = (y * map_width + x) as usize;
                // テクスチャインデックスを上位16ビット、タイルIDを下位16ビットにパックする
                let (tex_index, tile_id) = if (x + y) % 2 == 0 {
                    // 市松模様の片方: テクスチャ0 の タイル0
                    (0, 0)
                } else {
                    // 市松模様のもう片方: テクスチャ0 の タイル1
                    (0, 73)
                };
                let anim_index = if tile_id == 73 { 4 } else { 0 };

                // 31-28：テクスチャインデックス
                // 27-24：アニメーションインデックス
                // 23-21：アニメ枚数
                // 15-0：タイルID
                map_data[idx] =
                    (tex_index << 28) | (anim_index << 24) | (3 << 21) | (tile_id & 0xFFFF);
            }
        }
        // 重ねる部分を作成
        for y in 0..map_height {
            for x in 0..map_width {
                let idx = (y * map_width + x + map_width * map_height) as usize;
                if y % 5 == 0 {
                    map_data[idx] = (1 << 28) | 1 << 21 | (9 & 0xFFFF);
                } else {
                    map_data[idx] = (1 << 28) | 1 << 21 | 0xFFFF;
                }
            }
        }

        // ユニフォームデータ
        // シェーダーの共通データ
        let uniform: GlobalUniforms = GlobalUniforms {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            tile_pixel_size: [16.0, 16.0],
            pettern: [0, 0],
            atlases: [
                AtlasInfo {
                    atlas_size: [
                        textures[0].texture.width() / 16,
                        textures[0].texture.height() / 16,
                    ],
                    tile_uv_size: [
                        16.0 / textures[0].texture.width() as f32,
                        16.0 / textures[0].texture.height() as f32,
                    ],
                },
                AtlasInfo {
                    atlas_size: [
                        textures[1].texture.width() / 16,
                        textures[1].texture.height() / 16,
                    ],
                    tile_uv_size: [
                        16.0 / textures[1].texture.width() as f32,
                        16.0 / textures[1].texture.height() as f32,
                    ],
                },
            ],
        };

        // デバイスのアライメント制限を取得
        let alignment = device.limits().min_uniform_buffer_offset_alignment as usize;
        let uniform_size = std::mem::size_of::<GlobalUniforms>();
        // アライメントの倍数にパディングを計算
        // ２の累乗の最上位以外を落とす
        let aligned_size = (uniform_size + alignment - 1) & !(alignment - 1);

        // uniformバッファ
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Uniform Buffer"),
            size: (aligned_size * 2) as wgpu::BufferAddress,
            //contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        });
        {
            let mut buffer_view = uniform_buffer.get_mapped_range_mut(..)?;

            buffer_view
                .slice(0..uniform_size)
                .copy_from_slice(bytemuck::bytes_of(&uniform));
            buffer_view
                .slice(aligned_size..aligned_size + uniform_size)
                .copy_from_slice(bytemuck::bytes_of(&uniform));
        }
        uniform_buffer.unmap();

        // ユニフォームグループレイアウト
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX, // 頂点シェーダーで参照するため
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(wgpu::BufferSize::new(uniform_size as u64).unwrap()),
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer, // バッファ全体
                    offset: 0,
                    size: wgpu::BufferSize::new(uniform_size as u64), // 1回で使えるサイズ
                }),
            }],
            label: Some("uniform_bind_group"),
        });

        // WGSLシェーダーコードの動的構築
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tilemap Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader_tilemap.wgsl").into()),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    Some(&uniform_bind_group_layout),
                    Some(&texture_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let mut tiles = vec![];
        let mut draw_ranges = vec![];
        let counts: [u32; 2] = [
            textures[0].texture.width() / 32,
            textures[1].texture.width() / 32,
        ];
        for y in 0..20 {
            for x in 0..25 {
                // 32x32を16x16に分割する
                let tile = map_data[(y * map_width + x) as usize];
                TileInstance::push_tile(&mut tiles, tile, x as i32, y as i32, counts);
            }
        }
        let tile_count = tiles.len() as u32;
        draw_ranges.push(Range {
            start: 0,
            end: tile_count,
        });
        // 上に重ねる部分
        let one_layer_size = map_width * map_height;
        for y in 0..20 {
            for x in 0..25 {
                let tile = map_data[(y * map_width + x + one_layer_size) as usize];
                if tile & 0xFFFF == 0xFFFF {
                    continue;
                }
                TileInstance::push_tile(&mut tiles, tile, x as i32, y as i32, counts);
            }
        }
        draw_ranges.push(Range {
            start: tile_count,
            end: tiles.len() as u32,
        });

        // 全体のタイル頂点
        // これを層ごとに分配する
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (std::mem::size_of::<TileInstance>() * 122880) as wgpu::BufferAddress, // 実際は縮小に制限を持たせて超えないようにする
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true, // ← ここを true にすると作成と同時に書き込みを行うことが可能
        });
        {
            let initial_bytes = bytemuck::cast_slice(&tiles);

            let mut buffer_view = instance_buffer
                .slice(..initial_bytes.len() as wgpu::BufferAddress)
                .get_mapped_range_mut()?;

            // 取得した範囲全体にそのままコピーする
            buffer_view.copy_from_slice(initial_bytes);
        }
        instance_buffer.unmap();

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(TileInstance::desc())],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
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
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            textures,
            sampler,
            texture_bind_group,
            uniform_bind_group,
            instance_buffer,
            global_unoform_buffer: uniform_buffer,
            camera_pos: [400.0, 300.0],
            scale: 1.0,
            degress: 0.0,
            map_data,
            map_width,
            map_height,
            map_count: 0,
            tiles,
            draw_ranges,
            input: InputState::default(),
            window,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
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
        if self.update_input() {
            self.set_tile_data();
        }

        // 正射影行列を作成する、WGPUのNDCマッピング
        // 1draw中に変化することはないのでcpu側で行う
        let proj = glam::camera::rh::proj::directx::orthographic(0.0, 800.0, 600.0, 0.0, -1.0, 1.0);

        let rotation = self.degress.to_radians();
        // 右側から適用
        // ・画面中央への配置
        // ・スケーリング
        // ・カメラ位置の逆オフセット
        let view = glam::Mat4::from_translation(glam::Vec3::new(800.0 / 2.0, 600.0 / 2.0, 0.0))
            * glam::Mat4::from_rotation_z(rotation)
            * glam::Mat4::from_scale(glam::Vec3::new(self.scale, self.scale, 1.0))
            * glam::Mat4::from_translation(glam::Vec3::new(
                -self.camera_pos[0],
                -self.camera_pos[1],
                0.0,
            ));
        let view_proj = proj * view;

        self.queue.write_buffer(
            &self.global_unoform_buffer,
            0,
            bytemuck::bytes_of(&view_proj.to_cols_array_2d()),
        );

        // 回転を止める
        let view_upper =
            glam::Mat4::from_translation(glam::Vec3::new(800.0 / 2.0, 600.0 / 2.0, 0.0))
                * glam::Mat4::from_scale(glam::Vec3::new(self.scale, self.scale, 1.0))
                * glam::Mat4::from_translation(glam::Vec3::new(
                    -self.camera_pos[0],
                    -self.camera_pos[1],
                    0.0,
                ));
        let view_proj_upper = proj * view_upper;
        let alignment =
            self.device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
        self.queue.write_buffer(
            &self.global_unoform_buffer,
            alignment,
            bytemuck::bytes_of(&view_proj_upper.to_cols_array_2d()),
        );

        let pattern: [u32; 2] = [self.map_count / 20, 0];
        self.map_count = (self.map_count + 1) % 60;
        if pattern[0] != self.map_count {
            self.queue.write_buffer(
                &self.global_unoform_buffer,
                std::mem::size_of::<[[f32; 4]; 4]>() as wgpu::BufferAddress
                    + std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                bytemuck::bytes_of(&pattern),
            );
        }
    }

    fn update_input(&mut self) -> bool {
        if self.input.reset {
            self.camera_pos = [400.0, 300.0];
            self.scale = 1.0;
            self.degress = 0.0;
            return true;
        }

        let speed = 8.0;
        let [dx, dy] = self.input.direction();

        let mut dirty = false;

        if dx != 0.0 || dy != 0.0 {
            self.camera_pos[0] += dx * speed;
            self.camera_pos[1] += dy * speed;
            dirty = true;
        }
        // 値がずれないように２の負のべき乗単位で拡大縮小する
        if self.input.plus {
            self.scale += 0.125;
            dirty = true;
        } else if self.input.minus {
            if self.scale > 0.125 {
                self.scale -= 0.125;
                self.scale = self.scale.max(0.125);
                dirty = true;
            }
        }
        let prev_degress = self.degress;
        if self.input.rotation_l {
            self.degress = (self.degress + 360.0 - 4.0) % 360.0;
        } else if self.input.rotation_r {
            self.degress = (self.degress + 4.0) % 360.0;
        }
        if prev_degress != self.degress && (prev_degress == 0.0 || self.degress == 0.0) {
            dirty = true;
        }

        return dirty;
    }

    fn set_tile_data(&mut self) {
        let map_data = &self.map_data;
        let map_width = self.map_width as i32;
        let map_height = self.map_height as i32;
        let tiles = &mut self.tiles;
        let counts: [u32; 2] = [
            self.textures[0].texture.width() / 32,
            self.textures[1].texture.width() / 32,
        ];
        tiles.clear();

        // 可視範囲の計算
        let [half_w, half_h] = if self.degress != 0.0 {
            // 簡易的に最大領域を確保しているだけだが同じ位置なら更新がないのでメリットもある
            let half = ((400 * 400 + 300 * 300) as f32).sqrt() / self.scale;
            [half, half]
        } else {
            [800.0 / 2.0 / self.scale, 600.0 / 2.0 / self.scale]
        };

        let view_left = self.camera_pos[0] - half_w;
        let view_right = self.camera_pos[0] + half_w;
        let view_top = self.camera_pos[1] - half_h;
        let view_bottom = self.camera_pos[1] + half_h;

        // 32.0px単位のタイルインデックス範囲
        let start_x = (view_left / 32.0).floor() as i32;
        let end_x = (view_right / 32.0).ceil() as i32;
        let start_y = (view_top / 32.0).floor() as i32;
        let end_y = (view_bottom / 32.0).ceil() as i32;

        for y in start_y..=end_y {
            for x in start_x..=end_x {
                if x < 0 || x >= map_width || y < 0 || y >= map_height {
                    continue;
                }
                let tile = map_data[(y * map_width + x) as usize];
                TileInstance::push_tile(tiles, tile, x, y, counts);
            }
        }
        let tile_count = tiles.len() as u32;
        self.draw_ranges[0].start = 0;
        self.draw_ranges[0].end = tile_count;
        let one_layer_size = (self.map_width * self.map_height) as usize;
        for y in start_y..=end_y {
            for x in start_x..=end_x {
                if x < 0 || x >= map_width || y < 0 || y >= map_height {
                    continue;
                }
                let tile = map_data[(y * map_width + x) as usize + one_layer_size];
                if tile & 0xFFFF == 0xFFFF {
                    continue;
                }
                TileInstance::push_tile(tiles, tile, x, y, counts);
            }
        }
        self.draw_ranges[1].start = tile_count;
        self.draw_ranges[1].end = tiles.len() as u32;
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&tiles));
    }

    fn render(&mut self) -> anyhow::Result<()> {
        self.window.request_redraw();

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            let alignment = self.device.limits().min_uniform_buffer_offset_alignment as u32;

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[0]);
            render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            // 層ごとにdrawを分ける
            // タイルは1つのinstance_bufferにまとめる
            // tileバッファは固定だからそれぞれの層にstartは固定で割り当ててendを変化させたほうがいい気がするな
            // todo これはのちほど
            render_pass.draw(0..4, self.draw_ranges[0].clone());
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[alignment]);
            render_pass.draw(0..4, self.draw_ranges[1].clone());
        }

        self.queue.submit(iter::once(encoder.finish()));
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
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
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
