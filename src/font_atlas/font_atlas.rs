use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;
use crate::font_atlas::packer::area_protect;

use std::collections::HashMap;
use wgpu::CommandEncoderDescriptor;
use wgpu::MAP_ALIGNMENT;
use wgpu::COPY_BUFFER_ALIGNMENT;

const CHANNELS: u64 = 4;

#[derive(Clone)]
pub struct TermConfig{
    pub font_dir: String,
    pub font_size: f32
}


fn is_multiple_of(n : u128, multiple: u128) -> bool
{
    if n == 0 || multiple == 0 {
        return false;
    }
    return n&(multiple - 1) == 0;
}

pub struct FontAtlas {
    pub atlas : wgpu::Buffer,
    // point = (u64, u64) => ((w, h), offset, (x, y))
    pub lookup : HashMap<char, (Point, u128, Point)>,
    pub atlas_size : Point,
}

impl FontAtlas {
    
    // creates the font texture atlas given a vector of rasterized glpyhs
    // and positions of where those glpyhs are using a wpu::Buffer
    // device is locked so need reference
    fn font_atlas(pixels: &mut Vec<u8>, 
                  device: &mut wgpu::Device, queue: &mut wgpu::Queue) -> 
        wgpu::Buffer {

        let enc = device.create_command_encoder(
            &CommandEncoderDescriptor { label: Some("font_atlas_enc") });

        // size conversion error handling next
        let u64_size = <usize as TryInto::<u64>>::try_into(pixels.len())
            .unwrap();

        // create texture buffer
        let atlas_buf = device.create_buffer(&wgpu::BufferDescriptor{
            size: u64_size,
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::MAP_READ,
            label: Some("atlas_buffer"),
            mapped_at_creation: false 
        });

        
        // write as group 
        queue.write_buffer(&atlas_buf, 0, pixels.as_slice()); 

        // submit queue with empty command buffer to write to gpu
        use std::iter;
        queue.submit(iter::once(enc.finish()));

        #[cfg(debug_assertions)]
        println!("buffer submitted returning function...");

        device.poll(wgpu::Maintain::Wait);

        #[cfg(debug_assertions)]
        println!("buffer complete");

        return atlas_buf;
    }

    // force pixels into alignment by adding to end
    fn aligner_start (mut pixels : Vec<u8>, offset: usize) -> Vec<u8> {
        use std::iter;
        let mut offset_vec: Vec<u8> = 
                iter::repeat(0).take(offset).collect();
        offset_vec.append(&mut pixels);
        return pixels;
    } 

    // add whitespace to pixels through pos_boxes
    fn add_whitespace(pixels : Vec<u8>, pos_boxes : Vec<(BBox, (u64, u64))>) -> Vec<u8> {
        let mut old_pixels = pixels.clone();
        // assuming pos_boxes are sorted and aligned with pixels
        for windows in pos_boxes.windows(2) {
            let bbox = &windows[0].0; 
            let pos = windows[0].1;
            let pixel_end = (bbox.width + pos.0) * (bbox.height * pos.1); 
            // take the position of the next element
            let end_pos = windows[1].1.0 * windows[1].1.1; 

            for idx in pixel_end..end_pos {
                old_pixels.insert(idx as usize, 0); // insert whitespace 
                                                    // for pixel
            }
        }

        return old_pixels;
    }

    // force pixels into alignment
    // puts in whitespace where packer has left it 
    fn record_align_to_offset (pixels : Vec<u8>, 
                        pos_boxes : Vec<(BBox, (u64, u64))>) 
            -> (Vec<u8>, HashMap<char, (Point, u128, Point)>){

        let mut new_pixels: Vec<u8> = vec![0; MAP_ALIGNMENT as usize]; 
        let mut positions: HashMap<char,
            (Point, u128, Point)> = HashMap::new();

        let last_glpyh = '\0';
        for (bbox, pos) in pos_boxes.into_iter() {

            // flatten out position from packer
            let mut start = (area_protect(pos.0) * area_protect(pos.1) 
                             * CHANNELS) as usize;
            if start == 1 {start = 0;} // get rid of carryover

            // find endpoint
            let mut end =  start + (((bbox.width * bbox.height) 
                                     * CHANNELS) as usize);

            // if in range
            if let Some(mut glpyh) = pixels.get(start..end)
                .and_then(|x| Some(x.to_vec())) {

                // start check to alginment
                if !is_multiple_of(start as u128, MAP_ALIGNMENT.into()) {
                    let padding = start.next_multiple_of(MAP_ALIGNMENT as usize);
                    if last_glpyh != '\0' {
                        positions.get_mut(&last_glpyh).and_then(|bbox| {
                            bbox.1 += (padding as i64 - bbox.2.1 as i64)
                                .abs() as u128; 
                            bbox.2.1 = padding as u64;
                            return Some(bbox);
                        });
                    } 
                    // set start to the next MAP_ALIGNMENT
                    // recalc end for finding start.
                    start = padding;
                    end =  start + (((bbox.width * bbox.height) 
                    * CHANNELS) as usize);
                }

                let len = glpyh.len();
                let mut offset : isize = (end - start) as isize - len as isize;
                offset = offset.abs();

                // this is the inner check for texutres
                if !is_multiple_of(end as u128,
                        COPY_BUFFER_ALIGNMENT.into()) || 
                    !is_multiple_of(len as u128, COPY_BUFFER_ALIGNMENT.into())
                {  
                    end = end.next_multiple_of(COPY_BUFFER_ALIGNMENT as usize);

                    // set end as length aligned just update.
                    offset = (end - start) as isize - len as isize;
                    offset = offset.abs();
                    glpyh = Self::aligner_start(glpyh, offset as usize);
                }

                // if conversion OK then add to hash
                if let (Ok(start_u64), Ok(end_u64)) = (start.try_into(),
                    end.try_into()) {

                    #[cfg(debug_assertions)]
                    if start == end { panic!("start end the same!"); }
                    else if !is_multiple_of(start as u128,
                                           MAP_ALIGNMENT as u128) {
                        println!("offset {} end {} start {}", 
                                 offset, end, start);
 
                        panic!("start is misaligned");
                    }
                    else if !is_multiple_of(end as u128,
                                           COPY_BUFFER_ALIGNMENT as u128) {
                        println!("offset {} end {} start {}", 
                                 offset, end, start);
                        panic!("end is misaligned");
                    }
                    
                    // if too short pad
                    if end - start !=
                        (bbox.width * bbox.height * CHANNELS) as usize {
                        let pad : isize = (end - start) as isize - 
                            ((bbox.width * bbox.height * CHANNELS)
                                as isize);
                        let pad: usize = pad.abs() as usize; // set padding to N. 
                        glpyh = Self::aligner_start(glpyh, pad);
                        offset += pad as isize;
                        end += pad;
                    } 


                    new_pixels.append(&mut glpyh);
                    positions.insert(bbox.glpyh, ((bbox.width, bbox.height), 
                                    offset as u128, (start_u64, end_u64))); 
                }
                else{
                    #[cfg(debug_assertions)]
                    println!("glpyh {}, start {}, end {} couldn't be positioned",
                            bbox.glpyh, start, end);
                }
            }
        }

        return (new_pixels, positions);
    }

    // creates a new FontAtlas struct
    pub fn new(term_config: TermConfig, device: &mut wgpu::Device,
               queue: &mut wgpu::Queue)
        -> Self {

        let data = term_config.font_dir;
        let font_size = term_config.font_size;

        // read font from file and load data into abstraction
        let font_data = std::fs::read(data).unwrap();
        let face = fontdue::Font::from_bytes(font_data.as_slice(), 
                                     fontdue::FontSettings::default()).unwrap();

        // find raster data and bboxes
        let mut pixels : Vec<u8> = Vec::new();
        let mut bboxes = Vec::new();
        for (&glyph_c, &id) in face.chars() {
            // convert id -> u16 
            let (metrics, glyph) = 
                face.rasterize_indexed_subpixel(id.into(), font_size);
                        // use px 

            // no alpha channel so we create ours with 0 init
            let (_, mut rgba) = glyph.into_iter().fold((vec![], vec![]),
                |(mut pixel, mut state), channel | {
                if pixel.len() < 3 {
                    pixel.push(channel);
                }   
                else {
                    pixel.push(255);
                    state.append(&mut pixel);
                    pixel.clear();
                }
                return (pixel, state);
            });

            // push pixel data 
            // null char has problems with encoding
            if  !(metrics.width == 0 || metrics.height == 0 || glyph_c == '\0') {
                pixels.append(&mut rgba);
                bboxes.push(BBox { glpyh: glyph_c, 
                    width: metrics.width as u64,
                    height: metrics.height as u64 });
                

                #[cfg(debug_assertions)]
                println!("w {} h {} char {}", 
                         metrics.width, metrics.height, glyph_c)
            }
        }

        // pos_boxes is not in order  
        let (size, pos_boxes) = packer(&mut bboxes);


        // sorting for some reason drops positions
        // so we aren't going to sort here.
        pixels = Self::add_whitespace(pixels, pos_boxes.clone());

        // remove None types
        let pos_boxes = 
            pos_boxes.into_iter().
            filter(|(_, pos)| 
                   area_protect(pos.1) * 
                   area_protect(pos.0) != 0)
            .collect();

        // align
        let (mut pixels, atlas_lookup) =
            Self::record_align_to_offset(pixels, pos_boxes);


        // create atlas texutre set up as image tex
        let atlas = Self::font_atlas(&mut pixels, device, queue);
        return Self{atlas, 
            lookup : atlas_lookup, atlas_size : size}; 
    }

    // function to get glpyh data on a single char
    // returns wgpu::BufferSlice ready to be rendered as image data
    pub fn get_glpyh_data(&self, glpyh: char) -> 
        (wgpu::BufferSlice, u128) {
        // get position of char 
        let pos = self.lookup.get(&glpyh).unwrap();
        // return glpyh data as slice and offset
        #[cfg(debug_assertions)]
        println!("offsets start {} end {} width {} height {} offset {}", 
            pos.2.0, pos.2.1, pos.0.0, pos.0.1, pos.1);
        return (self.atlas.slice(pos.2.0..pos.2.1), pos.1); 
    }

}
