/*
 * This needs a rewrite to resturcture functions 
 * into more sizable and readable blocks.
 */
#![feature(int_roundings)]
#![feature(iter_intersperse)]
#![feature(slice_pattern)] 
pub mod font_atlas;
use bytemuck::bytes_of;
use font_atlas::font_atlas::FontAtlas;
use font_atlas::font_atlas::TermConfig;
use font_atlas::glpyh_loader::GlpyhLoader;

use num::integer::Roots;
use wgpu::ImageDataLayout;
use wgpu::TextureFormat;
use wgpu::{include_wgsl, CommandEncoderDescriptor, RenderPipeline};
use wgpu::util::DeviceExt;
use wgpu_types::ImageCopyTexture;
use winit::{
    event::*,
    window::Window
};

use core::slice::SlicePattern;
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
    term_config: TermConfig,
    glpyh_indicies: [u16;6],
    glpyh_indicies_buf: wgpu::Buffer,
}

pub struct ShellBuf {
    pub string_buf: String,
    pub glpyhs_pos: Vec<wgpu::Buffer>
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
    return (seen, s); 
} 

impl State {
    pub async fn new(window: &Window, term_config : TermConfig) -> Self {

        let (surface, mut device, mut queue, config) = 
            Self::surface_config(window).await;

        // load fontatlas
        let font_atlas = FontAtlas::new(term_config.clone(), &mut device,
                                        &mut queue);
        #[cfg(debug_assertions)]
        {
            println!("atlas texture complete creating dbg buffer...");

            let width = (font_atlas.atlas_size.0 * 4).next_multiple_of(256);
            let font_buf = device.create_buffer(
                &wgpu::BufferDescriptor{
                    label: Some("fontatlas dgb buf"),
                    size: width * font_atlas.atlas.height() as u64,
                    usage: wgpu::BufferUsages::MAP_READ | 
                        wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false
                });

            let mut font_enc = device.create_command_encoder(
                &wgpu_types::CommandEncoderDescriptor { label: Some("fontatlas cmd enc")});
                
            font_enc.copy_texture_to_buffer(
                wgpu_types::ImageCopyTexture { texture: &font_atlas.atlas, 
                    mip_level: 0, 
                    origin: wgpu_types::Origin3d {
                        x: 0, 
                        y: 0, 
                        z: 0
                    },
                    aspect: wgpu_types::TextureAspect::All}, 
                wgpu_types::ImageCopyBuffer { 
                    buffer: &font_buf, 
                    layout: ImageDataLayout { 
                        offset: 0, 
                        bytes_per_row: Some(width as u32), 
                        rows_per_image: Some(font_atlas.atlas.height())} 
                },
                wgpu_types::Extent3d { 
                    width: font_atlas.atlas.width(), 
                    height: font_atlas.atlas.height(), 
                    depth_or_array_layers: 1 
            });

            // poll the buffers
            queue.submit(iter::once(font_enc.finish()));
            device.poll(wgpu::Maintain::Wait);

            let slice = font_buf.slice(..);

            let (sender, receiver) = 
                    futures_intrusive::channel::shared::oneshot_channel();
                slice.map_async(wgpu::MapMode::Read, 
                                      move |v| sender.send(v).unwrap());


            println!("polling for entire buffer");

            device.poll(wgpu::Maintain::Wait);
            if let Some(Ok(())) = receiver.receive().await {
                let buf_data = slice.get_mapped_range();

                use image::Rgba;
                // save buffer image as file
                // find next_multiple_of and then divide by bytes
                // match 
                match image::ImageBuffer::
                    <Rgba<u8>, _>::from_raw(width.div_ceil(4) as u32, 
                                            font_atlas.atlas.height() as u32, 
                     buf_data.as_slice()) {
                        Some(ibuf) => match ibuf.save("fontmap.png") {
                            Ok(()) => println!("Image save succesful"),
                            Err(e) => println!("Image save unsuccesful\n {}", e)
                        }
                        None => {
                            println!("Image buffer creation unsuccseful");
                        }
                }
            }
            font_buf.unmap();
        } 

        let (render_pipeline, glpyh_sampler, glpyh_layout) = 
            Self::make_render_pipeline(&mut device, config.format); 

        let glpyh_loader = GlpyhLoader::new(term_config.clone());

        let glpyhs = Self::make_glpyhs(&mut device, &mut queue, 
                                       &font_atlas, 
                                       glpyh_loader, glpyh_sampler, glpyh_layout).await;
     
        // controls indicies for debug code and render
        let glpyh_indicies: [u16;6] = [
            0, 1, 2,
            0, 2, 3, 
        ];

        // create buffer for position
        let glpyh_indicies_buf = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor { 
                label: Some("dgb buffer indicies"),
                contents: bytemuck::cast_slice(&glpyh_indicies.clone()),
                usage: wgpu::BufferUsages::INDEX,
        });

        // pack into struct
        return Self {
                surface,
                device,
                queue,
                config,
                size: window.inner_size(),
                render_pipeline,
                font_atlas,
                shell_buf: ShellBuf{
                    string_buf: String::new(), glpyhs_pos: vec![]
                },
                glpyhs,
                term_config,
                glpyh_indicies,
                glpyh_indicies_buf,
           };
    }
            
    pub fn make_render_pipeline(device: &mut wgpu::Device, 
                                 config: TextureFormat) ->
         (RenderPipeline, wgpu::Sampler, wgpu::BindGroupLayout) {
        
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
                            sample_type: wgpu::TextureSampleType::Float { 
                                filterable: true
                            },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(
                            wgpu::SamplerBindingType::Filtering),
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
        let render_pipeline = 
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format: config,
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
                // Setting this to anything other than
                // Fill requires Features::POLYGON_MODE_LINE
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

        return (render_pipeline, glpyh_sampler, glpyh_layout);
    }

    pub async fn surface_config(window: &Window)
        -> (wgpu::Surface, wgpu::Device, wgpu::Queue, wgpu::SurfaceConfiguration) {
        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let surface = unsafe { instance.create_surface(&window) }.unwrap();
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
                    label: Some("terminal adapter"),
                    features: wgpu::Features::empty(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                #[cfg(debug_assertions)]
                None, // Some(&std::path::Path::new("trace")), // Trace path
                #[cfg(not(debug_assertions))]
                None,
            )
            .await
            .unwrap();

        let size = window.inner_size();
        let capablities = surface.get_capabilities(&adapter);

        let config = wgpu::SurfaceConfiguration {
            alpha_mode: *capablities.alpha_modes.first().unwrap(),
            usage: 
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            view_formats: vec![TextureFormat::Bgra8UnormSrgb],
        };
        surface.configure(&device, &config);
        return (surface, device, queue, config);
     }



       pub async fn make_glpyhs(device: &mut wgpu::Device, 
                                queue: &mut wgpu::Queue, font_atlas: &FontAtlas, glpyh_loader: GlpyhLoader,
                                glpyh_sampler: wgpu::Sampler, glpyh_layout: wgpu::BindGroupLayout) -> 
        HashMap<char, wgpu::BindGroup> {

        #[cfg(debug_assertions)]
        println!("make glpyhs has been called");

        let mut glpyhs: HashMap<char, wgpu::BindGroup> = HashMap::new();

        /* HOTSWAP
        // render each glpyh to texture
        for glpyh in font_atlas.lookup.keys() {
        */
        for glpyh in glpyh_loader.glpyh_map.keys() {
            // create buffer
            let Some(glpyh_buf)
                = glpyh_loader.get_glpyh_data(*glpyh, device, queue).await
                    else { panic!("no glpyh found for {}", glpyh); };

            let buffer_slice = glpyh_buf.slice(..);
            
            // NOTE: We have to create the mapping THEN device.poll() before await
            // the future. Otherwise the application will freeze.
            let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
            device.poll(wgpu::Maintain::Wait);
            rx.receive().await.unwrap().unwrap();

            let data = buffer_slice.get_mapped_range();


            #[cfg(debug_assertions)]
            println!("font_atlas lookup glpyh {}", glpyh);
            {
                #[cfg(debug_assertions)]
                println!("polling device for glpyh {} started", glpyh);

                device.poll(wgpu::Maintain::Wait);

                #[cfg(debug_assertions)]
                println!("found glpyh mapping to texture...");

                    // create buf view and lookup bounding box
                let Some((bbox, _)) = glpyh_loader.glpyh_map.get(glpyh) else 
                    {panic!("no lookup for glpyh")};

                let tex_size = wgpu::Extent3d{
                            height: bbox.height as u32, 
                            width: (bbox.width as u32 * 4).next_multiple_of(256).div_ceil(4),
                            depth_or_array_layers: 1
                        };

                let glpyh_tex = device.create_texture(&wgpu::TextureDescriptor{
                        label: Some(&format!("glpyh_tex {}", glpyh)),
                        size: tex_size,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::COPY_SRC,
                        view_formats: &[wgpu::TextureFormat::Bgra8UnormSrgb],
                });

                #[cfg(debug_assertions)]
                {
                    println!("bbox size ({}, {})", bbox.width, bbox.height);
                }

                let mut glpyh_encoder = device.create_command_encoder(
                    &CommandEncoderDescriptor { label: Some(&format!("glpyh enc {}", glpyh))});

                use num::Integer;
                // write from buffer

                /*
                 * pulling out of the buffer directly doesn't work
                 * glpyh_encoder.copy_buffer_to_texture(
                    wgpu::ImageCopyBuffer { 
                        buffer: &glpyh_buf, 
                        layout: ImageDataLayout { 
                            offset: 
                            ((bbox.height * (bbox.width * 4).next_multiple_of(256))
                                    - bbox.height * bbox.width).prev_multiple_of(&256),
                            bytes_per_row: Some(((bbox.width * 4).next_multiple_of(256)) as u32),
                            rows_per_image: Some(bbox.height as u32)}
                    },
                    ImageCopyTexture { 
                        texture: &glpyh_tex, 
                        mip_level: 0, 
                        origin: wgpu::Origin3d::default(), 
                        aspect: wgpu_types::TextureAspect::All, 
                    }, tex_size);
                */

                queue.write_texture(
                    glpyh_tex.as_image_copy(), 
                    &data.as_slice()[0..
                             (((bbox.width * 4).next_multiple_of(256) * bbox.height) as usize)],
                    ImageDataLayout { 
                        offset: 0, 
                        bytes_per_row: Some((bbox.width * 4).next_multiple_of(256) as u32),
                        rows_per_image: Some(bbox.height as u32)
                    }, 
                    tex_size);

                

               // create view for bindgroup
                let view = glpyh_tex.create_view(&wgpu::TextureViewDescriptor { 
                    label: Some(&format!("tex view {}", glpyh)), 
                    format: Some(wgpu::TextureFormat::Bgra8UnormSrgb), 
                    dimension: Some(wgpu::TextureViewDimension::D2), 
                    aspect: wgpu::TextureAspect::All, 
                    base_mip_level: 0, 
                    mip_level_count: None, 
                    base_array_layer: 0, 
                    array_layer_count: None 
                });

                // submit queue 
                // should only do this once in future revisions
                queue.submit(iter::once(glpyh_encoder.finish()));

                // write texture to bindgroup using device.
                glpyhs.insert(*glpyh, device.create_bind_group(
                    &wgpu::BindGroupDescriptor {
                        layout: &glpyh_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::
                                    TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::
                                    Sampler(&glpyh_sampler),
                            }
                        ],
                        label: Some(&format!("glpyh bindgroup {}", *glpyh))
                    }
                ));

                #[cfg(debug_assertions)]
                println!("glpyh {} inserted to hashmap", *glpyh);

                #[cfg(debug_assertions)]
                println!("polling device for glpyh {} complete", glpyh);
            }

            #[cfg(debug_assertions)]
            println!("starting submitted glpyh queue polling");

            device.poll(wgpu::Maintain::Wait);

            #[cfg(debug_assertions)]
            println!("finished glpyh queue polling");

            // clean up glpyh mapped in font_atlas code
            // glpyh_buf.unmap
            // we do this in dtor not ctor xD
        }
        return glpyhs;
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
        let mut start : (f32, f32) = (0.0,0.0);
        for line in self.shell_buf.string_buf.lines(){
            for cbuf_char in line.chars() {
                let Some(bbox) = self.font_atlas.lookup.get(&cbuf_char) else 
                    { continue; };    

                let bbox_normalized = 
                    ((bbox.0.0 as f64 / self.config.width as f64) as f32,
                     (bbox.0.1 as f64 / self.config.height as f64) as f32);
                // add poisition for next char
                start.0 += bbox_normalized.0; // set as width
                // if start smaller than bbox then set as bbox 
                if start.1 < bbox_normalized.1 { start.1 = bbox_normalized.1}

                #[cfg(debug_assertions)]{
                    println!("bbox normalized for rendered glpyh: ({}, {})",
                        bbox_normalized.0, bbox_normalized.1);
                }

                // create coords:
                // start pos , bbox width + pos, bbox height + height
                let glpyh_vert: &[Vertex] = &[
                    Vertex { position: [start.0 , start.1, 0.0],
                    tex_coords: [0.0, 0.0], }, // b lh corner
                    Vertex { position: [start.0 , (start.1 + bbox_normalized.1)
                        , 0.0], tex_coords: [0.0, 1.0], }, // t lh coner
                    Vertex { position: [(start.0 + bbox_normalized.0), start.1 
                        ,0.0], tex_coords: [1.0, 0.0], }, // b rh corner
                    Vertex { position: [(start.0 + bbox_normalized.0),
                    (start.1 + bbox_normalized.1) ,0.0], tex_coords: [1.0,1.0], },
                    // t rh corner
                ];

                // create buffer for position
                let glpyh_buf = self.device.create_buffer_init(
                    &wgpu::util::BufferInitDescriptor { 
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

    // BIG OVERHEAD, creates copy of renderpipline to debug glpyh usage
    pub async fn debug_glpyhs(&mut self) {
       let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions{
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }).await.unwrap();

        // we use theese instead of self so we can copy over 
        let (mut device, mut queue) = adapter.request_device(&Default::default(),
            None).await.unwrap();

        let (render_pipeline, glpyh_sampler, glpyh_layout) = 
            Self::make_render_pipeline(&mut device, wgpu::TextureFormat::Bgra8UnormSrgb);
        
        // repopulate hashmap in font_atlas
        let font_atlas = FontAtlas::new(self.term_config.clone(), &mut device,
                                        &mut queue);
        let glpyh_loader = GlpyhLoader::new(self.term_config.clone());
        // create second set of bindgroups HOT call
        let glpyhs = 
            pollster::block_on(Self::make_glpyhs(&mut device, &mut queue, 
                              &font_atlas, glpyh_loader, 
                              glpyh_sampler, glpyh_layout));

        
         // create coords:
        // start pos , bbox width + pos, bbox height + height
        let glpyh_vert: &[Vertex] = &[
            Vertex { position: [0.0, 0.0, 0.0],
            tex_coords: [0.0, 0.0], }, // b lh corner
            Vertex { position: [0.0, 1.0
                , 0.0], tex_coords: [0.0, 1.0], }, // t lh coner
            Vertex { position: [1.0, 0.0
                ,0.0], tex_coords: [1.0, 0.0], }, // b rh corner
            Vertex { position: [1.0, 0.0, 0.0], tex_coords: [1.0,1.0], 
            },  // t rh corner
        ];

        // create buffer for position
        let glpyh_positions = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor { 
                label: Some("dgb buffer positions"),
                contents: bytemuck::cast_slice(glpyh_vert),
                usage: wgpu::BufferUsages::VERTEX,
        });

        let glpyh_indicies_buf = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor{
                label: Some("dgb buffer indicies"),
                contents: bytemuck::cast_slice(&self.glpyh_indicies.as_slice()),
                usage: wgpu::BufferUsages::INDEX
            });

        // recreate loader to get texture sizes
        let glpyh_loader = GlpyhLoader::new(self.term_config.clone());
        for (glpyh, bindgroup) in glpyhs {
            
            let Some((bbox, _)) = glpyh_loader.glpyh_map.get(&glpyh)
                else { panic!("no bbox for glpyhs debugging") };

            let texture_desc = wgpu::TextureDescriptor {
                size: wgpu::Extent3d {
                    width: bbox.width as u32,
                    height: bbox.height as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                label: None,
                view_formats: &[wgpu::TextureFormat::Bgra8UnormSrgb]
            };

            let texture = device.create_texture(&texture_desc);
            let texture_view = texture.create_view(&Default::default());

            // we need this for saving images later
            let tex_size = (4 * texture.width()).next_multiple_of(256) * texture.height();

            let glpyh_dgb_buf_desc = wgpu::BufferDescriptor {
                size: tex_size as u64,
                usage: wgpu::BufferUsages::MAP_READ
                    | wgpu::BufferUsages::COPY_DST,
                label: None,
                mapped_at_creation: false,
            };
     
     
            let mut encoder = device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Debug Encoder"),
            });
            // encap for secuirty
            {
                let mut render_pass = encoder.begin_render_pass(
                    &wgpu::RenderPassDescriptor {
                        label: Some("debug Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &texture_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    g: 1.0,
                                    r: 0.0,
                                    b: 0.0,
                                    a: 0.0,
                                }),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });
                render_pass.set_pipeline(&render_pipeline);
                println!("rendering glpyh {} for dgb", glpyh);

                // hack into render pipeline to render glpyh for dbg purposes
                render_pass.set_bind_group(0, &bindgroup, &[]);
                render_pass.set_vertex_buffer(0, glpyh_positions.slice(..));
                render_pass.set_index_buffer(glpyh_indicies_buf.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..4, 1, 0..1);
                println!("glpyh drawn");
            }
            // don't present output
            let glpyh_dbg_buf = device.create_buffer(&glpyh_dgb_buf_desc);
            let mut encoder = device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Debug Encoder"),
            });
 
            // save into buf from texture
            let width = texture.width();
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::ImageCopyBuffer {
                    buffer: &glpyh_dbg_buf,
                    layout: wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some((4 * width).next_multiple_of(256)),
                        rows_per_image: Some(texture.height()),
                    },
                },
                texture.size());

            // submit copy
            queue.submit(iter::once(encoder.finish()));
            device.poll(wgpu::Maintain::Wait);
            

            println!("glpyh copied to buf");

            // pull out the image from buffer
            {
                let buffer_slice = glpyh_dbg_buf.slice(..);
                
                // NOTE: We have to create the mapping THEN device.poll() before await
                // the future. Otherwise the application will freeze.
                let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
                buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                    tx.send(result).unwrap();
                });
                device.poll(wgpu::Maintain::Wait);
                rx.receive().await.unwrap().unwrap();

                let data = buffer_slice.get_mapped_range();

                // save the glpyh to .png
                use image::{ImageBuffer, Rgba};
                let width = (texture.width() * 4).next_multiple_of(256).div_ceil(4);
                let Some(buffer) =
                   ImageBuffer::<Rgba<u8>, _>::from_raw(width, 
                                                         texture.height(),
                                                         data.as_slice()) else {
                       println!("no glpyh printed to debug - QUIET FAIL");
                       return;
                   };

                let result = buffer.save(format!("glpyh_{}.png", glpyh.to_string()));
                if let Err(e) = result {
                    println!("glpyh formatting error, glpyh: {}, error: {}",
                                glpyh, e);
                }
                else if let Ok(()) = result {
                    println!("glpyh {} saved as glpyh_{}.png", glpyh, glpyh); 
                }
                else {
                    println!("Unknown image formatting error");
                }
            }
            glpyh_dbg_buf.unmap();
        }

        return;
    }                   



    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

                    // we ignore DRY here because of debug operations
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
                                        a: 0.0,
                                    }),
                                    store: false,
                                },
                            })],
                            depth_stencil_attachment: None,
                        });
         
                    if !self.shell_buf.string_buf.is_empty() {
                        render_pass.set_pipeline(&self.render_pipeline);
                        #[cfg(debug_assertions)]
                        println!("shellbuf : {}", self.shell_buf.string_buf);

                        for (i, chr) in self.shell_buf.string_buf.chars().enumerate() {
                            
                             #[cfg(debug_assertions)]
                             println!("chr {} printed to shell", chr);

                            if chr != ' ' {
                                if let Some(glpyh) = self.glpyhs.get(&chr) {
                                    if let Some(positions) = 
                                        self.shell_buf.glpyhs_pos.get(i) {
                                        render_pass.set_bind_group(0, &glpyh, &[]);
                                        render_pass.set_vertex_buffer(0, 
                                                                      positions.slice(..));
                                        render_pass.set_index_buffer(self.glpyh_indicies_buf.slice(..), wgpu::IndexFormat::Uint16);
                                        render_pass.draw_indexed(0..4, 1, 0..1);
                                        #[cfg(debug_assertions)]
                                        println!("pixels rendered for glpyh {}", &chr);
                                }
                            }
                        }
                    }
                }
            }
        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
