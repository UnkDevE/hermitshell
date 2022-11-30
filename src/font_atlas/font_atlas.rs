use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;

use std::collections::HashMap;

pub struct FontAtlas {
    pub atlas : wgpu::Buffer,
    // point = (u32, u32) => ((w, h), (x, y))
    pub lookup : HashMap<char, (Point, Point)> 
}

impl FontAtlas {
    
    // creates the font texture atlas given a vector of rasterized glpyhs
    // and positions of where those glpyhs are using a wpu::Buffer
    async fn font_atlas(glyphs_with_bbox: Vec<((BBox, Point), Vec<u8>)>,
            size: Point) -> wgpu::Buffer {

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

        // u8 size for buffer 
        let u8_size = std::mem::size_of::<u8>() as u64;

        // create texture buffer
        let atlas_buf = device.create_buffer(&wgpu::BufferDescriptor{
            size: ((size.0 * size.1) as u64 * u8_size) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ
                | wgpu::BufferUsages::COPY_DST,
            label: None,
            mapped_at_creation: false 
        });

 
        // start write 
        for ((_, point), pixels) in glyphs_with_bbox {
            queue.write_buffer(&atlas_buf,
                (point.0 * point.1) as u64 * u8_size * pixels.len() as u64,
                &pixels);
        }

       return atlas_buf;
    }

    // creates a new FontAtlas struct
    pub async fn new(data: String, font_size: f32)
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
                width: metrics.width as u32,
                height: metrics.height as u32 });
        }

        // pos_boxes is not in order  
        let (size, mut pos_boxes) = packer(&mut bboxes);

        print!("pos box len: {}", pos_boxes.len());

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

        // create atlas texutre set up as image tex
        let atlas = Self::font_atlas(pos_glpyhs, size).await;

        // flush writes and put in GPU
        // atlas.unmap();

        return Self{ atlas, lookup : atlas_lookup}
    }

    // function to get glpyh data on a single char
    // returns wgpu::BufferSlice ready to be rendered as image data
    pub fn get_glpyh_data(&self, glpyh: char) -> wgpu::BufferSlice {
        // get position of char 
        let pos = self.lookup.get(&glpyh).unwrap();
        // x,y coordinates
        let offset_start = (pos.1.0 * pos.1.1) as u64;
        // x,y coords plus w, h
        let offset_end = ((pos.1.0 + pos.0.0) * (pos.1.1 + pos.0.1)) as u64;

        // return glpyh data as slice
        return self.atlas.slice(offset_start..offset_end); 
    }
}

