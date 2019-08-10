use super::*;
use glium::backend::Facade;
use glium::draw_parameters::DrawParameters;

/*
/// Builder for a [`GlyphBrush`](struct.GlyphBrush.html).
///
/// # Example
///
/// ```no_run
/// # extern crate gfx;
/// # extern crate gfx_window_glutin;
/// # extern crate glutin;
/// extern crate gfx_glyph;
/// use gfx_glyph::GlyphBrushBuilder;
/// # fn main() {
/// # let events_loop = glutin::EventsLoop::new();
/// # let (_window, _device, gfx_factory, _gfx_target, _main_depth) =
/// #     gfx_window_glutin::init::<gfx::format::Srgba8, gfx::format::Depth>(
/// #         glutin::WindowBuilder::new(),
/// #         glutin::ContextBuilder::new(),
/// #         &events_loop);
///
/// let dejavu: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
/// let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(dejavu).build(gfx_factory.clone());
/// # let _ = glyph_brush;
/// # }
/// ```
*/

pub struct GlyphBrushBuilder<'a, 'b, H = DefaultSectionHasher> {
    inner: glyph_brush::GlyphBrushBuilder<'a, H>,
    params: DrawParameters<'b>,
}

impl<'a, 'b> GlyphBrushBuilder<'a, 'b> {
    /// Specifies the default font data used to render glyphs.
    /// Referenced with `FontId(0)`, which is default.
    #[inline]
    pub fn using_font_bytes<B: Into<SharedBytes<'a>>>(font_0_data: B) -> Self {
        Self::using_font(Font::from_bytes(font_0_data).unwrap())
    }

    #[inline]
    pub fn using_fonts_bytes<B, V>(font_data: V) -> Self
    where
        B: Into<SharedBytes<'a>>,
        V: Into<Vec<B>>,
    {
        Self::using_fonts(
            font_data
                .into()
                .into_iter()
                .map(|data| Font::from_bytes(data).unwrap())
                .collect::<Vec<_>>(),
        )
    }

    /// Specifies the default font used to render glyphs.
    /// Referenced with `FontId(0)`, which is default.
    #[inline]
    pub fn using_font(font_0: Font<'a>) -> Self {
        Self::using_fonts(vec![font_0])
    }

    pub fn using_fonts<V: Into<Vec<Font<'a>>>>(fonts: V) -> Self {
        GlyphBrushBuilder {
            inner: glyph_brush::GlyphBrushBuilder::using_fonts(fonts),
            params: glium::DrawParameters {
                blend: glium::Blend::alpha_blending(),
                ..Default::default()
            },
        }
    }
}

impl<'a, 'b, H: BuildHasher> GlyphBrushBuilder<'a, 'b, H> {
    delegate_glyph_brush_builder_fns!(inner);

    /*
    /// Sets the depth test to use on the text section **z** values.
    ///
    /// Defaults to: *Always pass the depth test, never write to the depth buffer write*
    ///
    /// # Example
    ///
    /// ```no_run
    /// # extern crate gfx;
    /// # extern crate gfx_glyph;
    /// # use gfx_glyph::GlyphBrushBuilder;
    /// # fn main() {
    /// # let some_font: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
    /// GlyphBrushBuilder::using_font_bytes(some_font)
    ///     .depth_test(gfx::preset::depth::LESS_EQUAL_WRITE)
    ///     // ...
    /// # ;
    /// # }
    /// ```
    pub fn depth_test(mut self, depth_test: gfx::state::Depth) -> Self {
        self.depth_test = depth_test;
        self
    }

    /// Sets the texture filtering method.
    ///
    /// Defaults to `Bilinear`
    ///
    /// # Example
    /// ```no_run
    /// # extern crate gfx;
    /// # extern crate gfx_glyph;
    /// # use gfx_glyph::GlyphBrushBuilder;
    /// # fn main() {
    /// # let some_font: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
    /// GlyphBrushBuilder::using_font_bytes(some_font)
    ///     .texture_filter_method(gfx::texture::FilterMethod::Scale)
    ///     // ...
    /// # ;
    /// # }
    /// ```
    pub fn texture_filter_method(mut self, filter_method: texture::FilterMethod) -> Self {
        self.texture_filter_method = filter_method;
        self
    }
    */

    /*
    /// Sets the section hasher. `GlyphBrush` cannot handle absolute section hash collisions
    /// so use a good hash algorithm.
    ///
    /// This hasher is used to distinguish sections, rather than for hashmap internal use.
    ///
    /// Defaults to [seahash](https://docs.rs/seahash).
    ///
    /// # Example
    /// ```no_run
    /// # extern crate gfx;
    /// # extern crate gfx_glyph;
    /// # use gfx_glyph::GlyphBrushBuilder;
    /// # fn main() {
    /// # let some_font: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
    /// # type SomeOtherBuildHasher = std::collections::hash_map::RandomState;
    /// GlyphBrushBuilder::using_font_bytes(some_font)
    ///     .section_hasher(SomeOtherBuildHasher::default())
    ///     // ...
    /// # ;
    /// # }
    /// ```
    	*/

    pub fn section_hasher<T: BuildHasher>(self, section_hasher: T) -> GlyphBrushBuilder<'a, 'b, T> {
        GlyphBrushBuilder {
            inner: self.inner.section_hasher(section_hasher),
            params: self.params,
        }
    }

    pub fn params<'p>(self, params: DrawParameters<'p>) -> GlyphBrushBuilder<'a, 'p, H> {
        GlyphBrushBuilder {
            inner: self.inner,
            params,
        }
    }

    /// Builds a `GlyphBrush` using the input glium facade
    pub fn build<F: Facade>(self, facade: &F) -> GlyphBrush<'a, 'b, H> {
        let (cache_width, cache_height) = self.inner.initial_cache_size;
        let glyph_brush = self.inner.build();

        static VERTEX_SHADER: &str = include_str!("shader/vert.glsl");
        static FRAGMENT_SHADER: &str = include_str!("shader/frag.glsl");
        let program = Program::from_source(facade, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();

        let texture = Texture2d::empty(facade, cache_width, cache_height).unwrap();
        let index_buffer = glium::index::NoIndices(PrimitiveType::TriangleStrip);

        // We only need this so that we have groups of four
        // instances each which is what the shader expects.
        // Dunno if there is a nicer way to do this than this
        // hack.
        let instances = glium::VertexBuffer::new(facade, &[InstanceVertex { v: 0.0 }; 4]).unwrap();
        let vertex_buffer = glium::VertexBuffer::empty(facade, 0).unwrap();

        GlyphBrush {
            glyph_brush,
            params: self.params,
            program,
            texture,
            index_buffer,
            vertex_buffer,
            instances,
        }
    }
}
