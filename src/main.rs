use portable_pty::{native_pty_system, PtySize, CommandBuilder};

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
}

mod font_atlas;

impl State {
    async fn new(window: &Window) -> Self {
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

        Self {
            surface,
            device,
            queue,
            config,
            size,
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

    fn update(&mut self) {}

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

    // make buffers
    let mut command_buf = read_from_pty(&mut reader);
    let mut scratch_buf:String = String::from("");


    // impl state
    let mut state = State::new(&window).await;

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
                    command_buf.push_str(read_from_pty(&mut reader).as_str());

                    #[cfg(debug_assertions)]
                    println!("{}", command_buf);

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

