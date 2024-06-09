#![feature(int_roundings)]
#![feature(slice_pattern)]

use hermitshell::App;
mod font_atlas;
use hermitshell::font_atlas::font_atlas::TermConfig;

use winit::event_loop::EventLoop;

fn main(){
    let event_loop = EventLoop::new().unwrap();
    let app = &mut App::default();
    event_loop.run_app(app).unwrap();
}

