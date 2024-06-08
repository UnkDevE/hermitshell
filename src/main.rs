#![feature(int_roundings)]
#![feature(slice_pattern)]

use hermitshell::State
mod font_atlas;
use hermitshell::font_atlas::font_atlas::TermConfig;

use winit::event_loop::EventLoop;

fn main(){
    event_loop.run_app(&mut State::Default()).unwrap();
}

