use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;

use std::collections::HashMap;

pub struct FontAtlas {
    atlas : wgpu::Texture,
    // point = (i16, i16)
    lookup : HashMap<char, (Point, Point)> 
}

impl FontAtlas {
    
    // creates the font texture atlas given a vector of rasterized glpyhs
    // and positions of where those glpyhs are
    async fn font_atlas(&self, glyphs_with_bbox: Vec<((BBox, Point), Vec<u8>)>,
            size: Point) -> wgpu::Texture {

            // gpu boilerplate
            let instance = wgpu::Instance::new(wgpu::Backends::all());

            // create which driver and queue
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

            // create texture
        let atlas = device.create_texture(&atlas_desc);

        // start write 
        for ((bbox, point), pixels) in glyphs_with_bbox {
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
                    offset: (point.0 * point.1) as u64,
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

    pub async fn new(&self, data: String, font_size: f32)
        -> Self {

        // read font from file and load data into abstraction
        let font_data = std::fs::read(data).unwrap();
        let face = fontdue::Font::from_bytes(font_data.as_slice(), 
                                         fontdue::FontSettings::default()).unwrap();

        // calculate scale of the font
        let units_per_em = face.units_per_em();
        let scale = font_size / units_per_em as f32;

        // find raster data and bboxes
        let mut pixels = Vec::new();
        let mut bboxes = Vec::new();
        for (&glyph_c, &id) in face.chars() {
            // convert id -> u16 
            let (metrics, glyph) = 
                face.rasterize_indexed_subpixel(id.into(), scale);

            // push pixel data 
            pixels.push(glyph);
            // push glpyh char with bbox 
            bboxes.push(BBox { glpyh: glyph_c, 
                width: metrics.width as i16,
                height: metrics.height as i16});
        }

        // pos_boxes is not in order  
        let (size, mut pos_boxes) = packer(&mut bboxes);

        // sort by comparing two glpyh positions 
        pos_boxes.sort_by(|(bbox1,_), (bbox2,_)| 
                          face.lookup_glyph_index(bbox1.glpyh)
                          .cmp(&face.lookup_glyph_index(bbox2.glpyh)));

        let mut atlas_lookup : HashMap<char, (Point, Point)> = HashMap::new();

        for (bbox, point) in pos_boxes.clone() {
            atlas_lookup.insert(bbox.glpyh, ((bbox.width, bbox.height), point));
        }

        // zip the pixels with boxes
        let pos_glpyhs = pos_boxes.into_iter().zip(pixels).collect();

        // create atlas texutre
        let atlas = self.font_atlas(pos_glpyhs, size).await;

        return Self{atlas, lookup : atlas_lookup}
    }

    pub fn get_glpyh_data(&self, glpyh: char) -> wgpu::TextureView {
        // get position of char 
        let pos = self.lookup.get(&glpyh);
        use wgpu::TextureViewDescriptor;

        let glpyh_desc = TextureViewDescriptor{
            
        }
        
        self.atlas.create_view()
        
    }
}

