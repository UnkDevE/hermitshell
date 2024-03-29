use crate::font_atlas::packer::packer;
use crate::font_atlas::packer::Point;
use crate::font_atlas::packer::BBox;
use crate::font_atlas::packer::area_protect;

use core::slice::SlicePattern;
use std::collections::HashMap;
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
    pub atlas : wgpu::Texture,
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
        wgpu::Texture {

        let enc = device.create_command_encoder(
            &CommandEncoderDescriptor { label: Some("font_atlas_enc") });

        let font_atlas_tex = device.create_texture(
            &TextureDescriptor { 
                label: Some("font_atlas_tex"), 
                size: Extent3d{
                    width: size.0 as u32,
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

        #[cfg(debug_assertions)]
        {
            println!("Size of atlas: ({}, {})", size.0, size.1);
        }

        for (pixel_box_data, (pixels_bbox, pos_pixels)) in pixels_boxes { 
             #[cfg(debug_assertions)]
            {
             println!("pixel_bbox size ({},{}) pos: ({},{})", 
                      pixels_bbox.width, pixels_bbox.height, pos_pixels.0, pos_pixels.1);
            }
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
                   rows_per_image: Some(pixels_bbox.height as u32) 
               },
               wgpu::Extent3d{
                    width: pixels_bbox.width as u32,
                    height: pixels_bbox.height as u32,
                    depth_or_array_layers: 1
                }
            );
        }
        // submit queue with empty command buffer to write to gpu
        use std::iter;
        queue.submit(iter::once(enc.finish()));
        device.poll(wgpu::Maintain::Wait);

        return font_atlas_tex;
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
                pixels.push((glyph_c, bgra));
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
    pub async fn get_glpyh_data(&self, glpyh: char, 
          device: &mut wgpu::Device, queue: &mut wgpu::Queue) -> wgpu::Buffer {
            // if get position of char 
            if let Some(position) = self.lookup.get(&glpyh) {
                // create buffer for loading glpyh
                let mut encoder = device.create_command_encoder(
                    &CommandEncoderDescriptor { label: Some("font_atlas_glpyh_enc") });

                let buf = device.create_buffer(&BufferDescriptor{
                    label: Some(&format!("glpyh {} buf internal", glpyh)),
                    size: position.0.1 * (4 * position.0.0).next_multiple_of(256),
                    usage: wgpu::BufferUsages::MAP_READ |
                            wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });

                encoder.copy_texture_to_buffer(
                    wgpu_types::ImageCopyTexture { 
                        texture: &self.atlas, 
                        mip_level: 0, 
                        origin: wgpu::Origin3d{
                            x: position.1.0 as u32,
                            y: position.1.1 as u32, 
                            z: 0}, 
                        aspect: wgpu_types::TextureAspect::All 
                    },
                    wgpu_types::ImageCopyBuffer { 
                        buffer: &buf, 
                        layout: wgpu_types::ImageDataLayout {
                            offset: 0, 
                            bytes_per_row: Some((position.0.0 as u32 * 4).next_multiple_of(256)),
                            rows_per_image: Some(position.0.1 as u32)
                        }
                    },
                    Extent3d { 
                        width: (position.0.0 as u32 * 4).next_multiple_of(256).div_ceil(4), 
                        height: position.0.1 as u32, 
                        depth_or_array_layers: 1 
                });
               
                // submit to queue to write buf.
                use std::iter;
                queue.submit(iter::once(encoder.finish()));
                //buf set for write, wait for completion
                device.poll(wgpu::Maintain::Wait);

                #[cfg(debug_assertions)]
                {
                    use image::Rgba;
                    let slice = buf.slice(..);

                    let (sender, reciver) = 
                            futures_intrusive::channel::shared::oneshot_channel();

                    slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());
                    device.poll(wgpu::Maintain::Wait);

                    if let Some(Ok(())) = reciver.receive().await {
                        let buf_data = slice.get_mapped_range();
                        let width = (position.0.0 as u32 * 4).next_multiple_of(256).div_ceil(4);
                        match image::ImageBuffer::<Rgba<u8>, _>
                            ::from_raw(width, position.0.1 as u32, buf_data.as_slice()) {
                                Some(image) => match 
                                    image.save(format!("glpyh_get_{}.png", &glpyh)) {
                                        Ok(()) => println!("image get save succesful"),
                                        Err(e) => println!("image get save unsuccesful , {}", e)
                                    }
                                None => {
                                    println!(
                                        "Image get buffer for glpyh {} unsuccesful", glpyh                                          )
                                }
                            }

                    }
                }

                return buf;
        }
        else{
            panic!("no position found in atlas");
        }
    }
}
             
