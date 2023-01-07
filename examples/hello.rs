extern crate glium;
extern crate glium_glyph;

use glium::glutin::{Api, GlProfile, GlRequest};
use glium::{glutin, Surface};

use glium_glyph::glyph_brush::{
    ab_glyph::FontRef, HorizontalAlign, Layout, Section, Text, VerticalAlign,
};
use glium_glyph::GlyphBrushBuilder;

use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};

pub fn main() {
    if cfg!(target_os = "linux") {
        // winit wayland has rendering problems on some setups
        if std::env::var("WINIT_UNIX_BACKEND").is_err() {
            std::env::set_var("WINIT_UNIX_BACKEND", "x11");
        }
    }

    let event_loop = EventLoop::new();
    let window = glutin::window::WindowBuilder::new();
    let context = glutin::ContextBuilder::new()
        .with_gl_profile(GlProfile::Core)
        .with_gl(GlRequest::Specific(Api::OpenGl, (3, 2)))
        .with_srgb(true);
    let display = glium::Display::new(window, context, &event_loop).unwrap();

    let dejavu: &[u8] = include_bytes!("../fonts/DejaVuSans-2.37.ttf");
    let dejavu_font = FontRef::try_from_slice(dejavu).unwrap();

    let mut glyph_brush = GlyphBrushBuilder::using_font(dejavu_font).build(&display);

    event_loop.run(move |event, _tgt, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => (),
            },
            _ => (),
        }
        let screen_dims = display.get_framebuffer_dimensions();

        glyph_brush.queue(
            Section::default()
                .add_text(Text::new("Hello, World!").with_scale(48.0))
                .with_bounds((screen_dims.0 as f32, screen_dims.1 as f32 / 2.0)),
        );
        glyph_brush.queue(
            Section::default()
                .add_text(Text::new("This is in the middle of the screen").with_scale(48.0))
                .with_screen_position((screen_dims.0 as f32 / 2.0, screen_dims.1 as f32 / 2.0))
                .with_bounds((screen_dims.0 as f32, screen_dims.1 as f32))
                .with_layout(
                    Layout::default()
                        .h_align(HorizontalAlign::Center)
                        .v_align(VerticalAlign::Center),
                ),
        );

        let mut target = display.draw();
        target.clear_color_and_depth((1.0, 1.0, 1.0, 0.0), 1.0);
        glyph_brush.draw_queued(&display, &mut target);
        target.finish().unwrap();
    });
}
