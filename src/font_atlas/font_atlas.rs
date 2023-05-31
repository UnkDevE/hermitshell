use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;
use crate::font_atlas::packer::area_protect;

use std::collections::HashMap;
use num::Integer;
use wgpu::CommandEncoderDescriptor;
use wgpu::BufferDescriptor;
use wgpu::Extent3d;
use wgpu::TextureDimension;
use wgpu::TextureUsages;
use wgpu::TextureFormat;
use wgpu::TextureDescriptor;


#[derive(Clone)]
pub struct TermConfig{
    pub font_dir: String,
    pub font_size: f32
}

pub struct FontAtlas {
    pub atlas : wgpu::Buffer,
    // point = (u64, u64) => ((w, h), (x, y))
    pub lookup : HashMap<char, (Point, Point)>,
    pub atlas_size : Point,
}

impl FontAtlas {
    
    // creates the font texture atlas given a vector of rasterized glpyhs
    // and positions of where those glpyhs are using a wpu::Buffer
    // device is locked so need reference
    fn font_atlas(pixels_boxes: &mut Vec<(Vec<u8>, (BBox, (u64, u64)))>, 
                  device: &mut wgpu::Device, queue: &mut wgpu::Queue, size: (u64, u64)) -> 
        wgpu::Buffer {

        let mut enc = device.create_command_encoder(
            &CommandEncoderDescriptor { label: Some("font_atlas_enc") });

        let font_atlas_tex = device.create_texture(
            &TextureDescriptor { 
                label: Some("font_atlas_tex"), 
                size: Extent3d{
                    width: size.0  as u32,
                    height: size.1 as u32,
                    depth_or_array_layers: 1
                }, 
                mip_level_count: 1, 
                sample_count: 1, 
                dimension: TextureDimension::D2,
                format: TextureFormat::Bgra8UnormSrgb, 
                usage: TextureUsages::RENDER_ATTACHMENT |
                   TextureUsages::COPY_SRC |
                   TextureUsages::COPY_DST,
                view_formats: &[wgpu::TextureFormat::Bgra8UnormSrgb],
            }
        );

        for (pixel_box_data, (pixels_bbox, pos_pixels)) in pixels_boxes { 
             queue.write_texture(
                 wgpu::ImageCopyTextureBase {
                   texture: &font_atlas_tex,
                   mip_level: 0,
                   origin: wgpu::Origin3d {
                       x: pos_pixels.0 as u32,
                       y: pos_pixels.1 as u32,
                       z: 0
                   },
                   aspect: wgpu::TextureAspect::All
               }, 
               &pixel_box_data.as_slice(), 
               wgpu::ImageDataLayout { 
                   offset: 0, 
                   bytes_per_row: Some(pixels_bbox.width as u32 * 4), 
                   rows_per_image: None 
               },
               wgpu::Extent3d{
                    width: pixels_bbox.width as u32,
                    height: pixels_bbox.height as u32,
                    depth_or_array_layers: 1
                }
            );
        }

        let u32_size = std::mem::size_of::<u32>() as u64;
        let atlas_buf = device.create_buffer(
           &BufferDescriptor { 
                label: Some("font_atlas buffer") , 
                usage: wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: true,
                size: (size.0 * u32_size as u64).next_multiple_of(256) * size.1
            });

        enc.copy_texture_to_buffer(    
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &font_atlas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                }, 
            wgpu::ImageCopyBufferBase { 
                buffer: &atlas_buf, 
                layout: wgpu::ImageDataLayout {
                    offset: 0, 
                    bytes_per_row: Some((size.0 * u32_size).next_multiple_of(256) as u32),
                    rows_per_image: Some(size.1 as u32)
                }
            },
            Extent3d { 
                width: size.0 as u32, 
                height: size.1 as u32, 
                depth_or_array_layers:1 
            }
        );

        // unmap the buffer
        atlas_buf.unmap();

        // submit queue with empty command buffer to write to gpu
        use std::iter;
        queue.submit(iter::once(enc.finish()));
        device.poll(wgpu::Maintain::Wait);
        return atlas_buf;
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
        let mut pixels : Vec<(char, Vec<u8>)> = Vec::new();
        let mut bboxes = Vec::new();
        for (&glyph_c, &id) in face.chars() {
            // convert id -> u16 
            let (metrics, glyph) = 
                face.rasterize_indexed_subpixel(id.into(), font_size);
                        // use px 

            #[cfg(debug_assertions)]
            {
                let _ = 
                image::save_buffer_with_format(format!("pure_glpyh_{}.png", glyph_c), 
                                               &(glyph.clone()), metrics.width as u32, metrics.height as u32, 
                                               image::ColorType::Rgb8, image::ImageFormat::Png);
            }

            // no alpha channel so we create ours with 255 init
            let mut rgba = Vec::new();
            for channels in glyph.chunks(3) {
                let mut pixel = Vec::from(channels);
                pixel.push(255); // alpha
                rgba.append(&mut pixel);
            }
            #[cfg(debug_assertions)]
            {
                println!("rgba len {}", rgba.len());
                let _ = 
                image::save_buffer_with_format(format!("alpha_glpyh_{}.png", glyph_c), 
                                               &rgba.clone(), metrics.width as u32, metrics.height as u32, 
                                               image::ColorType::Rgba8, image::ImageFormat::Png);
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
                pixels.push((glyph_c, rgba));
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

        // remove None types
        let pos_boxes : Vec<(BBox, (u64, u64))> = 
            pos_boxes.into_iter().
            filter(|(_, pos)| 
                   area_protect(pos.1) * 
                   area_protect(pos.0) != 0)
            .collect();

        let mut atlas_lookup = HashMap::new(); 
        for boxes in pos_boxes.clone() {
            atlas_lookup.insert(boxes.0.glpyh,
                                ((boxes.0.width, boxes.0.height), 
                                 boxes.1));
        }

        let mut pixels_boxes : Vec<(Vec<u8>, (BBox, (u64, u64)))> = Vec::new();
        for (glpyh, data) in pixels { 
            let mut position : Vec<(BBox, (u64, u64))> =
                pos_boxes.clone().into_iter().filter(|(bbox, _pos)| bbox.glpyh == glpyh).collect(); 
            pixels_boxes.push((data, position.pop().unwrap()));            
        }

        // create atlas texutre set up as image tex
        let atlas = Self::font_atlas(&mut pixels_boxes, 
                                     device, queue, size);


        return Self{atlas, 
            lookup : atlas_lookup, atlas_size : size}; 
    }

    

    // function to get glpyh data on a single char
    // returns wgpu::BufferSlice ready to be rendered as image data
    pub fn get_glpyh_data(&self, glpyh: char) -> 
        (wgpu::BufferSlice, u128) {
        // get position of char 
        let pos = self.lookup.get(&glpyh).unwrap();

        // calc start and end
        
        // position
        let start = pos.1.0 * pos.1.1;
        let start_buf = start.prev_multiple_of(&8);
    
        // add w*h area 
        let end = start_buf + pos.0.0 * pos.0.1;
        let end_buf = end.next_multiple_of(4);

        // we have increased bytes_per_row to a multiple of 256
        // buffer alignment requires it to start at MAP_ALIGNMENT
        // and end at COPY_BUFFER_ALIGNMENT

        // return glpyh data as slice and offset
        #[cfg(debug_assertions)]
        println!("offsets {} start {} end {} width {} height {}", 
            end_buf.abs_diff(start_buf).abs_diff(end.abs_diff(start)),
                start_buf, end_buf, pos.0.0, pos.0.1);

        // we don't know if aligned but we are anyway
        // but this is best guess, not good.
        return (self.atlas.
                slice(start_buf..end_buf), (
                    end_buf.abs_diff(start_buf).abs_diff(end.abs_diff(start))
                ) as u128); 
    }
}
