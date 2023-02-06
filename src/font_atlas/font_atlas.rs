use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;

use std::collections::HashMap;

pub struct FontAtlas {
    pub atlas : wgpu::Buffer,
    // point = (u64, u64) => ((w, h), offset(start, end))
    pub lookup : HashMap<char, (Point, Point)>,
    pub atlas_size : Point,
    atlas_len : usize,
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
    fn aligner_end (mut pixels : Vec<u8>, offset: u8) -> Vec<u8> {
        use num::Integer;
        if pixels.len().is_multiple_of(&(offset as usize)) {  
            return pixels;
        }
        else { 
            let len = pixels.len();
            let next = len.next_multiple_of(&(offset as usize));
            for _i in len..next {
                pixels.push(0);
            }
        }
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
                old_pixels.insert(idx as usize, 0); // insert whitespace for pixel
            }
        }

        return old_pixels;
    }

    // force pixels into alignment
    // puts in whitespace where packer has left it 
    fn align_to_offset (pixels : Vec<u8>, 
                        pos_boxes : &mut Vec<(BBox, (u64, u64))>) 
            -> Vec<u8> {

       let mut new_pixels : Vec<u8> = Vec::from([0; 8]);

        for (bbox, pos) in pos_boxes.iter_mut() {
            let start = (pos.0 * pos.1) as usize;
            let end = ((pos.0 + bbox.width) * (bbox.height + pos.1)) as usize;
            let glpyh = pixels.get(start..end).unwrap();

            // start and end of offsets recorded in positions
            // cloning so no references
            pos.0 = new_pixels.len().clone() as u64;

            // align by highest value 8 for MAP_READ | COPY_DST
            new_pixels.append(&mut Self::aligner_end(glpyh.to_vec(), 8));

            pos.1 = new_pixels.len().clone() as u64;
        }

        return new_pixels;
    }

    // creates a new FontAtlas struct
    pub fn new(data: String, font_size: f32, device: &mut wgpu::Device,
               queue: &mut wgpu::Queue)
        -> Self {

        // read font from file and load data into abstraction
        let font_data = std::fs::read(data).unwrap();
        let face = fontdue::Font::from_bytes(font_data.as_slice(), 
                                     fontdue::FontSettings::default()).unwrap();

        // calculate scale of the font
        let units_per_em = face.units_per_em();
        print!("units per em {}", units_per_em);
        let scale = units_per_em / font_size as f32;
        print!("scale {}",scale);

        // find raster data and bboxes
        let mut pixels : Vec<u8> = Vec::new();
        let mut bboxes = Vec::new();
        for (&glyph_c, &id) in face.chars() {
            // convert id -> u16 
            let (metrics, mut glyph) = 
                face.rasterize_indexed_subpixel(id.into(), scale);

            // push pixel data 
            pixels.append(&mut glyph);
            bboxes.push(BBox { glpyh: glyph_c, 
                width: metrics.width as u64,
                height: metrics.height as u64 });
        }

        // pos_boxes is not in order  
        let (size, mut pos_boxes) = packer(&mut bboxes);

        // sort by comparing two glpyh positions 
        pos_boxes.sort_by(|(bbox1,_), (bbox2,_)| 
                          face.lookup_glyph_index(bbox1.glpyh)
                          .cmp(&face.lookup_glyph_index(bbox2.glpyh)));

        pixels = Self::add_whitespace(pixels, pos_boxes.clone());
        pixels = Self::align_to_offset(pixels, &mut pos_boxes);

        let mut atlas_lookup : HashMap<char, (Point, Point)> = HashMap::new();

        for (bbox, point) in pos_boxes.clone() {
            atlas_lookup.insert(bbox.glpyh, ((bbox.width, bbox.height), 
                                             point));
        }

        // create atlas texutre set up as image tex
        let atlas = Self::font_atlas(&mut pixels, device, queue);
        return Self{atlas, 
            lookup : atlas_lookup, atlas_size : size, 
            atlas_len: pixels.len()}
    }

    // function to get glpyh data on a single char
    // returns wgpu::BufferSlice ready to be rendered as image data
    pub fn get_glpyh_data(&self, glpyh: char) -> wgpu::BufferSlice { 
        // get position of char 
        let pos = self.lookup.get(&glpyh).unwrap();
        // debug
        print!("start {} end {} full len {}", pos.1.0, pos.1.1, 
               self.atlas_len);
        // return glpyh data as slice
        return self.atlas.slice(pos.1.0 .. pos.1.1); 
    }
}

