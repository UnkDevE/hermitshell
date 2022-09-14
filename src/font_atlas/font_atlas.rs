mod packer;

pub fn raster_texture(pixels : Vec<u8>) -> () {
    let glpyh_img = image::load_from_memory(pixels.as_slice()).unwrap();

    // using from wgpu tutorial code
    use image::GenericImageView;
    let dim = glpyh_img.dimensions();
    let text_size = wgpu::Extent3d {
        width: dim.0,
        height: dim.1,
        depth_or_array_layers: 1
    };


}

// creates the font texture atlas given a vector of rasterized glpyhs
// and positions of where those glpyhs are
fn font_atlas(glyphs_with_bbox: Vec<(Vec<u8>, BBox)>) -> () {

    // gpu boilerplate
    let instance = wgpu::Instance::new(wgpu::Backends::all());


    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
        })
        .await
        .unwrap();
    let (device, queue) = adapter
        .request_device(&Default::default(), None)
        .await
        .unwrap();

    let atlas = wgpu::TextureDescriptor {
        size: wgpu::Extent3d{

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


    device.create_texture();
    todo!();


}

pub fn read_font(data: String, font_size: f32)
    -> () {


    // read font from file and load data into abstraction
    let font_data = std::fs::read(data).unwrap();
    let face = fontdue::Font::from_bytes(font_data.as_slice(), fontdue::FontSettings::default()).unwrap();

    // calculate scale of the font
    let units_per_em = face.units_per_em();
    let scale = font_size / units_per_em as f32;

    // invert the char hashmap of font
    let inv_hash = || -> Vec<(u16, char)> {
        let mut inv_vec = Vec::new();
        face.chars().clone().drain().map(|x| {
            inv_vec.push((x.1.into(), x.0))
        });

        return inv_vec;
    }();

    // find id code
    let find_char = | id  : u16 | -> char{
        for hash in inv_hash.clone(){
            if id == hash.0 {
                return hash.1;
            }
        }
        return '\0';
    };

    // find glpyhs and bboxes
    let mut glyphs = Vec::new();
    let mut bboxes = Vec::new();
    for id in 0..face.glyph_count() {
        let (metrics, glyph) = face.rasterize_indexed_subpixel(id, scale);
        // get glpyh and bbox
        glyphs.push(glyph);
        // push glpyh id to reverse into hashmap
        bboxes.push(BBox { glpyh: find_char(id),
            width: metrics.width as i16,
            height: metrics.height as i16});
    }

    let pos_boxes = packer(bboxes);
}
