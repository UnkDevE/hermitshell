use ttf_parser as ttf;

/*
 *  using a modified algorithm of this paper
 *  https://www.researchgate.net/publication/254776781_A_Time-Efficient_and_Exploratory_Algorithm_for_the_Rectangle_Packing_Problem
 *  we modifiy it to be able to not use rotations which is quite helpful for font packing.
 *
 * we do this by modifying the front line strategy to use two lines: a height line and a width
 * line.
 *
 * the edit to the code goes as such:
 *
 * if rect w < h then
 *  Use H line
 *      if H line fits
 *          check w line else ++H line
 *  else if rect h < w
 *      if W line fits
 *          check h line
 *      else ++W line
 *  else if No fits:
 *      ++W && ++H line
 *  repeat until no rects left
 *
 *  fifth step of algorithm given is defined poorly
 *  It mentions using 'houses' which my best guess
 *  are convex rectangles of free space
 *
 */

type Point = (i16, i16);


/*
 *  first elem of line is start of it
 *  second is end of line
 *  third is distance between the base and height
 *  fourth is the opossing width or height left
 *  from the base line (line 0)
 *  fifth is wether line is occupied,
 *      1 => not occupied, -1 => fully occupied.
 */
type Line = [i16; 5];

// gets the minumum size given the maximum bbox
fn get_min_size(bboxes: Vec<ttf::Rect>) -> Point {

    return bboxes.clone().into_iter().fold((0,0), |acc, bbox| {
        acc.0 += bbox.x_max;
        acc.1 += bbox.y_max;
        return acc;
    });
}

// higher order function for sorting
fn area_ord(area_a: i16, area_b: i16)
    -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if  area_a < area_b {
        return Ordering::Less;
    }
    else if area_a == area_b {
        return Ordering::Equal;
    }
    else {
        return Ordering::Greater;
    }
}

// sorts boxes using area
fn sort_bboxes(bboxes: &mut Vec<ttf::Rect>) {
    bboxes.sort_by(|bb_a, bb_b| {
        let area_a = bb_a.x_max * bb_a.y_max;
        let area_b = bb_b.x_max * bb_b.y_max;
        return area_ord(area_a, area_b);
    });
}


// allows for multiplication to carry through
// for calculation of area
fn mut_abs(i : i16) -> i16 {
    if i == 0 {
        return 1;
    }
    else {
        return i16::abs(i);
    }
}

fn search_lines(rect : ttf::Rect, xlines : &mut Vec<Line>) -> Option<&mut Line> {

    fn lines_sort(xlines: &mut Vec<Line>, carryover: bool) {
        xlines.sort_by(| line_a, line_b | {
            let mut area_a = 0;
            let mut area_b = 0;
            if !carryover {
                area_a = (line_a[1] - line_a[0]) * line_a[3];
                area_b = (line_b[1] - line_b[0]) * line_b[3];
            }
            else {
                area_a = (line_a[1] - line_a[0]) * mut_abs(line_a[3]);
                area_b = (line_b[1] - line_b[0]) * mut_abs(line_b[3]);
            }
            return area_ord(area_a, area_b);
        });
    }


    // do not carry over for exact
    lines_sort(xlines, false);
    for line in xlines {
        if rect.x_max == (line[1] - line[0]) && rect.y_max <= line[3] {
            // selected the smallest line for the rectangle due to sort
            return Some(line);
        }
    }

    lines_sort(xlines, true);
    for line in xlines {
        // check if any with GT width are availble
        if rect.x_max * rect.y_max <= (line[1] - line[0]) * mut_abs(line[3]) {
            // selected the smallest line for the rectangle due to sort
            return Some(line);
        }
    }

    // No lines that fit
    return None;
}

// makes a new line from the rectangle that does not fit
fn create_layer(rect : ttf::Rect, height: &mut i16, xlines : Vec<Line>) -> Line {

    // sort by width
    xlines.sort_by(| line_a, line_b | {
        return area_ord(line_a[1] - line_a[1], line_b[2] - line_a[1]);
    });

    // clone longest to create new
    let mut longest_line = xlines.last().clone().unwrap();

    // set height to rects y_max
    longest_line[3] = rect.y_max;
    longest_line[4] = 1;

    return *longest_line;
}

// finds the leftmost position for the rectangle given
fn find_leftmost(placements : Vec<Placement>,
                 rect: ttf::Rect, line : &Line) -> Point {
    // if line is empty
    if line[4] == -1 {
        return (line[0] + rect.x_max, line[3]);
    }
    else {
        // search placements in line
        let new_pos = placements.into_iter().filter(| (_, _, search_line) | search_line == line)
            .fold((line[0] + rect.x_max, line[3]), |acc, (_, point, _) | {
                // check if there is a leftmost point already stored
                if acc.0 < point.0 {
                    // set start pos to that point
                    acc.0 = point.0 + rect.x_max;
                }
                return acc;
            });

        return new_pos;
    }
}


type Placement = (ttf::Rect, Point, Line);

// finds the wasted space sets them up as inversed point i.e. space leftover
fn wasted_space(placements : Vec<Placement>, lines: Vec<Line>, height: i16)
    -> Vec<Placement> {
    // clone max width
    let width = (lines[0][1] - lines[0][1]).clone();

    let empty_lines : Vec<Line>  =
        lines.into_iter().filter(| line | line[5] == -1 ).collect();

    // group all placements on the same line
    let mut place_groups : Vec<Vec<Placement>>
        = placements.into_iter().
        map(|x|
            placements.into_iter().filter(| (_, _, line) | *line == x.2).collect()
        ).collect();

    // sort by position starting at LH
    // then add largest to vec
    let mut largest_lines : Vec<Placement> = Vec::new();

    for group in place_groups {
        group.sort_by(| pla_a, pla_b | {
            use std::cmp::Ordering;
            //ignore y coordinate due to algo
            if pla_a.1.0 < pla_b.1.0 {
                return Ordering::Less;
            }
            else {
                return Ordering::Greater;
            }
        });

        // clone to stop borrow
        largest_lines.push(group.last().unwrap().clone());
    }

    // find the RH then invert to find empty space
    // here we need to know wxh
    let mut inversed_point : Vec<Placement> = Vec::new();

    for line in largest_lines {
        // clone so no data is lost in move
        let mut inv_point_place = line.clone();

        // use abs to remove overflow
        // line1.0 is the x coord of the placement
        inv_point_place.1 = ((width - line.1.0).abs(), line.2[3]);
        inversed_point.push(inv_point_place);
    }

    return inversed_point;
}

type Houses = Vec::<Vec<Placement>>;

// find adjecent lines and create a house using empty space
fn into_houses(inv_space : &mut Vec<Placement>) -> Houses {

    let house_groups : Vec<Vec<Placement>> = Vec::new();

    for [space_a, space_b] in inv_space.chunks(1) {

        // setup
        let mut break_group = false;
        let mut last_group = house_groups.pop().unwrap_or(vec!());
        let b_height =  space_b.2[3] - space_b.2[2];
        let a_height =  space_a.2[3] - space_a.2[2];

        // group up in old group
        let last_elem = last_group.last().unwrap();
        // inverse if to deal with control flow
        if a_height != last_elem.2[2] {break_group = true;}
        else {last_group.push(*space_a);}
        if b_height != last_elem.2[2] {break_group = true;}
        else {last_group.push(*space_b);}


        // check if same height at base
       if a_height == space_b.2[3] || b_height == space_a.2[3] {
             //group up
            if last_group.is_empty() {
                last_group.push(*space_a);
                last_group.push(*space_b);
            }

        }

        house_groups.push(last_group);
        if break_group {
            // create new vec to group
            house_groups.push(vec!());
        }
    }

    // cleanup
    if house_groups.last().unwrap().is_empty(){
        house_groups.pop();
    }
    return house_groups;
}

// gets the area of the house
fn house_size(house: Vec<Placement>) -> i16 {
    return house.into_iter().fold(0, | area, place | {
        let width = place.2[2] - place.2[1];
        let new_area = width * place.2[3];
        return area + new_area;
    });
}

// merges the smaller houses with greater ones on their borders
fn merge_houses(houses: Houses) -> Houses {
    // sort lines by area
    use std::cmp::Ordering;
    houses.iter_mut().map(| house | {
        house.sort_by(| line_a, line_b | {
            // if height LT
            if line_a.2[2] <= line_b.2[2]
                && line_a.2[3] <=  line_b.2[3] {
                    return Ordering::Less;
            }
            else { return Ordering::Greater; }
        });
    });

    // assume houses are sorted
    houses.iter_mut().scan(Vec::new(), |house_groups, house_a| {
        let house_b = house_groups.pop().unwrap_or(house_a);

        // get biggest line in houses
        let greatest_a = house_a.last().unwrap();
        let greatest_b = house_b.last().unwrap();

        // if adj heights then concat
        if greatest_b.2[3] - greatest_b.2[2] == greatest_b.2[3] {
            house_a.append(house_b);
            house_groups.push(house_a);
        }
        else {
            house_groups.push(house_a)
        }

        // cleanup
        house_groups.dedup();
        return Some(house_groups);
    });


    return houses;
}

fn packer(bboxes: &mut Vec<ttf::Rect>) -> Vec<(ttf::Rect, Point)> {

    // first step init qual
    // add to line the width of all rects
    let mut xlines : Vec<Line> = Vec::new();

    let min_size = get_min_size(*bboxes);
    let mut height = xlines[0][2];

    xlines.push([0, min_size.0, min_size.0, 0, 1]);

    // set inital xy to zero
    let packed_size : Point = (0,0);

    // we skip second step of algo to allow for the mod
    // third step sort boxes in decreasing order
    sort_bboxes(bboxes);
    // we reverse the list to create queue
    bboxes.reverse();

    let mut placements : Vec<Placement> = Vec::new();


    // forth step select rect
    for rect in bboxes {

        // mode 1 & 1 search, unwrap is mode 2
        let selected_line = search_lines(*rect, &mut xlines)
            .unwrap_or_else(|| {
                xlines.push(create_layer(*rect, &mut height, xlines.clone()));
                return xlines.last_mut().unwrap();
            });

        // fifth step
        // update selected line to selected rect
        placements.push((*rect, find_leftmost(placements, *rect, selected_line), *selected_line));

        // find the waste space
        let mut inv_space = wasted_space(placements, xlines, height);

        // get houses and group them
        let houses = into_houses(&mut inv_space);
        let grouped_houses = merge_houses(houses);

        // set the merged lines to our available lines
        xlines = grouped_houses.into_iter().flatten().map(|x| x.2).collect();
    }

    // remove line data to return as unused
    return placements.into_iter().map(|(a, b, _)| (a, b)).collect();
}

struct GlpyhRaster (String);

// impl glpyh builder
// openGL code goes here
impl ttf_parser::OutlineBuilder for GlpyhRaster {
    fn move_to(&mut self, x: f32, y: f32) {
        write!(&mut self.0, "M {} {} ", x, y).unwrap();
    }

    fn line_to(&mut self, x: f32, y: f32) {
        write!(&mut self.0, "L {} {} ", x, y).unwrap();
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        write!(&mut self.0, "Q {} {} {} {} ", x1, y1, x, y).unwrap();
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        write!(&mut self.0, "C {} {} {} {} {} {} ", x1, y1, x2, y2, x, y).unwrap();
    }

    fn close(&mut self) {
        write!(&mut self.0, "Z ").unwrap();
    }
}


pub fn read_font(data: String, font_size: f64)
    -> Result<(), Box<dyn std::error::Error>> {
    // read font from file and load data into abstraction
    let font_data = std::fs::read(data)?;
    let face = ttf::Face::from_slice(&font_data, 0)?;

    // calculate scale of the font
    let units_per_em = face.units_per_em();
    let scale = font_size / units_per_em as f64;

    // find glpyhs and bboxes
    let mut glyphs = Vec::new();
    let mut bboxes = Vec::new();
    let mut builder = GlpyhRaster(String::new());
    for id in 0..face.number_of_glyphs() {
        let glyph = face.outline_glyph(ttf::GlyphId(id), &mut builder);
        // get glpyh and bbox
        glyphs.push(glyph.unwrap());
        bboxes.push(face.glyph_bounding_box(ttf::GlyphId(id)).unwrap());
    }

    // get posistions for bboxes
    let placements = packer(&mut bboxes);


    todo!();

}
