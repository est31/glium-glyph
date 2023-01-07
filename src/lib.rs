#[macro_use]
extern crate glium;
#[macro_use]
pub extern crate glyph_brush;

mod builder;

pub use builder::GlyphBrushBuilder;

use std::borrow::Cow;
use std::hash::{BuildHasher, Hash};
use std::ops::Deref;

use glium::backend::{Context, Facade};
use glium::index::PrimitiveType;
use glium::texture::texture2d::Texture2d;
use glium::texture::{ClientFormat, RawImage2d};
use glium::{Program, Surface};

use glyph_brush::ab_glyph::{point, Font};
use glyph_brush::{
    BrushAction, BrushError, DefaultSectionHasher, FontId, GlyphCruncher, GlyphPositioner, Section,
    SectionGlyphIter,
};
use glyph_brush::{Extra, Rectangle};

#[derive(Copy, Clone, Debug)]
struct GlyphVertex {
    /// screen position
    left_top: [f32; 3],
    right_bottom: [f32; 2],
    /// texture position
    tex_left_top: [f32; 2],
    tex_right_bottom: [f32; 2],
    /// text color
    color: [f32; 4],
}

implement_vertex!(
    GlyphVertex,
    left_top,
    right_bottom,
    tex_left_top,
    tex_right_bottom,
    color
);

#[derive(Copy, Clone, Debug)]
struct InstanceVertex {
    v: f32,
}

implement_vertex!(InstanceVertex, v);

fn rect_to_rect(rect: Rectangle<u32>) -> glium::Rect {
    glium::Rect {
        left: rect.min[0],
        bottom: rect.min[1],
        width: rect.width(),
        height: rect.height(),
    }
}

fn update_texture(tex: &Texture2d, rect: Rectangle<u32>, tex_data: &[u8]) {
    let image = RawImage2d {
        data: std::borrow::Cow::Borrowed(tex_data),
        format: ClientFormat::U8,
        height: rect.height(),
        width: rect.width(),
    };
    tex.write(rect_to_rect(rect), image);
}

#[inline]
fn to_vertex(
    glyph_brush::GlyphVertex {
        mut tex_coords,
        pixel_coords,
        bounds,
        extra,
    }: glyph_brush::GlyphVertex,
) -> GlyphVertex {
    let gl_bounds = bounds;

    let mut gl_rect = glyph_brush::ab_glyph::Rect {
        min: point(pixel_coords.min.x, pixel_coords.min.y),
        max: point(pixel_coords.max.x, pixel_coords.max.y),
    };

    // handle overlapping bounds, modify uv_rect to preserve texture aspect
    if gl_rect.max.x > gl_bounds.max.x {
        let old_width = gl_rect.width();
        gl_rect.max.x = gl_bounds.max.x;
        tex_coords.max.x = tex_coords.min.x + tex_coords.width() * gl_rect.width() / old_width;
    }
    if gl_rect.min.x < gl_bounds.min.x {
        let old_width = gl_rect.width();
        gl_rect.min.x = gl_bounds.min.x;
        tex_coords.min.x = tex_coords.max.x - tex_coords.width() * gl_rect.width() / old_width;
    }
    if gl_rect.max.y > gl_bounds.max.y {
        let old_height = gl_rect.height();
        gl_rect.max.y = gl_bounds.max.y;
        tex_coords.max.y = tex_coords.min.y + tex_coords.height() * gl_rect.height() / old_height;
    }
    if gl_rect.min.y < gl_bounds.min.y {
        let old_height = gl_rect.height();
        gl_rect.min.y = gl_bounds.min.y;
        tex_coords.min.y = tex_coords.max.y - tex_coords.height() * gl_rect.height() / old_height;
    }

    GlyphVertex {
        left_top: [gl_rect.min.x, gl_rect.max.y, extra.z],
        right_bottom: [gl_rect.max.x, gl_rect.min.y],
        tex_left_top: [tex_coords.min.x, tex_coords.max.y],
        tex_right_bottom: [tex_coords.max.x, tex_coords.min.y],
        color: extra.color,
    }
}

/*
/// Object allowing glyph drawing, containing cache state. Manages glyph positioning cacheing,
/// glyph draw caching & efficient GPU texture cache updating and re-sizing on demand.
///
/// Build using a [`GlyphBrushBuilder`](struct.GlyphBrushBuilder.html).
///
/// # Example
///
/// ```no_run
/// # extern crate gfx;
/// # extern crate gfx_window_glutin;
/// # extern crate glutin;
/// extern crate gfx_glyph;
/// # use gfx_glyph::{GlyphBrushBuilder};
/// use gfx_glyph::Section;
/// # fn main() -> Result<(), String> {
/// # let events_loop = glutin::EventsLoop::new();
/// # let (_window, _device, mut gfx_factory, gfx_color, gfx_depth) =
/// #     gfx_window_glutin::init::<gfx::format::Srgba8, gfx::format::Depth>(
/// #         glutin::WindowBuilder::new(),
/// #         glutin::ContextBuilder::new(),
/// #         &events_loop);
/// # let mut gfx_encoder: gfx::Encoder<_, _> = gfx_factory.create_command_buffer().into();
/// # let dejavu: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
/// # let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(dejavu)
/// #     .build(gfx_factory.clone());
/// # let some_other_section = Section { text: "another", ..Section::default() };
///
/// let section = Section {
///     text: "Hello gfx_glyph",
///     ..Section::default()
/// };
///
/// glyph_brush.queue(section);
/// glyph_brush.queue(some_other_section);
///
/// glyph_brush.draw_queued(&mut gfx_encoder, &gfx_color, &gfx_depth)?;
/// # Ok(())
/// # }
/// ```
///
/// # Caching behaviour
///
/// Calls to [`GlyphBrush::queue`](#method.queue),
/// [`GlyphBrush::pixel_bounds`](#method.pixel_bounds), [`GlyphBrush::glyphs`](#method.glyphs)
/// calculate the positioned glyphs for a section.
/// This is cached so future calls to any of the methods for the same section are much
/// cheaper. In the case of [`GlyphBrush::queue`](#method.queue) the calculations will also be
/// used for actual drawing.
///
/// The cache for a section will be **cleared** after a
/// [`GlyphBrush::draw_queued`](#method.draw_queued) call when that section has not been used since
/// the previous draw call.
*/

pub struct GlyphBrush<'a, F: Font, H: BuildHasher = DefaultSectionHasher> {
    glyph_brush: glyph_brush::GlyphBrush<GlyphVertex, Extra, F, H>,
    params: glium::DrawParameters<'a>,
    program: Program,
    texture: Texture2d,
    index_buffer: glium::index::NoIndices,
    vertex_buffer: glium::VertexBuffer<GlyphVertex>,
    instances: glium::VertexBuffer<InstanceVertex>,
}

impl<'p, F: Font> GlyphBrush<'p, F> {
    pub fn new<C: Facade, V: Into<Vec<F>>>(facade: &C, fonts: V) -> Self {
        GlyphBrushBuilder::using_fonts(fonts).build(facade)
    }
}

impl<'p, F: Font + Sync, H: BuildHasher> GlyphBrush<'p, F, H> {
    /// Queues a section/layout to be drawn by the next call of
    /// [`draw_queued`](struct.GlyphBrush.html#method.draw_queued). Can be called multiple times
    /// to queue multiple sections for drawing.
    ///
    /// Used to provide custom `GlyphPositioner` logic, if using built-in
    /// [`Layout`](enum.Layout.html) simply use [`queue`](struct.GlyphBrush.html#method.queue)
    ///
    /// Benefits from caching, see [caching behaviour](#caching-behaviour).
    #[inline]
    pub fn queue_custom_layout<'a, S, G>(&mut self, section: S, custom_layout: &G)
    where
        G: GlyphPositioner,
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush.queue_custom_layout(section, custom_layout)
    }

    /// Queues a section/layout to be drawn by the next call of
    /// [`draw_queued`](struct.GlyphBrush.html#method.draw_queued). Can be called multiple times
    /// to queue multiple sections for drawing.
    ///
    /// Benefits from caching, see [caching behaviour](#caching-behaviour).
    #[inline]
    pub fn queue<'a, S>(&mut self, section: S)
    where
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush.queue(section)
    }

    /*
    /// Draws all queued sections onto a render target.
    /// See [`queue`](struct.GlyphBrush.html#method.queue).
    ///
    /// Trims the cache, see [caching behaviour](#caching-behaviour).
    ///
    /// # Raw usage
    /// Can also be used with gfx raw render & depth views if necessary. The `Format` must also
    /// be provided. [See example.](struct.GlyphBrush.html#raw-usage-1)
    	*/

    #[inline]
    pub fn draw_queued<C: Facade + Deref<Target = Context>, S: Surface>(
        &mut self,
        facade: &C,
        surface: &mut S,
    ) {
        let dims = facade.get_framebuffer_dimensions();
        let transform = [
            [2.0 / (dims.0 as f32), 0.0, 0.0, 0.0],
            [0.0, 2.0 / (dims.1 as f32), 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, -1.0, 0.0, 1.0],
        ];
        self.draw_queued_with_transform(transform, facade, surface)
    }

    /*
    /// Draws all queued sections onto a render target, applying a position transform (e.g.
    /// a projection). The transform applies directly to the `screen_position` coordinates from
    /// queued `Sections`, with the Y axis inverted. Callers must account for the window size in
    /// this transform.
    /// See [`queue`](struct.GlyphBrush.html#method.queue).
    ///
    /// Trims the cache, see [caching behaviour](#caching-behaviour).
    ///
    /// # Raw usage
    /// Can also be used with gfx raw render & depth views if necessary. The `Format` must also
    /// be provided.
    ///
    /// ```no_run
    /// # extern crate gfx;
    /// # extern crate gfx_window_glutin;
    /// # extern crate glutin;
    /// # extern crate gfx_glyph;
    /// # use gfx_glyph::{GlyphBrushBuilder};
    /// # use gfx_glyph::Section;
    /// # use gfx::format;
    /// # use gfx::format::Formatted;
    /// # use gfx::memory::Typed;
    /// # fn main() -> Result<(), String> {
    /// # let events_loop = glutin::EventsLoop::new();
    /// # let (_window, _device, mut gfx_factory, gfx_color, gfx_depth) =
    /// #     gfx_window_glutin::init::<gfx::format::Srgba8, gfx::format::Depth>(
    /// #         glutin::WindowBuilder::new(),
    /// #         glutin::ContextBuilder::new(),
    /// #         &events_loop);
    /// # let mut gfx_encoder: gfx::Encoder<_, _> = gfx_factory.create_command_buffer().into();
    /// # let dejavu: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
    /// # let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(dejavu)
    /// #     .build(gfx_factory.clone());
    /// # let raw_render_view = gfx_color.raw();
    /// # let raw_depth_view = gfx_depth.raw();
    /// # let transform = [[0.0; 4]; 4];
    /// glyph_brush.draw_queued_with_transform(
    ///     transform,
    ///     &mut gfx_encoder,
    ///     &(raw_render_view, format::Srgba8::get_format()),
    ///     &(raw_depth_view, format::Depth::get_format()),
    /// )?
    /// # ;
    /// # Ok(())
    /// # }
    /// ```
    	*/

    pub fn draw_queued_with_transform<C: Facade + Deref<Target = Context>, S: Surface>(
        &mut self,
        transform: [[f32; 4]; 4],
        facade: &C,
        surface: &mut S,
    ) {
        let mut brush_action;
        loop {
            // We need this scope because of lifetimes.
            // Ultimately, we'd like to put the &self.texture
            // into the closure, but that'd inevitably
            // borrow the entirety of self inside the closure.
            // This is a problem with the language and is
            // discussed here:
            // http://smallcultfollowing.com/babysteps/blog/2018/11/01/after-nll-interprocedural-conflicts/
            {
                let tex = &self.texture;
                brush_action = self.glyph_brush.process_queued(
                    |rect, tex_data| {
                        update_texture(tex, rect, tex_data);
                    },
                    to_vertex,
                );
            }
            match brush_action {
                Ok(_) => break,
                Err(BrushError::TextureTooSmall { suggested }) => {
                    let (nwidth, nheight) = suggested;
                    self.texture = Texture2d::empty(facade, nwidth, nheight).unwrap();
                    self.glyph_brush.resize_texture(nwidth, nheight);
                }
            }
        }

        let sampler = glium::uniforms::Sampler::new(&self.texture)
            .wrap_function(glium::uniforms::SamplerWrapFunction::Clamp)
            .minify_filter(glium::uniforms::MinifySamplerFilter::Linear)
            .magnify_filter(glium::uniforms::MagnifySamplerFilter::Linear);

        match brush_action.unwrap() {
            BrushAction::Draw(verts) => {
                self.vertex_buffer = glium::VertexBuffer::new(facade, &verts).unwrap();
            }
            BrushAction::ReDraw => {}
        };

        let uniforms = uniform! {
            font_tex: sampler,
            transform: transform,
        };

        // drawing a frame
        surface
            .draw(
                (&self.instances, self.vertex_buffer.per_instance().unwrap()),
                &self.index_buffer,
                &self.program,
                &uniforms,
                &self.params,
            )
            .unwrap();
    }

    /// Adds an additional font to the one(s) initially added on build.
    ///
    /// Returns a new [`FontId`](struct.FontId.html) to reference this font.
    pub fn add_font<I: Into<F>>(&mut self, font_data: I) -> FontId {
        self.glyph_brush.add_font(font_data)
    }
}

impl<'l, F: Font, H: BuildHasher> GlyphCruncher<F> for GlyphBrush<'l, F, H> {
    fn glyph_bounds_custom_layout<'a, S, L>(
        &mut self,
        section: S,
        custom_layout: &L,
    ) -> Option<glyph_brush::ab_glyph::Rect>
    where
        L: GlyphPositioner + Hash,
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush
            .glyph_bounds_custom_layout(section, custom_layout)
    }

    fn glyphs_custom_layout<'a, 'b, S, L>(
        &'b mut self,
        section: S,
        custom_layout: &L,
    ) -> SectionGlyphIter<'b>
    where
        L: GlyphPositioner + Hash,
        S: Into<Cow<'a, Section<'a>>>,
    {
        self.glyph_brush
            .glyphs_custom_layout(section, custom_layout)
    }

    /// Returns the available fonts.
    ///
    /// The `FontId` corresponds to the index of the font data.
    #[inline]
    fn fonts(&self) -> &[F] {
        self.glyph_brush.fonts()
    }
}
