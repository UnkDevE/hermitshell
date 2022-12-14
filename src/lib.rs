mod font_atlas;

use font_atlas::font_atlas::FontAtlas;


use wgpu::include_wgsl;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    window::Window
};

use std::iter;

#[repr(C)] #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

 
pub struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    pub font_atlas: FontAtlas,
    glpyhs: HashMap<char, wgpu::BindGroup>,
    pub shell_buf : ShellBuf,
}

pub struct ShellBuf {
    pub string_buf: String,
    pub glpyhs_pos: Vec<wgpu::Buffer>
}
pub struct TermConfig{
    pub font_dir: String,
    pub font_size: f32
}

use std::collections::HashMap;

pub fn remove_duplicates(mut s: String) -> (HashMap<char,i32>, String) {
    let mut seen: HashMap<char, i32> = HashMap::new();
    s.retain(|c| {
        let is_in = seen.contains_key(&c);
        {
            let Some(v) = seen.get_mut(&c) else {
                seen.insert(c, 1);
                return is_in;
            };
            *v += 1;
        }
        return is_in;
    });
    return (seen, s); } impl State {
    pub async fn new(window: &Window, term_config : TermConfig) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                // Some(&std::path::Path::new("trace")), // Trace path
                None,
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            alpha_mode: *surface.get_supported_alpha_modes(&adapter).first().unwrap(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: *surface.get_supported_formats(&adapter).first().unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let font_atlas = FontAtlas::new(term_config.font_dir,
                                        term_config.font_size).await;

        let mut glpyhs: HashMap<char, wgpu::BindGroup> = HashMap::new();

        let glpyh_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        
        let glpyh_layout = 
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("glpyh_bind_group_layout"),
            });

        let shader = device.create_shader_module(include_wgsl!("shader.wgsl"));

        // some boilerplate
        let render_pipeline_layout = device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&glpyh_layout], 
                push_constant_ranges: &[],
            }
        );
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::POLYGON_MODE_LINE
                // or Features::POLYGON_MODE_POINT
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            // If the pipeline will be used with a multiview render pass, this
            // indicates how many array layers the attachments will have.
            multiview: None,
        });

       
        // render each glpyh to texture
        for glpyh in font_atlas.lookup.keys() {
            
            let glpyh_slice = font_atlas.get_glpyh_data(*glpyh);                  
            {
                // NOTE: We have to create the mapping THEN device.poll() before await
                // the future. Otherwise the application will freeze.
                let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
                glpyh_slice.map_async(wgpu::MapMode::Read, move |result| {
                    tx.send(result).unwrap();
                });

                device.poll(wgpu::Maintain::Wait);

                rx.receive().await.unwrap().unwrap();
            }
            // create buf view and lookup bounding box
            let glpyh_data = glpyh_slice.get_mapped_range();
            let Some(bbox) = font_atlas.lookup.get(&glpyh) else {panic!("no lookup for glpyh")};
            
            let tex = device.create_texture_with_data(&queue, 
                &wgpu::TextureDescriptor{
                    label: Some("glpyh_tex"),
                    size: wgpu::Extent3d{
                        height: bbox.1.1 as u32, // height
                        width: bbox.1.0 as u32, // width
                        depth_or_array_layers: 1 
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Uint,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                }, glpyh_data.as_ref());

            // create view for bindgroup
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

            // write texture to bindgroup using device.
            glpyhs.insert(*glpyh, device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    layout: &glpyh_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&glpyh_sampler),
                        }
                    ],
                    label: Some(&format!("glpyh bindgroup {}", *glpyh))
                }
            ));
        }

       // pack into struct
        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            font_atlas,
            shell_buf: ShellBuf{string_buf: String::new(), glpyhs_pos: vec![]},
            glpyhs,
       }
   }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    #[allow(unused_variables)]
    pub fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    pub fn update(&mut self) {
        // set the position for drawing charecters
        let mut start = (0,0);
        for line in self.shell_buf.string_buf.lines(){
            for cbuf_char in line.chars() {
                let Some(bbox) = self.font_atlas.lookup.get(&cbuf_char) else {panic!("no bbox")};
                // add poisition for next char
                start.0 += bbox.0.0; // set as width
                // if start smaller than bbox then set as bbox 
                if start.1 < bbox.0.1 { start.1 = bbox.0.1; }

                // create coords:
                // start pos , bbox width + pos, bbox height + height
                let glpyh_vert: &[Vertex] = &[
                    Vertex { position: [start.0 as f32, start.1 as f32, 0.0], tex_coords: [0.0, 0.0], }, // b lh corner
                    Vertex { position: [start.0 as f32, (start.1 + bbox.0.1) as f32, 0.0], tex_coords: [0.0, 1.0], }, // t lh coner
                    Vertex { position: [(start.0 + bbox.0.0) as f32,start.1 as f32,0.0], tex_coords: [1.0, 0.0], }, // b rh corner
                    Vertex { position: [(start.0 + bbox.0.0) as f32,(start.1 + bbox.0.1) as f32,0.0], tex_coords: [1.0,1.0], }, // t rh corner
                ];

                // create buffer for position
                let glpyh_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { 
                    label: Some(&format!("buffer {}", cbuf_char)),
                    contents: bytemuck::cast_slice(glpyh_vert),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                self.shell_buf.glpyhs_pos.push(glpyh_buf);

            }

            // move down by height 
            start.1 += start.1;
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                g: 0.0,
                                r: 0.0,
                                b: 0.0,
                                a: 1.0,
                            }),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });

                    
            render_pass.set_pipeline(&self.render_pipeline);
            for (i, chr) in self.shell_buf.string_buf.chars().enumerate() {
                let Some(glpyh) = &self.glpyhs.get(&chr)
                    else {
                        panic!("glpyh unsupported");
                    };
                render_pass.set_bind_group(0, &glpyh, &[]);
                render_pass.set_vertex_buffer(0, self.shell_buf.glpyhs_pos.get(i).unwrap().slice(..));
                render_pass.draw(0..4, 0..1);
            }
        }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}


