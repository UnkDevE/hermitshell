#![feature(int_roundings)]
#![feature(slice_pattern)]

use hermitshell::State;
mod font_atlas;
use hermitshell::font_atlas::font_atlas::TermConfig;

use portable_pty::{native_pty_system, PtySize, CommandBuilder};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop}, keyboard::PhysicalKey,
};

use std::{io::Read, ptr};

fn read_from_pty(reader: &mut Box<dyn Read + Send>) -> String {
    // make buffer the same as a max data of the terminal
    let mut u8_buf: [u8; 80*24] = [0; 80*24];
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
        PtySize { rows: 80, cols: 24, pixel_width: 18, pixel_height: 18})
            .unwrap();

    // setup window
    let event_loop = EventLoop::new();
    use winit::dpi::LogicalSize;
    let window = event_loop.create_window(window::with_min_inner_size(
        LogicalSize::new(640.0, 1080.0)).build(&event_loop)).unwrap();
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
    let mut state = State::new(&window, TermConfig {font_dir, font_size: 64.0}).await;
    
    // glpyh debug implicit(?) is called in state new
    // println!("GLPYH DEBUG STARTED");
    // pollster::block_on(state.debug_glpyhs());

    // make buffers
    let mut command_str = read_from_pty(&mut reader);
    let mut scratch_buf:String = String::from("");
    // add carage return so that sh command starts up
    write!(pty_pair.master, "\r\n").unwrap();

    state.update();

    event_loop.run_app(&mut state);
}

fn main(){
    pollster::block_on(run());
}

