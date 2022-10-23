use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;

// creates the font texture atlas given a vector of rasterized glpyhs
// and positions of where those glpyhs are
async fn font_atlas(glyphs_with_bbox: Vec<(Vec<u8>, BBox)>, size: Point) -> wgpu::Texture {

    // gpu boilerplate
    let instance = wgpu::Instance::new(wgpu::Backends::all());


    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();

    // create texture atlas var
    let atlas_desc = wgpu::TextureDescriptor {
        size: wgpu::Extent3d{
            width: size.0 as u32,
            height: size.1 as u32,
            depth_or_array_layers: 1,

        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::RENDER_ATTACHMENT
        ,
        label: None

    };

    let atlas = device.create_texture(&atlas_desc);

    for (pixels, bbox) in glyphs_with_bbox {
        queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::ImageCopyTexture {
                texture: &atlas,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            &pixels,

            // The layout of the texture
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * bbox.width as u32),
                rows_per_image: None, 
            },

            wgpu::Extent3d{
                width: bbox.width as u32,
                height: bbox.height as u32,
                depth_or_array_layers: 1 
            }
        );
    }

    return atlas;
}

pub fn read_font(data: String, font_size: f32)
    -> () {


    // read font from file and load data into abstraction
    let font_data = std::fs::read(data).unwrap();
    let face = fontdue::Font::from_bytes(font_data.as_slice(), 
                                     fontdue::FontSettings::default()).unwrap();

    // calculate scale of the font
    let units_per_em = face.units_per_em();
    let scale = font_size / units_per_em as f32;

    // invert the char hashmap of font
    let inv_hash = || -> Vec<(u16, char)> {
        let mut inv_vec = Vec::new();
        face.chars().clone().drain().map(|x| {
            inv_vec.push((x.1.into(), x.0))
        });

        return inv_vec;
    }();

    // find id code
    let find_char = | id  : u16 | -> char{
        for hash in inv_hash.clone(){
            if id == hash.0 {
                return hash.1;
            }
        }
        return '\0';
    };

    // find glpyhs and bboxes
    let mut glyphs = Vec::new();
    let mut bboxes = Vec::new();
    for id in 0..face.glyph_count() {
        let (metrics, glyph) = face.rasterize_indexed_subpixel(id, scale);
        // get glpyh and bbox
        glyphs.push(glyph);
        // push glpyh id to reverse into hashmap
        bboxes.push(BBox { glpyh: find_char(id),
            width: metrics.width as i16,
            height: metrics.height as i16});
    }

    let (pos_boxes, sizes) = packer(bboxes);
}
