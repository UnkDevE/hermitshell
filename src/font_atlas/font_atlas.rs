use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;

use std::collections::HashMap;

pub struct FontAtlas {
    pub atlas : wgpu::Buffer,
    // point = (u32, u32) => ((w, h), (x, y))
    pub lookup : HashMap<char, (Point, Point)>,
    pub atlas_size : Point,
}

impl FontAtlas {
    
    // creates the font texture atlas given a vector of rasterized glpyhs
    // and positions of where those glpyhs are using a wpu::Buffer
    async fn font_atlas(pixels: &mut Vec<u8>) -> wgpu::Buffer {
        // gpu boilerplate - create instance for use
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

        // start at 8 for map align, 
        let mut aligned = Vec::from([0 as u8; 8]);
        aligned.append(pixels);
        aligned = Self::aligner(aligned);

        // create texture buffer
        let atlas_buf = device.create_buffer(&wgpu::BufferDescriptor{
            size: aligned.len() 
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ
                | wgpu::BufferUsages::COPY_DST,
            label: Some("atlas_buffer"),
            mapped_at_creation: false 
        });

        // write as group 
        queue.write_buffer(&atlas_buf, 0, aligned.as_slice()); 
        
        return atlas_buf;
    }

    // gives if power of two and if not how much from next power 
    fn ispowertwo (n : u64) -> bool{
        return (n != 0) && (n & (!n - 1)) == n; 
    }

    // force pixels into alignment
    fn aligner (mut pixels : Vec<u8>) -> Vec<u8> {
        if Self::ispowertwo(pixels.len() as u64) {  
            return pixels;
        }
        else { 
            let len = pixels.len();
            let next = len.next_power_of_two();
            for _i in len..next {
                pixels.push(0);
            }
        }
        return pixels;
    } 

    // puts in whitespace where packer has left it 
    fn add_whitespace (pixels : &mut Vec<u8>, 
                       pos_boxes : Vec<(BBox, (u32, u32))>){

        // assuming pos_boxes are sorted and aligned with pixels
        for windows in pos_boxes.windows(2) {
            let bbox = &windows[0].0; 
            let pos = windows[0].1;
            let pixel_end = (bbox.width + pos.0) * (bbox.height * pos.1); 
            // take the position of the next element
            let end_pos = windows[1].1.0 * windows[1].1.1; 

            for idx in pixel_end..end_pos {
                print!("whitspace");
                pixels.insert(idx as usize, 0); // insert whitespace for pixel
            }
        }
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
        let scale = units_per_em / font_size;

        // find raster data and bboxes
        let mut pixels : Vec<u8> = Vec::new();
        let mut bboxes = Vec::new();
        for (&glyph_c, &id) in face.chars() {
            // convert id -> u16 
            let (metrics, mut glyph) = 
                face.rasterize_indexed_subpixel(id.into(), scale);

            // push pixel data 
            pixels.append(&mut glyph);
            // push glpyh char with bbox 
            bboxes.push(BBox { glpyh: glyph_c, 
                width: metrics.width as u32,
                height: metrics.height as u32 });
        }

        // pos_boxes is not in order  
        let (size, mut pos_boxes) = packer(&mut bboxes);

        // sort by comparing two glpyh positions 
        pos_boxes.sort_by(|(bbox1,_), (bbox2,_)| 
                          face.lookup_glyph_index(bbox1.glpyh)
                          .cmp(&face.lookup_glyph_index(bbox2.glpyh)));

        // ptr return of pixels.
        Self::add_whitespace(&mut pixels, pos_boxes.clone());

        let mut atlas_lookup : HashMap<char, (Point, Point)> = HashMap::new();

        for (bbox, point) in pos_boxes.clone() {
            atlas_lookup.insert(bbox.glpyh, ((bbox.width, bbox.height), 
                                             point));
        }

        // create atlas texutre set up as image tex
        let atlas = Self::font_atlas(&mut pixels).await;

        // flush writes and put in GPU
        // atlas.unmap();

        return Self{atlas, lookup : atlas_lookup, atlas_size : size}
    }

    // function to get glpyh data on a single char
    // returns wgpu::BufferSlice ready to be rendered as image data
    pub fn get_glpyh_data(&self, glpyh: char) -> wgpu::BufferSlice { 
        // get position of char 
        let pos = self.lookup.get(&glpyh).unwrap();
        
        print!("{:#?}", pos);
        // x,y coordinates
        let offset_start = (pos.1.0 * pos.1.1) as u64 + 8; // init offset
        
        // x,y coords plus w, h
        let offset_end = offset_start + ((pos.1.0 + pos.0.0) * (pos.1.1 + pos.0.1)) 
            as u64 + 8; // add init offset

        print!("offset_start {}, offset end {}", offset_start,
               offset_end); 
        // return glpyh data as slice
        return self.atlas.slice(offset_start..offset_end); 
    }
}

