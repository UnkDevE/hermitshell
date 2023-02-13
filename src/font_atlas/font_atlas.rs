use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;
use crate::font_atlas::packer::area_protect;


use std::collections::HashMap;
use wgpu::MAP_ALIGNMENT;
use wgpu::COPY_BUFFER_ALIGNMENT;

const CHANNELS: u64 = 4;

fn is_multiple_of(n : u128, multiple: u128) -> bool
{
    if n == 0 {
        return false;
    }
    return n&(multiple - 1) == 0;
}

fn next_multiple_of(n : u128, multiple: u128) -> u128 
{
    if n.checked_rem(multiple).unwrap_or(0) == 0 {
        return n;
    }
    let frac = n.checked_div(multiple).unwrap_or(0);
    if frac != 0 {
        return n * frac;
    }
    return n;
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
       // create texture buffer
        let atlas_buf = device.create_buffer(&wgpu::BufferDescriptor{
            size: pixels.len() as u64
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::MAP_READ,
            label: Some("atlas_buffer"),
            mapped_at_creation: false 
        });

        // write as group 
        queue.write_buffer(&atlas_buf, 0, pixels.as_slice()); 
        
        return atlas_buf;
    }

    // force pixels into alignment by adding to end
    fn aligner_start (mut pixels : Vec<u8>, offset: usize) -> Vec<u8> {
        use std::iter;
        let mut offset_vec: Vec<u8> = 
                iter::repeat(0).take(offset).collect();
        pixels.append(&mut offset_vec);
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

        for (bbox, pos) in pos_boxes.into_iter() {
            let mut start = (area_protect(pos.0) * area_protect(pos.1)) as usize;
            if start == 1 {start = 0;}
            let end = ((pos.0 + (bbox.width * bbox.height)) * CHANNELS) as usize;
            let mut glpyh = pixels.get(start..end).unwrap().to_vec();

            let mut offset = 0;
            // offset of WGPU is 8 here 
            if !is_multiple_of(glpyh.len() as u128,
                MAP_ALIGNMENT as u128) {  
                let len = glpyh.len();
                let next = self::next_multiple_of(len as u128, MAP_ALIGNMENT as u128 & COPY_BUFFER_ALIGNMENT as u128);

                // make offset available to storage buffers 
                // overflow checking here
                offset = next as isize - len as isize;
                glpyh = Self::aligner_start(glpyh, offset as usize);
            }
            new_pixels.append(&mut glpyh);
            positions.insert(bbox.glpyh, ((bbox.width, bbox.height), 
                            offset as u128, pos.clone())); 
        }

        // realign pixels
        if !is_multiple_of(new_pixels.len() as u128, 
                           COPY_BUFFER_ALIGNMENT as u128) {  
            let new_start = next_multiple_of(new_pixels.len() as u128, 
                                             COPY_BUFFER_ALIGNMENT as u128);
            print!("length {} new_start {}", new_pixels.len(), new_start);
            // push need pixels to align
            for _px in (new_pixels.len() as u128)..new_start{
                // we don't need to update offsets here as we are pushing 
                // to the end of the vector.
                new_pixels.push(0);
            }
        }

        return (new_pixels, positions);
    }

    // creates a new FontAtlas struct
    pub fn new(data: String, font_size: f32, device: &mut wgpu::Device,
               queue: &mut wgpu::Queue)
        -> Self {

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
                    pixel.push(0);
                    state.append(&mut pixel);
                    pixel.clear();
                }
                return (pixel, state);
            });

            // push pixel data 
            pixels.append(&mut rgba);
            bboxes.push(BBox { glpyh: glyph_c, 
                width: metrics.width as u64,
                height: metrics.height as u64 });
        }

        // pos_boxes is not in order  
        let (size, pos_boxes) = packer(&mut bboxes);

        // sorting for some reason drops positions
        // so we aren't going to sort here.
        pixels = Self::add_whitespace(pixels, pos_boxes.clone());
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
        // start = x aligned to 8 
        let start : u64 = self::next_multiple_of(
            pos.2.0.try_into().unwrap(),
            MAP_ALIGNMENT as u128).try_into().unwrap();
        // end = ((w * h) * x + offset) * rgba channel count  
        let end = (start + (pos.0.0 * pos.0.1)) * CHANNELS;
        // start on x
        // return glpyh data as slice and offset
        return (self.atlas.slice(start..end)
                , pos.1); 
    }

}
