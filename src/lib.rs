#[macro_use]
extern crate glium;
pub extern crate glyph_brush;

use std::borrow::Cow;
use std::ops::Deref;
use std::hash::BuildHasher;

use glium::backend::{Facade, Context};
use glium::{Surface, Program, Frame};
use glium::index::PrimitiveType;
use glium::texture::{RawImage2d, ClientFormat};
use glium::texture::texture2d::Texture2d;

use glyph_brush::rusttype::{Rect, SharedBytes};
use glyph_brush::{
	rusttype::{point, Font}, BrushAction, BrushError, DefaultSectionHasher, FontId, VariedSection, GlyphPositioner,
};

const IDENTITY_MATRIX4: [[f32; 4]; 4] = [
	[1.0, 0.0, 0.0, 0.0],
	[0.0, 1.0, 0.0, 0.0],
	[0.0, 0.0, 1.0, 0.0],
	[0.0, 0.0, 0.0, 1.0],
];

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

implement_vertex!(GlyphVertex, left_top, right_bottom, tex_left_top,
	tex_right_bottom, color);

#[derive(Copy, Clone, Debug)]
struct InstanceVertex {
	v: f32,
}

implement_vertex!(InstanceVertex, v);

fn rect_to_rect(rect: Rect<u32>) -> glium::Rect {
	glium::Rect {
		left: rect.min.x,
		bottom: rect.min.y,
		width: rect.width(),
		height: rect.height(),
	}
}

fn update_texture(tex: &Texture2d, rect: Rect<u32>, tex_data: &[u8]) {
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
		screen_dimensions: (screen_w, screen_h),
		color,
		z,
	}: glyph_brush::GlyphVertex,
) -> GlyphVertex {
	let gl_bounds = Rect {
		min: point(
			2.0 * (bounds.min.x / screen_w - 0.5),
			2.0 * (0.5 - bounds.min.y / screen_h),
		),
		max: point(
			2.0 * (bounds.max.x / screen_w - 0.5),
			2.0 * (0.5 - bounds.max.y / screen_h),
		),
	};

	let mut gl_rect = Rect {
		min: point(
			2.0 * (pixel_coords.min.x as f32 / screen_w - 0.5),
			2.0 * (0.5 - pixel_coords.min.y as f32 / screen_h),
		),
		max: point(
			2.0 * (pixel_coords.max.x as f32 / screen_w - 0.5),
			2.0 * (0.5 - pixel_coords.max.y as f32 / screen_h),
		),
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
	// note: y access is flipped gl compared with screen,
	// texture is not flipped (ie is a headache)
	if gl_rect.max.y < gl_bounds.max.y {
		let old_height = gl_rect.height();
		gl_rect.max.y = gl_bounds.max.y;
		tex_coords.max.y = tex_coords.min.y + tex_coords.height() * gl_rect.height() / old_height;
	}
	if gl_rect.min.y > gl_bounds.min.y {
		let old_height = gl_rect.height();
		gl_rect.min.y = gl_bounds.min.y;
		tex_coords.min.y = tex_coords.max.y - tex_coords.height() * gl_rect.height() / old_height;
	}

	GlyphVertex {
		left_top: [gl_rect.min.x, gl_rect.max.y, z],
		right_bottom: [gl_rect.max.x, gl_rect.min.y],
		tex_left_top: [tex_coords.min.x, tex_coords.max.y],
		tex_right_bottom: [tex_coords.max.x, tex_coords.min.y],
		color,
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

pub struct GlyphBrush<'font, 'a, H :BuildHasher = DefaultSectionHasher> {
	glyph_brush :glyph_brush::GlyphBrush<'font, H>,
	params :glium::DrawParameters<'a>,
	program :Program,
	texture :Texture2d,
	index_buffer :glium::index::NoIndices,
	verts :Vec<GlyphVertex>,
}

static VERTEX_SHADER :&str = include_str!("shader/vert.glsl");
static FRAGMENT_SHADER :&str = include_str!("shader/frag.glsl");

impl<'font, 'p> GlyphBrush<'font, 'p> {
	pub fn new<'a :'font, F :Facade, V :Into<Vec<Font<'a>>>>(facade :&F, fonts :V) -> Self {
		let glyph_brush = glyph_brush::GlyphBrushBuilder::using_fonts(fonts).build();
		let program = Program::from_source(facade, VERTEX_SHADER,
			FRAGMENT_SHADER, None).unwrap();
		let params = glium::DrawParameters {
			depth: glium::Depth {
				test: glium::DepthTest::IfLess,
				write: true,
				.. Default::default()
			},
			blend: glium::Blend::alpha_blending(),
			.. Default::default()
		};
		let (twidth, theight) = glyph_brush.texture_dimensions();
		let texture = Texture2d::empty(facade, twidth, theight).unwrap();
		let index_buffer = glium::index::NoIndices(PrimitiveType::TriangleStrip);
		GlyphBrush {
			glyph_brush,
			params,
			program,
			texture,
			index_buffer,
			verts : Vec::new(),
		}
	}
}

impl<'font, 'p, H :BuildHasher> GlyphBrush<'font, 'p, H> {
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
		S: Into<Cow<'a, VariedSection<'a>>>,
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
		S: Into<Cow<'a, VariedSection<'a>>>,
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
	pub fn draw_queued<F :Facade + Deref<Target = Context>, T :Fn() -> Frame>(&mut self, facade :&F, draw_fn :T) -> Frame {
		self.draw_queued_with_transform(IDENTITY_MATRIX4, facade, draw_fn)
	}

	/*
	/// Draws all queued sections onto a render target, applying a position transform (e.g.
	/// a projection).
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

	pub fn draw_queued_with_transform<F :Facade + Deref<Target = Context>, T :Fn() -> Frame>(&mut self, transform :[[f32; 4]; 4],  facade :&F, draw_fn :T) -> Frame {
		let screen_dims = facade.get_framebuffer_dimensions();
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
					screen_dims,
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
				},
			}
		}

		let sampler = glium::uniforms::Sampler::new(&self.texture)
			.wrap_function(glium::uniforms::SamplerWrapFunction::Clamp)
			.minify_filter(glium::uniforms::MinifySamplerFilter::Linear)
			.magnify_filter(glium::uniforms::MagnifySamplerFilter::Linear);

		match brush_action.unwrap() {
			BrushAction::Draw(verts) => {
				self.verts = verts;
			},
			BrushAction::ReDraw => {},
		};

		let vertex_buffer = glium::VertexBuffer::new(facade, &self.verts).unwrap();

		// We only need this so that we have groups of four
		// instances each which is what the shader expects.
		// Dunno if there is a nicer way to do this than this
		// hack.
		let instances = glium::VertexBuffer::new(facade, &[InstanceVertex{ v : 0.0}; 4]).unwrap();

		let uniforms = uniform! {
			font_tex: sampler,
			transform: transform,
		};

		// drawing a frame
		let mut target = draw_fn();
		target.draw((&instances, vertex_buffer.per_instance().unwrap()), &self.index_buffer, &self.program, &uniforms, &self.params).unwrap();
		target
	}

	/// Returns the available fonts.
	///
	/// The `FontId` corresponds to the index of the font data.
	#[inline]
	pub fn fonts(&self) -> &[Font<'font>] {
		self.glyph_brush.fonts()
	}

	/*
	/// Adds an additional font to the one(s) initially added on build.
	///
	/// Returns a new [`FontId`](struct.FontId.html) to reference this font.
	///
	/// # Example
	///
	/// ```no_run
	/// # extern crate gfx;
	/// # extern crate gfx_window_glutin;
	/// # extern crate glutin;
	/// extern crate gfx_glyph;
	/// use gfx_glyph::{GlyphBrushBuilder, Section};
	/// # fn main() {
	/// # let events_loop = glutin::EventsLoop::new();
	/// # let (_window, _device, mut gfx_factory, gfx_color, gfx_depth) =
	/// #     gfx_window_glutin::init::<gfx::format::Srgba8, gfx::format::Depth>(
	/// #         glutin::WindowBuilder::new(),
	/// #         glutin::ContextBuilder::new(),
	/// #         &events_loop);
	/// # let mut gfx_encoder: gfx::Encoder<_, _> = gfx_factory.create_command_buffer().into();
	///
	/// // dejavu is built as default `FontId(0)`
	/// let dejavu: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
	/// let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(dejavu).build(gfx_factory.clone());
	///
	/// // some time later, add another font referenced by a new `FontId`
	/// let open_sans_italic: &[u8] = include_bytes!("../../fonts/OpenSans-Italic.ttf");
	/// let open_sans_italic_id = glyph_brush.add_font_bytes(open_sans_italic);
	/// # glyph_brush.draw_queued(&mut gfx_encoder, &gfx_color, &gfx_depth).unwrap();
	/// # let _ = open_sans_italic_id;
	/// # }
	/// ```
	*/

	pub fn add_font_bytes<'a: 'font, B: Into<SharedBytes<'a>>>(&mut self, font_data: B) -> FontId {
		self.glyph_brush.add_font_bytes(font_data)
	}

	/// Adds an additional font to the one(s) initially added on build.
	///
	/// Returns a new [`FontId`](struct.FontId.html) to reference this font.
	pub fn add_font<'a: 'font>(&mut self, font_data: Font<'a>) -> FontId {
		self.glyph_brush.add_font(font_data)
	}
}
