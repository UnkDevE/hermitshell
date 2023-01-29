/*
 *  using a modified algorithm of this paper
 *  https://www.researchgate.net/publication/254776781_A_Time-Efficient_and_Exploratory_Algorithm_for_the_Rectangle_Packing_Problem
 *  we modifiy it to be able to not use rotations which is quite helpful for font packing.
 *
 *  we skip 2nd stage and take a hit to preformance to allow for 
 *  _fixed_ orenatations of rectangles when packing 
 *
 *  fifth step of algorithm given is defined poorly It mentions using 'houses' which my best guess
 *  are convex rectangles of free space
 *
 */

// ALL OF THIS IS WRONG NEEDS REWRITE ~~~~~
pub type Point = (u32, u32);

#[derive(Clone)]
pub struct BBox {
    pub glpyh: char,
    pub width: u32,
    pub height: u32,
}

impl PartialEq for BBox {
    fn eq(&self, other: &Self) -> bool {
        return self.width == other.width && self.height == other.height;
    }
}


/*
 *  first elem of line is start of it
 *  second is end of line
 *  third is distance between the base and height
 *  fourth is the height left
 *  from the base line (line 0)
 *  fifth is wether line is occupied,
 *      1 => not occupied, -1 => fully occupied.
 */
type Line = [i16; 5];

// gets the minumum size given the maximum bbox
fn get_min_size(bboxes: Vec<BBox>) -> Point {
    return bboxes.clone().into_iter().fold((0, 0), |mut acc, bbox| {
        acc.0 += bbox.width;
        acc.1 += bbox.height;
        return acc;
    });
}

// higher order function for sorting
fn area_ord(area_a: u32, area_b: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if area_a < area_b {
        return Ordering::Less;
    } else if area_a == area_b {
        return Ordering::Equal;
    } else {
        return Ordering::Greater;
    }
}

// allows for multiplication to carry through
// for calculation of area
fn area_protect<T>(i: T) -> T
where T : num::PrimInt + std::iter::Sum {
   if i == num::zero::<T>() {
        return num::one::<T>();
   } 
   return i;
}
fn area_protect_abs<T>(i: T) -> T
where T : num::Signed + std::iter::Sum {
    if i == num::zero::<T>() {
        return num::one::<T>();
   } 
   return num::abs::<T>(i);
}
// sorts boxes using area
fn sort_bboxes(bboxes: &mut Vec<BBox>) {
    bboxes.sort_by(|bb_a, bb_b| {
        // if width or height is zero change to 1
        let area_a = area_protect(bb_a.width) * area_protect(bb_a.height);
        let area_b = area_protect(bb_b.width) * area_protect(bb_b.height);
        return area_ord(area_a, area_b);
    });
}

#[cfg(test)]
fn box_maker() -> Vec<BBox> {
    use rand::random;
    let boxes = Vec::new();
    for i in 0..random() {
        boxes.push(BBox{glpyh: 'a', width: random(), height:random()});
    }
    return boxes;
}

#[cfg(test)]
fn sort_box_test(){
   let sort = box_maker();
   sort_bboxes(&mut sort);

   let sorted = sort.into_iter().fold(|acc, e| {
       if acc < e.height * e.width {
           return true;
       }
       else {
           return false;
       }
   },  true).collect();

   return sorted;
}

fn lines_sort(mut xlines: Vec<Line>, carryover: bool) -> Vec<Line> {
    xlines.sort_by(|line_a, line_b| {
        let mut area_a = (line_a[1] - line_a[0]).abs() as u32;
        let mut area_b = (line_b[1] - line_b[0]).abs() as u32;
        if !carryover {
            area_a *= line_a[2] as u32;
            area_b *= line_b[2] as u32;
        } else {
            area_a *= area_protect_abs(line_a[2]) as u32;
            area_b *= area_protect_abs(line_b[2]) as u32;
        }
        return area_ord(area_a, area_b);
    });

    return xlines;
}

fn search_lines(rect: BBox, xlines: Vec<Line>) -> Option<Line> {
    // do not carry over for exact
    let sort_lines = lines_sort(xlines.clone(), false);
    for line in sort_lines {
        if rect.width == (line[1] - line[0]).abs() as u32 
            && rect.height <= area_protect_abs(line[2]) as u32 {
            // selected the smallest line for the rectangle due to sort
            return Some(line);
        }
        // check if any with GT width are availble
        if rect.width * rect.height <= 
            ((line[1] - line[0]) * area_protect_abs(line[2])) as u32 {
            // selected the smallest line for the rectangle due to sort
            return Some(line);
        }
    }

    // No lines that fit
    return None;
}

// makes a new line from the rectangle that does not fit
fn create_layer(rect: BBox, xlines: &mut Vec<Line>) -> Line {
    // sort by width
    xlines.sort_by(|line_a, line_b| {
        return area_ord(area_protect_abs(line_a[2] - line_a[1]) as u32,
            area_protect_abs(line_b[2] - line_a[1]) as u32);
    });

    // clone longest to create new
    let mut longest_line = xlines.last().unwrap().clone();

    // set height to rects height
    let u_height = rect.height.try_into().unwrap();
    longest_line[2] = u_height;
    longest_line[3] = longest_line[3] + u_height;

    xlines.push(longest_line);
    return longest_line;
}

#[derive(Clone)]
struct Placement {
    bbox: BBox,
    pos: Point,
    line: Line,
}

// straitfoward == for struct
impl PartialEq for Placement {
    fn eq(&self, other: &Self) -> bool {
        return self.bbox == other.bbox && self.pos == other.pos && self.line == other.line;
    }
}

// finds the leftmost position for the rectangle given
fn find_leftmost(placements: &mut Vec<Placement>, rect: BBox, line: Line)
    -> Point {
    // if line is empty
    if line[4] == 1 {
        let new_pos = (line[0] as u32 + rect.width, line[3] as u32);
        placements.push(Placement {
            bbox: rect,
            pos: new_pos,
            line,
        });
        return new_pos;
    } else {
        // search placements in line
        let new_pos = placements
            .into_iter()
            .filter(|place| place.line == line)
            .fold((line[0] as u32 + rect.width, line[3] as u32), |mut acc, place| {
                // check if there is a leftmost point already stored
                if acc.0 < place.pos.0 {
                    // set start pos to that point
                    acc.0 = place.pos.0 + rect.width;
                }
                return acc;
            });

        placements.push(Placement {
            bbox: rect,
            pos: new_pos,
            line,
        });
        return new_pos;
    }
}

// finds the wasted space sets them up as inversed point i.e. space leftover
fn wasted_space(mut placements: Vec<Placement>, lines: Vec<Line>) 
    -> Vec<Placement> {
    // clone max width
    let width = (lines[0][2] - lines[0][1]).clone();

    // group all placements on the same line
    // this imperative could be changed into functional with too much effort
    let mut last_placement = placements.pop().unwrap();
    let mut place_groups: Vec<Vec<Placement>> = 
        vec![vec![last_placement.clone()]];

    for pl in placements {
        if pl.line == last_placement.line {
            last_placement = pl.clone();
            place_groups.last_mut().and_then(|group| {
                return Some(group.push(pl.to_owned()));
            }).unwrap_or_else(|| {
                return place_groups.push(vec![pl.to_owned()]);
            });
        }
        else {
            // push new empty group to break up groups 
            place_groups.push(vec![]);
        }
    }
    // cleanup for place_groups
    place_groups.retain(|x| {x.len() > 0}); 

    // then add largest to vec
    let mut largest_lines: Vec<Placement> = Vec::new();

    for mut group in place_groups {
        group.sort_by(|pla_a, pla_b| {
            use std::cmp::Ordering;
            //ignore y coordinate due to algo
            if pla_a.pos.0 < pla_b.pos.0 {
                return Ordering::Less;
            } else {
                return Ordering::Greater;
            }
        });

        // clone to stop borrow
        largest_lines.push(group.last().unwrap().clone());
    }

    // find the RH then invert to find empty space
    // here we need to know wxh
    let mut inversed_point: Vec<Placement> = Vec::new();

    for line in largest_lines {
        // clone so no data is lost in move
        let mut inv_point_place = line.clone();

        // use abs to remove overflow
        // line1.0 is the x coord of the placement
        inv_point_place.pos = ((width as i32 - 
                                line.pos.0 as i32).abs() as u32,
                                line.line[3] as u32);
        inversed_point.push(inv_point_place);
    }

    return inversed_point;
}

type Houses = Vec<Vec<Placement>>;

// find adjecent lines and create a house using empty space
fn empty_houses(inv_spaces: Vec<Placement>) 
    -> Houses {
    let mut house_groups: Vec<Vec<Placement>> = Vec::new();

    for space in inv_spaces.windows(2) {
        // setup
        let mut break_group = false;
        let mut last_group = house_groups.pop().unwrap_or(vec![]);
        let b_height = space[0].line[3] - space[1].line[2];
        let a_height = space[1].line[3] - space[0].line[2];

        // group up in old group
        let last_elem = last_group.last().unwrap_or(
            space.last().unwrap()).clone();
        // inverse if to deal with control flow
        if a_height != last_elem.line[2] {
            break_group = true;
        } else {
            last_group.push(space[0].clone());
        }
        if b_height != last_elem.line[2] {
            break_group = true;
        } else {
            last_group.push(space[1].clone());
        }

        // check if same height at base
        if a_height == space[1].line[3] || b_height == space[0].line[3] {
            //group up
            if last_group.is_empty() {
                last_group.push(space[0].clone());
                last_group.push(space[1].clone());
            }
        }

        house_groups.push(last_group);
        if break_group {
            // create new vec to group
            house_groups.push(vec![]);
        }
    }

    // cleanup
    if house_groups.last().unwrap().is_empty() {
        house_groups.pop();
    }
    return house_groups;
}

// merges the smaller houses with greater ones on their borders
fn merge_houses(houses: Houses) -> Houses {
    // sort lines by area
    fn compare_houses(line_a : &Placement, line_b: &Placement) 
            -> std::cmp::Ordering{
        use std::cmp::Ordering;
        // if height LT
        if line_a.line[2] <= line_b.line[2] && 
            line_a.line[3] <= line_b.line[3] {
            return Ordering::Less;
        }
        return Ordering::Greater;
    }

    let mut sorted_houses : Houses = houses.into_iter().map(|house| {
        house.clone().sort_by(compare_houses);
        return house;
    }).collect();

    // assume houses are sorted
    let mut house_groups = Vec::new();
    let mut house_b = sorted_houses.pop().unwrap();
    for mut house_a in sorted_houses {
        // get biggest line in houses
        let greatest_a = house_a.last().unwrap();
        let greatest_b = house_b.last().unwrap();

        // if adj heights then concat
        if greatest_b.line[3] - greatest_b.line[2] == greatest_a.line[3] {
            house_a.append(&mut house_b);
            house_groups.push(house_a.clone());
        } else {
            house_groups.push(house_a.clone())
        }

        // cleanup
        house_groups.dedup();
    }

    return house_groups;
}

// warning bboxes will not be in previous order
// returns size and the boxes with positions
pub fn packer(bboxes: &mut Vec<BBox>) -> (Point, Vec<(BBox, Point)>) {
    // first step init qual
    // add to line the width of all rects
    let min_size = get_min_size(bboxes.clone());
    let mut xlines: Vec<Line> = vec![[0, min_size.0 as i16, 0, 0, 1]];

    // we skip second step of algo to allow for the mod

    // third step sort boxes in decreasing order
    sort_bboxes(bboxes);

    // we reverse the list to create queue
    bboxes.reverse();

    let mut placements: Vec<Placement> = Vec::new();

    // forth step select rect
    for rect in bboxes {
        // mode 1 & 2 search, unwrap is mode 3
        let selected_line = search_lines(rect.clone(), xlines.clone())
            .unwrap_or_else(|| {
            return create_layer(rect.to_owned(), &mut xlines);
        });

        // fifth step
        // update selected line to selected rect
        find_leftmost(&mut placements, rect.to_owned(), selected_line);

        // find the waste space
        let inv_space = wasted_space(placements.clone(), xlines.clone()); 

        // if more than one house group them
        if inv_space.len() > 1 {
            // get houses and group them
            let houses = empty_houses(inv_space);

            // set the merged lines to our available lines
            let grouped_houses = merge_houses(houses);
            xlines.append(&mut grouped_houses.into_iter().flatten()
                .map(|x| x.line).collect::<Vec<Line>>());
        }
        else {
            // take directly from inv_space
            xlines.append(&mut inv_space.into_iter().map(|x| x.line)
                .collect::<Vec<Line>>());
        }
    }

    // if space len == 1 still returns
    // remove line data to return as unused
    print!("size predicted {} len {}", 
           xlines[0][1] as u32 * xlines.last().unwrap()[3] as u32,
           xlines.len());
    return (
           (xlines[0][1] as u32, xlines.last().unwrap()[3] as u32),
            placements.into_iter()
                .map(|place| ((place.bbox, place.pos)))
                .collect()
    );
}
