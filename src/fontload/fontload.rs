use ttf_parser as ttf;
// typedefs for readablility
type PackedSize = (usize, usize);

type Point = (u16, u16);

type Rect = (Point, Point);

type GlyphPoints = Vec<Point>;

/*  algorithm for packing:
1. sort rects
2. select largest rect to origin
3. select second largest rect
4. find the closest rect that completes the square
5. repeat

this would work for mostly large different sized rects,
we are working with the opposite: small similar sized ones

therefore a different packing algorithm should be used,
with alterations to our first alogrithm we get:

median or mean in this algorithm?

1. sort rects by size
2. cluster rects with same size into big rects (starting small)
3. find optimum size for clusters
    5.1 compute area of rects and add all together
    5.2 find minumum permiter by square root <- we are up to here
4. follow the algorithm in paper:
https://www.aaai.org/Papers/ICAPS/2003/ICAPS03-029.pdf
    which is as follows:
    0. prepack using greedy algorithm
        0.1 greedily place each rect with decreasing height on the left edge
            in uppermost position available
    1.treat packing as CSP:
        1.1 create binary matrix for collison
        1.2 check whether UPLH corner is in center of first rect
        1.3 any other solution can be done like 1.2 by flipping rect
    2. waste space pruning
        2.1 construct two vectors for empty rows and remaining packed
        2.2 empty row vector holds how many empty row segments * length
        2.3 scan in increasing order of length taking the:
                2.3.1.1 carryover strip area
                2.3.1.2 wasted space
            2.3.2 three cases are possible
                2.3.2.1 empty row area > current strip area + carryover
                    2.3.2.2 wasted space += (carryover - length);
                            carryover = 0;
                2.3.2.2 empty row area == current strip area + carryover
                    2.3.2.2 carrover = 0;
                2.3.2.3. else => carryover -= length;
    3. entry strip dominance
        3.1 first rect UPLH corner
        3.2 init vector with minumum heigth allowed with rect of given
            width (as square no two vectors are needed)
        3.3. consider rect with w and assume empty region above
        3.4. consider all rects with height less than empty region
        3.5. if no rects are avaible *do not* allow the considered rect be in
            that place
    4. search space rects
        4.1 find largest w and h
        4.2 search list of rects while decreasing w and increasing h by a unit
        4.3 if area candidate < total area -> increase size by 1 unit
        4.4 if area candidate > total area -> skip + decrease w by unit
    5. Minmizing to one dimension
        5.1 repeat until rectangles no longer fit then backtrack one step
*/

fn bb_to_rect(bb : ttf::Rect) -> self::Point{
   return (bb.width() as u16, bb.height() as u16);
}

type Cluster = Vec<(u16, Point)>;

fn find_clusters(boxes: Vec<Point>) -> Cluster {

    //group equal to boxes into vecs
    let mut group : Cluster = Vec::new();

    // loop through each rect
    for b in boxes {
        // if exists
        let pos =  group.iter_mut().position(|v| v.1 == b);
        if pos != None {
            // acclumlate cluster iter
            group[pos.unwrap()].0 += 1;
        }
        else {
            // else create new cluster
            group.push((1, b));
        }
    }

    return group;
}

fn area_clusters (cluster : Cluster) -> Vec<u16> {
    let mut areas = Vec::new();

    for c in cluster{
        // calc area of clusters
        areas.push(c.0 * c.1.0 * c.1.1);
    }

    return areas;
}

use nalgebra::DMatrix;

// packs and extends matrix
fn pack_sq(size: u16, start: (u16, u16), bb_test : &mut DMatrix<u8>)
    -> Option<(u16, (u16, u16))> {

    // check if collision if so return fail
    if bb_test[(start.0 as usize, start.1 as usize)] != 0 {
        return None;
    }
    else{
        let len = (bb_test.row(0).len().clone(),
                        bb_test.column(0).len().clone());
        // expand bb_test if size is too small
        let collen = len.1 as i32 - start.1 as i32;
        let rowlen = len.0 as i32 - start.0 as i32;

        if rowlen <= 0 || collen <= 0 {
            bb_test.resize_mut(rowlen.abs() as usize + len.0,
                              collen.abs() as usize + len.1, 0);
        }

        // fill square in with 1s in collision bb_test
        bb_test.slice_mut((start.0 as usize, start.1 as usize),
            (size as usize, size as usize)).fill(1);

        return Some((size, start));
    }
}

// estimates whether there is space left
fn entry_dominance(size : u16, cluster_sizes : &Vec<u16>) -> bool {
    // get the sizes smaller than selected size
    let smaller_sizes : Vec<u16> =
        cluster_sizes.clone().into_iter().filter(|&x| x <= size).collect();

    // square so l space is same as h space
    let sum_space : u16 = smaller_sizes.clone().into_iter().sum();

    // return true if all items of empty spaces are GT 0
    return smaller_sizes.into_iter().all(|x| x as i32 - sum_space as i32 >= 0);
}

fn pos_finder(size: u16, bb_test: &mut DMatrix<u8>, cluster_sizes:&mut Vec<u16>)
    -> Option<(u16, u16)> {
    let mut pos = (0, 0);
    loop {
        if entry_dominance(size, &cluster_sizes)  {
            // try packing square
            match pack_sq(size, pos, bb_test) {
                Some((size, _)) => {
                    // worst case move to new pos
                    let newpos = (pos.0 + size, pos.1 + size);
                    if newpos.0 as usize <= bb_test.row(0).len() {
                        pos = (newpos.0, pos.1); // best case
                    }
                    else if newpos.1 as usize >= bb_test.column(0).len() {

                        pos = (pos.0, newpos.1);
                    }
                    else{
                        pos = newpos;
                    }
                }
                _ => {
                    // either finished one pack or crashed
                    return Some(pos);
                }
            }
        }
        else {
            // entry fails but not done yet go back to search_squares
            return None;
        }
    }

}

// for packing using the matricies
fn greedy_pack(bb_test : &mut DMatrix<u8>,
            cluster_sizes : &mut Vec<u16>) -> Vec<(u16, u16)> {

    // reverse sort so big to small
    cluster_sizes.reverse();
    let mut positions : Vec<(u16, u16)> = Vec::new();

    loop {
        match cluster_sizes.pop() {

            Some(size) => {
                match pos_finder(size, bb_test, cluster_sizes) {
                    Some(pos) => {
                        // push to saved positions
                        positions.push(pos);
                    }
                    _ => {
                        // packing not done return to search_sqs
                        return positions;
                    }
                }
            }
            _ => {
                break;
            }
        }
    }
    // packing done
    return positions;
}


// search in sizes for next square
fn search_squares(cluster_sizes: Vec<u16>, cluster_areas: Vec<u16>,
                  bb_test : &mut DMatrix<u8>) -> usize {

   // find largest
   let mut maximum_rect = (bb_test.row(0).len(), bb_test.column(0).len());
   let mut last_size = 0;
   let mut size_i = 0;

   // backtrack if remove waste is true
   while !need_remove_waste(cluster_sizes.clone(), bb_test) {
        let max_area = maximum_rect.0 * maximum_rect.1;
        // if current size LT max area select, increase max width
        if cluster_areas[size_i] < max_area as u16 {
            last_size = size_i;
            maximum_rect.0 += 1;
        }
        else if maximum_rect.0 >= maximum_rect.1 {
            maximum_rect.1 -= 1;
        }
        // if w < h then break
        else {
            break;
        }

        // loop over sizes
        size_i = (size_i + 1) % cluster_sizes.len();
    }

   return last_size;
}

/*    2. waste space pruning
        2.1 construct two vectors for empty rows and remaining packed
        2.2 empty row vector holds how many empty row segments * length
        2.3 scan in increasing order of length taking the
                2.3.1.1 carryover strip area
                2.3.1.2 wasted space
            2.3.2 three cases are possible
                2.3.2.1 empty row area > current strip area + carryover
                    2.3.2.2 wasted space += (carryover - length);
                            carryover = 0;
                2.3.2.2 empty row area == current strip area + carryover
                    2.3.2.2 carrover = 0;
                2.3.2.3. else => carryover -= length;

*/
// removes waste space from matrix and remaining packed
// if True then backtrack, else keep going!
fn need_remove_waste(to_be_packed_sizes : Vec<u16>, bb_test: &mut DMatrix<u8>) -> bool {

    // find empty rows
    let mut empty_pos_rows = Vec::new();
    for row in bb_test.row_iter() {
        // if full have zero, only works for LH to RH
        empty_pos_rows.push(
            row.iter().position(|&x| x == 1).unwrap_or(0)
        );
    }

    // empty strip lengths
    let mut strip_lengths : Vec<usize> = Vec::new();
    let mut carryover_strip = 0;
    for (i, row) in empty_pos_rows.clone().into_iter().enumerate() {
        if carryover_strip == 0 && row >= 1 {
            // if current strip is eq then start counter
            carryover_strip = row;
        }
        else if row >= carryover_strip {
            *strip_lengths.last_mut().unwrap() += 1;
        }
        else { // row < carryover_strip -> reset counter
            carryover_strip = 0;
        }
        // for every row push
        strip_lengths.push(strip_lengths[(i - 1)]);
    }


    // setup for loop
    let mut sorted_strips = strip_lengths.clone();
    sorted_strips.sort();

    // calculate strip areas
    let strip_areas : Vec<usize> = strip_lengths.into_iter().enumerate()
        .map(|(i, x)| x * empty_pos_rows[i]).collect();


    let enclosing_area = bb_test.row(0).len() * bb_test.column(0).len();

    // 2.3.2 to 2.3.2.3
    let mut wasted_space = 0;
    carryover_strip = 0;
    for length in sorted_strips {
        for area in strip_areas.clone() {
            if length > carryover_strip + area {
                wasted_space += carryover_strip - length;
                carryover_strip = 0;
            }
            else if length == area + carryover_strip {
                carryover_strip = 0;
            }
            else {
                carryover_strip -= length;
            }
        }
    }

    // if enclosing_area GT then backtrack
    return enclosing_area >= wasted_space + carryover_strip;
}

// packing function broken up into loop and it's function
// for readablilty functions
fn packing_loop(cluster_sizes: &mut Vec<u16>, cluster_areas: &mut Vec<u16>,
                pos: &mut Vec<(u16, Point)>, bb_test: &mut DMatrix<u8>)
    -> () {

    let mut is_packed = false;
    while !is_packed {
        if pos.len() >= cluster_sizes.len() {
            is_packed = true;
        }
        else {
            // get selected size
            let (size, _) : (Vec<u16>, Vec<Point>)
                                     = pos.clone().into_iter().unzip();

            // search and pack rects
            let searched_size =
                search_squares(cluster_sizes.clone(),
                               cluster_areas.clone(), bb_test)
                    as u16;

            // repack our sqs
            match pos_finder(searched_size as u16, bb_test, cluster_sizes)
            {
                Some(nextpos) => {
                    match size.into_iter().position(|x| x == searched_size) {
                        Some(p) => {
                            pos[p] = (searched_size, nextpos);
                            cluster_sizes.remove(p);
                        }
                        _ => {
                            pos.push((searched_size, nextpos));
                            match cluster_sizes.into_iter()
                                .position(|&mut x| x == searched_size as u16) {
                                    Some(r_size) =>  {
                                        cluster_sizes.remove(r_size);
                                    }
                                    // last size is packed
                                    _ => { is_packed = true; }
                                }
                        }
                    }

                }
                _ => continue // pos not found, therefore continue loop
            }
        }
    }
 }

// the first of the pair ClusterRects carries n_clusters, and it's glyph size
// the next carries it's total size and position
type ClusterRects = Vec<((u16, Point), (u16, Point))>;
fn packer(boxes : Vec<Point>) -> (ClusterRects, (usize, usize)) {

    // find clusters + areas -> sizes
    let clusters = find_clusters(boxes);

    let mut cluster_areas = area_clusters(clusters.clone());
    cluster_areas.sort();

    let mut cluster_sizes : Vec<u16> = cluster_areas.clone().into_iter()
        .map(|x| (x as f64).sqrt() as u16).collect();

    let largest = cluster_sizes.pop().unwrap() as usize;
    // setup for search_squares
    // ignore error in linter, no error in compile time
    let mut bb_test = DMatrix::<u8>::zeros(largest, largest);
    cluster_sizes.push(largest as u16);

    // prepack the squares to find worst given packing
    let mut pos : Vec<(u16, Point)> = cluster_sizes.clone().into_iter()
        .zip(greedy_pack(&mut bb_test, &mut cluster_sizes)).collect();

    packing_loop(&mut cluster_sizes, &mut cluster_areas, &mut pos,
                               &mut bb_test);

    // sort so there are no zipping issues
    pos.sort();

    // return finalized positions + bbox and size of rect
    return (clusters.clone().into_iter().zip(pos.into_iter()).collect(),
        (bb_test.row(0).len(), bb_test.column(0).len()));
}

// TODO add Ok() result
pub fn read_font(data: String, font_size: f64)
    -> Result<(), Box<dyn std::error::Error>> {

    // read font from file and load data into abstraction
    let font_data = std::fs::read(data)?;
    let face = ttf::Face::from_slice(&font_data, 0)?;

    // calculate scale of the font
    let units_per_em = face.units_per_em();
    let scale = font_size / units_per_em as f64;

    let mut glyphs = Vec::new();
    let mut bboxes = Vec::new();
    for id in 0..face.number_of_glyphs() {
        let glyph = face.glyph_raster_image(ttf::GlyphId(id), std::u16::MAX);
        // get glpyh and bbox
        glyphs.push(glyph.unwrap());
        bboxes.push(bb_to_rect(
                face.glyph_bounding_box(ttf::GlyphId(id)).unwrap()));
    }

    let packed_points = packer(bboxes);


    return write_packing(glyphs, packed_points.0, packed_points.1, scale);
}

use ttf::RasterGlyphImage;

// this is the part of the algorithm that modifies
// the papers algorithm for glyph clusters
fn bbox_positioner(cluster_size: (u16, u16), n_clusters: u16,
                    cluster_position: (u16, u16), total_size: u16) {
    todo!();
}


fn write_packing(glpyh_rasters : Vec<RasterGlyphImage>, clusters: ClusterRects,
                 size: PackedSize, scale: f64)
    -> Result<(), Box<dyn std::error::Error>> {

    clsuters.into_iter().map(|cluster| bbox_positioner.)

    return Ok(());

}
