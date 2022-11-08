use portable_pty::{native_pty_system, PtySize, CommandBuilder};
mod font_atlas;
use font_atlas::font_atlas::FontAtlas;

use wgpu::Sampler;
use winit::{
    event::*,
    window::Window,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use std::io::Read;
use std::iter;

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    font_atlas: FontAtlas,
    command_buf: String,
    textures: HashMap<char, wgpu::BindGroup>
}


struct TermConfig{
    font_dir: String,
    font_size: f32
}

use std::collections::HashMap;

fn remove_duplicates(mut s: String) -> (HashMap<char,i32>, String) {
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
    async fn new(window: &Window, term_config : TermConfig) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let font_atlas = FontAtlas::new(term_config.font_dir,
                                        term_config.font_size).await;

        let mut textures: HashMap<char, wgpu::BindGroup> = HashMap::new();

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

        // render each glpyh to texture
        for glpyh in font_atlas.lookup.keys() {
            
            let glpyh_slice = font_atlas.get_glpyh_data(*glpyh);                  
            {
                // NOTE: We have to create the mapping THEN device.poll() before await
                // the future. Otherwise the application will freeze.
                let res = glpyh_slice.map_async(wgpu::MapMode::Read).await;
                device.poll(wgpu::Maintain::Wait);
                if res.is_err() {
                    panic!("error in buf read");
                }

            }
            // create buf view
            let glpyh_data = glpyh_slice.get_mapped_range();
            let Some(bbox) = font_atlas.lookup.get(&glpyh) else {panic!("no lookup for glpyh")};
            
            use wgpu::util::DeviceExt;
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
            textures.insert(*glpyh, device.create_bind_group(
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

        Self {
            surface,
            device,
            queue,
            config,
            size,
            font_atlas,
            command_buf: String::new(), 
            textures 
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
    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }


    fn update(&mut self) {
        // glpyh encoder for writing glpyhs to surface
        let mut glpyh_enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Glpyh Encoder"),
            });


        let surface = self.surface.get_current_texture().unwrap();

        // set the position for drawing charecters
        let mut start = (0,0);
        for cbuf_char in self.command_buf.chars() {
            let Some(tex) = self.textures.get(&cbuf_char) else {panic!("no tex")};
            let Some(bbox) = self.font_atlas.lookup.get(&cbuf_char) else {panic!("no bbox")};

            // TODO: create bindgroup  
            // add poisition for next char
            start.0 += bbox.0.0; // set as width
            start.1 += bbox.0.1; // set as height
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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
            let _render_pass = encoder.begin_render_pass(
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
                    }).unwrap()],
                    depth_stencil_attachment: None,
                });
            }

        self.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}


fn read_from_pty(reader: &mut Box<dyn Read + Send>) -> String {
    // make buffer the same as a max data of the terminal
    let mut u8_buf: [u8; (80*24)] = [0; (80*24)];
    reader.read(&mut u8_buf).unwrap();

    // convert u8 buffer to string
    let str_buf:String = String::from_utf8(u8_buf.to_vec()).unwrap();

    // return the output without left over data
    return str_buf.trim_end().to_string();
}


pub async fn run(){
    // wasm not supported in this project yet 
    #[cfg(target_arch = "wasm32")]{
        panic!("WASM not supported");
    }

    env_logger::init();

    // setup pty system
    let pty_system = native_pty_system();
    // TODO: set pixel size to font size
    let mut pty_pair = pty_system.openpty(
        PtySize { rows: 80, cols: 24, pixel_width: 0, pixel_height: 0})
            .unwrap();

    // setup window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("hermitshell");


    // spawn os-specific shell
    #[cfg(target_os = "windows")]
    let cmd = CommandBuilder::new("cmd");
    #[cfg(target_os = "linux")]
    let cmd = CommandBuilder::new("bash");
    #[cfg(target_os = "macos")]
    let cmd = CommandBuilder::new("bash");
    pty_pair.slave.spawn_command(cmd).unwrap();

    // create command reader for read_from_pty fn
    let mut reader = pty_pair.master.try_clone_reader().unwrap();

       use std::env;

    let Some(font_dir) = env::args().nth(1) else {todo!()};
    
    // impl state
    let mut state = State::new(&window, TermConfig { font_dir, font_size: 18.0}).await;

    // make buffers
    state.command_buf = read_from_pty(&mut reader);
    let mut scratch_buf:String = String::from("");

    // if <ESC> close window
    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    state:ElementState::Pressed,
                    virtual_keycode: Some(VirtualKeyCode::Back),
                    ..},
                ..} => {
                    // pop used to remove last char not for output
                    scratch_buf.pop();
                }
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    state:ElementState::Released,
                    virtual_keycode: Some(VirtualKeyCode::Return),
                    ..},
                ..} => {
                    // set window to and run command
                    window.set_title(format!("hermitshell - {}", scratch_buf)
                                     .as_str());
                    writeln!(pty_pair.master, "{}\r", scratch_buf).unwrap();

                    // clear buffer for next cmd
                    scratch_buf.clear();
                    // push output to buffer
                    state.command_buf.push_str(read_from_pty(&mut reader).as_str());

                    #[cfg(debug_assertions)]
                    println!("{}", state.command_buf);

                    // redraw window with output
                    window.request_redraw();
                }
            WindowEvent::ReceivedCharacter(char_grabbed) =>{
                // char is borrowed here so we clone
                scratch_buf.push(char_grabbed.clone());
                window.request_redraw();
            }
            _ => {}
        },
        Event::RedrawRequested(win_id) if win_id == window.id() =>{
            state.update();
            match state.render() {
                Ok(_) => {}
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                // The system is out of memory, we should probably quit
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    *control_flow = ControlFlow::Exit;
                }
                // All other errors (Outdated, Timeout)
                // should be resolved by the next frame
                Err(e) => eprintln!("{:?}", e),
            }
        }
        _ => {}
    });


}

fn main(){
    pollster::block_on(run());
}

