//! Pipeline opaco: posición, normal, UV y textura. La colisión no se dibuja.
//! Cada malla elige una matriz de modelo: la pista usa la identidad y cada auto
//! (chasis + cuatro ruedas) las que escribe la física en cada frame.

use glam::{Mat4, Vec3};
use revvy_formats::VisualMesh;

/// Autos que se pueden dibujar a la vez: los 32 de una sala (§5 de la arquitectura).
pub const MAX_CARS: usize = 32;
/// Slot 0 = identidad (pista). Después, cinco por auto: chasis y ruedas FL, FR, BL, BR.
/// Con 32 autos son 161 matrices de 256 bytes (41 KB); cada frame se escriben las usadas.
pub const MODEL_SLOTS: usize = 1 + 5 * MAX_CARS;
/// Alineación mínima de offsets dinámicos de uniform en wgpu.
const MODEL_STRIDE: u64 = 256;

pub struct Scene {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    white: wgpu::TextureView,
    track: Vec<DrawMesh>,
    car: Vec<DrawMesh>,
    model_buffer: wgpu::Buffer,
    model_bind: wgpu::BindGroup,
    depth: Option<wgpu::TextureView>,
    sky_pipeline: wgpu::RenderPipeline,
    sky_layout: wgpu::BindGroupLayout,
    sky_uniform: wgpu::Buffer,
    sky_sampler: wgpu::Sampler,
    sky_bind: wgpu::BindGroup,
}

struct DrawMesh {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
    index_count: u32,
    bind: wgpu::BindGroup,
    slot: u32,
}

pub struct CameraView {
    pub eye: Vec3,
    pub target: Vec3,
}

impl Scene {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("revvy-opaque"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("revvy-mesh"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let model_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("revvy-model"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(64),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("revvy-opaque"),
            bind_group_layouts: &[Some(&layout), Some(&model_layout)],
            immediate_size: 0,
        });
        let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("revvy-models"),
            size: MODEL_STRIDE * MODEL_SLOTS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let model_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("revvy-model"),
            layout: &model_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &model_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(64),
                }),
            }],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("revvy-opaque"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX_STRIDE,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 24,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 32,
                            shader_location: 3,
                        },
                    ],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("revvy-frame"),
            size: UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("revvy-tex"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let white = solid_texture(device, queue, [255, 255, 255, 255]);
        let (sky_pipeline, sky_layout, sky_uniform, sky_sampler, sky_bind) =
            create_sky(device, queue, format);
        let mut scene = Self {
            pipeline,
            layout,
            uniform,
            sampler,
            white,
            track: Vec::new(),
            car: Vec::new(),
            model_buffer,
            model_bind,
            depth: None,
            sky_pipeline,
            sky_layout,
            sky_uniform,
            sky_sampler,
            sky_bind,
        };
        scene.resize(device, width, height);
        scene
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("revvy-depth"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.depth = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
    }

    /// `color_key`: el negro puro de las texturas no se dibuja (pistas de Re-Volt).
    pub fn upload_track(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: &[VisualMesh],
        textures: &[(i16, image::RgbaImage)],
        color_key: bool,
    ) {
        let views = textures
            .iter()
            .map(|(page, image)| (*page, rgba_texture(device, queue, image, color_key)))
            .collect::<Vec<_>>();
        self.track = meshes
            .iter()
            .filter(|mesh| !mesh.indices.is_empty())
            .map(|mesh| {
                let view = views
                    .iter()
                    .find(|(page, _)| *page == mesh.texture_page)
                    .map(|(_, view)| view)
                    .unwrap_or(&self.white);
                self.static_mesh(device, queue, mesh, view, Mat4::IDENTITY, 0)
            })
            .collect();
    }

    /// Chasis y ruedas de cada auto. En cada auto, `parts[0]` es el chasis y
    /// `parts[1..=4]` las ruedas; cada parte va a su slot de matriz. Las caras con
    /// textura usan la del auto, sin color key (el negro no es transparente en autos).
    pub fn upload_cars(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cars: &[(&[Vec<VisualMesh>], Option<&image::RgbaImage>)],
    ) {
        let mut out = Vec::new();
        for (index, (parts, texture)) in cars.iter().enumerate().take(MAX_CARS) {
            let view = texture.map(|image| rgba_texture(device, queue, image, false));
            for (part, meshes) in parts.iter().enumerate().take(5) {
                let slot = 1 + index * 5 + part;
                for mesh in meshes.iter().filter(|mesh| !mesh.indices.is_empty()) {
                    let texture = match (&view, mesh.texture_page >= 0) {
                        (Some(view), true) => view,
                        _ => &self.white,
                    };
                    out.push(self.static_mesh(device, queue, mesh, texture, Mat4::IDENTITY, slot as u32));
                }
            }
        }
        if cars.len() > MAX_CARS {
            tracing::warn!(autos = cars.len(), "se dibujan los primeros {MAX_CARS} autos");
        }
        self.car = out;
    }

    pub fn upload_sky(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        faces: &[image::RgbaImage; 6],
    ) {
        let width = faces[0].width();
        let height = faces[0].height();
        if width == 0 || width != height {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("revvy-skybox"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (layer, face) in faces.iter().enumerate() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                face.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * face.width()),
                    rows_per_image: Some(face.height()),
                },
                wgpu::Extent3d {
                    width: face.width(),
                    height: face.height(),
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        self.sky_bind = sky_bind_group(
            device,
            &self.sky_layout,
            &self.sky_uniform,
            &view,
            &self.sky_sampler,
        );
    }

    /// Pista sin cielo: el fondo es liso, del color de la pista (`FOGCOLOR` en las de
    /// Re-Volt) o el de Revvy. No queda el cielo de la pista anterior.
    pub fn clear_sky(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, background: Option<[u8; 3]>) {
        let color = background.map_or(SKY_PLACEHOLDER, |[r, g, b]| [r, g, b, 255]);
        let placeholder = solid_cube(device, queue, color);
        self.sky_bind = sky_bind_group(
            device,
            &self.sky_layout,
            &self.sky_uniform,
            &placeholder,
            &self.sky_sampler,
        );
    }

    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color: &wgpu::TextureView,
        clear: wgpu::Color,
        aspect: f32,
        camera: &CameraView,
        models: &[Mat4],
    ) {
        let Some(depth) = &self.depth else { return };
        let used = (1 + models.len()).min(MODEL_SLOTS);
        let mut model_bytes = vec![0u8; (MODEL_STRIDE as usize) * used];
        for slot in 0..used {
            let matrix = if slot == 0 {
                Mat4::IDENTITY
            } else {
                models.get(slot - 1).copied().unwrap_or(Mat4::IDENTITY)
            };
            let at = slot * MODEL_STRIDE as usize;
            write_mat4(&mut model_bytes[at..at + 64], matrix);
        }
        queue.write_buffer(&self.model_buffer, 0, &model_bytes);
        let view_proj = chase_view_proj(camera.eye, camera.target, aspect);
        let mut bytes = [0u8; UNIFORM_SIZE as usize];
        write_mat4(&mut bytes[0..64], view_proj);
        write_vec4(&mut bytes[64..80], Vec3::new(0.25, 0.92, 0.2).normalize());
        write_vec4(&mut bytes[80..96], Vec3::new(1.0, 0.96, 0.86));
        write_vec4(&mut bytes[96..112], Vec3::new(0.55, 0.70, 0.92));
        queue.write_buffer(&self.uniform, 0, &bytes);
        let mut sky_bytes = [0u8; 80];
        write_mat4(&mut sky_bytes[0..64], view_proj.inverse());
        write_vec4(&mut sky_bytes[64..80], camera.eye);
        queue.write_buffer(&self.sky_uniform, 0, &sky_bytes);

        {
            let mut sky = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("revvy-sky"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            sky.set_pipeline(&self.sky_pipeline);
            sky.set_bind_group(0, &self.sky_bind, &[]);
            sky.draw(0..3, 0..1);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("revvy-scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        for mesh in &self.track {
            draw_mesh(&mut pass, mesh, &self.model_bind);
        }
        if !models.is_empty() {
            for mesh in &self.car {
                draw_mesh(&mut pass, mesh, &self.model_bind);
            }
        }
    }

    fn static_mesh(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &VisualMesh,
        view: &wgpu::TextureView,
        model: Mat4,
        slot: u32,
    ) -> DrawMesh {
        let (vertex, index, index_count) = buffers(device, queue, mesh, model);
        DrawMesh {
            bind: self.bind(device, view),
            vertex,
            index,
            index_count,
            slot,
        }
    }

    fn bind(&self, device: &wgpu::Device, view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("revvy-mesh"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}

fn draw_mesh(pass: &mut wgpu::RenderPass<'_>, mesh: &DrawMesh, model_bind: &wgpu::BindGroup) {
    pass.set_bind_group(0, &mesh.bind, &[]);
    pass.set_bind_group(1, model_bind, &[mesh.slot * MODEL_STRIDE as u32]);
    pass.set_vertex_buffer(0, mesh.vertex.slice(..));
    pass.set_index_buffer(mesh.index.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
}

fn buffers(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &VisualMesh,
    model: Mat4,
) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let vertices = pack_vertices(mesh, model);
    let mut indices = Vec::with_capacity(mesh.indices.len() * 4);
    for index in &mesh.indices {
        indices.extend_from_slice(&index.to_le_bytes());
    }
    let vertex_usage = wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST;
    let vertex = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("revvy-vb"),
        size: vertices.len().max(4) as u64,
        usage: vertex_usage,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertex, 0, &vertices);
    let index = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("revvy-ib"),
        size: indices.len().max(4) as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&index, 0, &indices);
    (vertex, index, mesh.indices.len() as u32)
}

fn create_sky(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    wgpu::BindGroupLayout,
    wgpu::Buffer,
    wgpu::Sampler,
    wgpu::BindGroup,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("revvy-sky"),
        source: wgpu::ShaderSource::Wgsl(SKY_SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("revvy-sky"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("revvy-sky"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("revvy-sky"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("revvy-sky"),
        size: 80,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("revvy-sky"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let placeholder = solid_cube(device, queue, SKY_PLACEHOLDER);
    let bind = sky_bind_group(device, &layout, &uniform, &placeholder, &sampler);
    (pipeline, layout, uniform, sampler, bind)
}

fn sky_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("revvy-sky"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn solid_cube(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("revvy-sky-placeholder"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for layer in 0..6 {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    })
}

fn pack_vertices(mesh: &VisualMesh, model: Mat4) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mesh.positions.len() * VERTEX_STRIDE as usize);
    for i in 0..mesh.positions.len() {
        let position = model.transform_point3(mesh.positions[i]);
        let normal = model
            .transform_vector3(mesh.normals.get(i).copied().unwrap_or(Vec3::Y))
            .normalize_or_zero();
        let uv = mesh.uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        let color = mesh.colors.get(i).copied().unwrap_or([255, 255, 255, 255]);
        bytes.extend_from_slice(&position.x.to_le_bytes());
        bytes.extend_from_slice(&position.y.to_le_bytes());
        bytes.extend_from_slice(&position.z.to_le_bytes());
        bytes.extend_from_slice(&normal.x.to_le_bytes());
        bytes.extend_from_slice(&normal.y.to_le_bytes());
        bytes.extend_from_slice(&normal.z.to_le_bytes());
        bytes.extend_from_slice(&uv[0].to_le_bytes());
        bytes.extend_from_slice(&uv[1].to_le_bytes());
        bytes.extend_from_slice(&color);
    }
    bytes
}

/// Campo de visión de Re-Volt: `BaseGeomPers` 512 sobre una pantalla de 640×480
/// da `2·atan(240/512)` ≈ 50° vertical. El horizontal crece con el aspecto.
fn revolt_vertical_fov() -> f32 {
    2.0 * (240.0f32 / 512.0).atan()
}

fn chase_view_proj(eye: Vec3, target: Vec3, aspect: f32) -> Mat4 {
    let view = glam::camera::rh::view::look_at_mat4(eye, target, Vec3::Y);
    let proj = glam::camera::rh::proj::directx::perspective(
        revolt_vertical_fov(),
        aspect.max(0.1),
        0.025,
        400.0,
    );
    proj * view
}

fn write_mat4(dst: &mut [u8], matrix: Mat4) {
    for (i, value) in matrix.to_cols_array().iter().enumerate() {
        dst[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fn write_vec4(dst: &mut [u8], v: Vec3) {
    for (i, value) in [v.x, v.y, v.z, 0.0].iter().enumerate() {
        dst[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fn solid_texture(device: &wgpu::Device, queue: &wgpu::Queue, rgba: [u8; 4]) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("revvy-white"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: &image::RgbaImage,
    color_key: bool,
) -> wgpu::TextureView {
    // `texture.cpp`: en la pista el color key es el negro puro. Esos texels no se dibujan.
    let mut keyed = image.clone();
    for pixel in keyed.pixels_mut() {
        if color_key && pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
            pixel[3] = 0;
        } else {
            pixel[3] = 255;
        }
    }
    let image = &keyed;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("revvy-bmp"),
        size: wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        image.as_raw(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * image.width()),
            rows_per_image: Some(image.height()),
        },
        wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        },
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Color del cielo de una pista que no trae uno.
const SKY_PLACEHOLDER: [u8; 4] = [120, 170, 220, 255];
const VERTEX_STRIDE: u64 = 36;
const UNIFORM_SIZE: u64 = 112;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const SHADER: &str = r#"
struct Frame {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;
@group(1) @binding(0) var<uniform> model: mat4x4<f32>;

struct Vin {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
}
struct Vout {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

@vertex
fn vs(v: Vin) -> Vout {
    var o: Vout;
    o.clip = frame.view_proj * model * vec4<f32>(v.position, 1.0);
    o.normal = (model * vec4<f32>(v.normal, 0.0)).xyz;
    o.uv = v.uv;
    o.color = v.color;
    return o;
}

@fragment
fn fs(v: Vout) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, tex_sampler, v.uv);
    if (texel.a < 0.5) {
        discard;
    }
    // `DrawCubePolys`: la luz global del mapa es el gouraud guardado en la cara.
    // La textura se modula por ese color. `WORLDRGBPER` 100 lo deja igual.
    let lit = texel.rgb * v.color.rgb;
    return vec4<f32>(lit, 1.0);
}
"#;

const SKY_SHADER: &str = r#"
struct Sky {
    inv_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
}
@group(0) @binding(0) var<uniform> sky: Sky;
@group(0) @binding(1) var sky_tex: texture_cube<f32>;
@group(0) @binding(2) var sky_samp: sampler;

struct Vout {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) id: u32) -> Vout {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var o: Vout;
    o.ndc = pos[id];
    o.clip = vec4<f32>(pos[id], 1.0, 1.0);
    return o;
}

@fragment
fn fs(v: Vout) -> @location(0) vec4<f32> {
    let world = sky.inv_view_proj * vec4<f32>(v.ndc, 1.0, 1.0);
    let dir = normalize(world.xyz / world.w - sky.camera.xyz);
    return vec4<f32>(textureSample(sky_tex, sky_samp, dir).rgb, 1.0);
}
"#;
