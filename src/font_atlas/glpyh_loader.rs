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

            // convert rgb to bgr 
            let mut bgr: Vec<u8> = Vec::new();
            for channel in glyph.chunks(3) {
                let mut rgb_chunk = Vec::from(channel);
                rgb_chunk.reverse();
                bgr.append(&mut rgb_chunk);
            }

            // no aplha so we create ours with 255 init
            // put alpha as 0 when colour is fully black
            let mut bgra = Vec::new();
            for channels in glyph.chunks(3) {
                let mut pixel = Vec::from(channels);
                if pixel == [0,0,0]{pixel.push(0)} else{pixel.push(255)}
                bgra.append(&mut pixel);
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

    // pads data from row_width to offset
    // is in U8 bytes or pixels
    // not rgba values
    fn padder(offset : u64, row_width: u64, mut data: Vec<u8>) -> Vec<u8>{
        let chunks = data.chunks_exact_mut(row_width as usize);
        let mut padded : Vec<u8> = vec![];
        for chunk in chunks {
            let mut row = chunk.to_owned();
            row.append(&mut vec![0 as u8; offset as usize]);
            padded.append(&mut row);
        }
        return padded;

    }

	/*
		depads the padding set to the buffer for compatibility
		in wgpu
	*/
	pub fn depadder(data: Vec<u8>, width : u64) -> Vec<u8>{
		// width de-padding code
		return data
			.chunks((width * 4).next_multiple_of(256) as usize)
			.fold(Vec::new(), |mut init, acc| {
				init.extend_from_slice(&acc[0..(width *4) as usize]);
				return init;
		}).to_owned();
	}

    pub async fn get_glpyh_data(&self, glpyh: char, 
                                device : &mut wgpu::Device, 
                                queue: &mut wgpu::Queue) -> Option<wgpu::Buffer> {
        if let Some((bbox, data)) = self.glpyh_map.get(&glpyh) {
            
            use wgpu::CommandEncoderDescriptor;
            let mut encoder = device.create_command_encoder(
                &CommandEncoderDescriptor { label: Some("glpyh_loader_enc") });

            let size = 
                bbox.height * (4 * bbox.width).next_multiple_of(256); 

            // calculate the offset for each row,
            // so bytes wrap round to the next row of the image
            let offset = (bbox.width * 4).next_multiple_of(256) - 
                (bbox.width * 4);
            let padded = 
                GlpyhLoader::padder(offset, bbox.width * 4, data.to_owned());
           
            
            #[cfg(debug_assertions)]
            {
                match image::save_buffer(&format!("get_glpyh_{}.png", glpyh),
                data.as_slice(), 
                    bbox.width as u32, bbox.height as u32,
                    image::ColorType::Rgba8) {
                    Ok(()) => {}
                    Err(e) => {
                        println!("glpyh {} error {}", glpyh, e);
                    }
                }
            }

            use wgpu::BufferDescriptor;
            let glpyh_buf = device.create_buffer(&BufferDescriptor{
                label: Some(&format!("glpyh {} buf internal", glpyh)),
                size,
                usage: wgpu::BufferUsages::MAP_READ |
                        wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            
            queue.write_buffer(&glpyh_buf, 0, &padded);

            use std::iter;
            queue.submit(iter::once(encoder.finish()));
            device.poll(wgpu::Maintain::Wait);

            return Some(glpyh_buf);
        }
        return None;
    }


}
