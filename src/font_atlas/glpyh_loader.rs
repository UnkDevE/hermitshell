use core::slice::SlicePattern;
use crate::TermConfig;
use std::collections::HashMap;

#[derive(Clone)]
pub struct BBox {
    pub glpyh: char,
    pub width: u64,
    pub height: u64,
}

impl PartialEq for BBox {
    fn eq(&self, other: &Self) -> bool {
        return self.width == other.width && self.height == other.height;
    }
}

pub struct GlpyhLoader {
    pub glpyh_map: HashMap<char, (BBox, Vec<u8>)>
}

impl GlpyhLoader {

    pub fn new(term_config: TermConfig) -> Self {
        let data = term_config.font_dir;
        let font_size = term_config.font_size;

        // read font from file and load data into abstraction
        let font_data = std::fs::read(data).unwrap();
        let face = fontdue::Font::from_bytes(font_data.as_slice(), 
                                     fontdue::FontSettings::default()).unwrap();

        // find raster data and bboxes
        let mut pixels : Vec<((char, Vec<u8>), BBox)> = Vec::new();
        let mut glpyh_map = HashMap::new();
        for (&glyph_c, &id) in face.chars() {
            // convert id -> u16 
            let (metrics, glyph) = 
                face.rasterize_indexed_subpixel(id.into(), font_size);
                        // use px 

            // no aplha so we create ours with 255 init
            let mut rgba = Vec::new();
            for channels in glyph.chunks(3) {
                let mut pixel = Vec::from(channels);
                pixel.push(255);
                rgba.append(&mut pixel);
            }

            // convert rgba to bgra
            let mut bgra: Vec<u8> = Vec::new();
            for channel in rgba.chunks(4) {
                let mut bgra_chunk = Vec::from(channel);
                bgra_chunk.reverse();
                bgra.append(&mut bgra_chunk);
            }
            
            // push pixel data 
            // null char has problems with encoding
            if  !(metrics.width == 0 || metrics.height == 0 || glyph_c == '\0') {
                pixels.push(((glyph_c, bgra), 
                    BBox { glpyh: glyph_c, 
                    width: metrics.width as u64,
                    height: metrics.height as u64 }));
                
            }

            for ((glpyh, data), bbox) in pixels.clone() {
                glpyh_map.insert(glpyh, (bbox, data));
            }
        }

        return Self {glpyh_map};
    }

    pub async fn get_glpyh_data(&self, glpyh: char, 
                                device : &mut wgpu::Device, 
                                queue: &mut wgpu::Queue) -> Option<wgpu::Buffer> {
        if let Some((bbox, data)) = self.glpyh_map.get(&glpyh) {
            
            use wgpu::CommandEncoderDescriptor;
            let mut encoder = device.create_command_encoder(
                &CommandEncoderDescriptor { label: Some("glpyh_loader_enc") });

            let size = 
                bbox.height * (4 * bbox.width).next_multiple_of(256).div_ceil(4); 

            use wgpu::BufferDescriptor;
            let glpyh_buf = device.create_buffer(&BufferDescriptor{
                label: Some(&format!("glpyh {} buf internal", glpyh)),
                size,
                usage: wgpu::BufferUsages::MAP_READ |
                        wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            
            queue.write_buffer(&glpyh_buf, 0, data);

            use std::iter;
            queue.submit(iter::once(encoder.finish()));
            device.poll(wgpu::Maintain::Wait);

            return Some(glpyh_buf);
        }
        return None;
    }


}
