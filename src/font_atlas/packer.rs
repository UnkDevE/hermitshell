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
pub type Point = (u64, u64);

#[derive(Clone)]
pub struct BBox {
    pub glpyh: char,
    pub width: u64,
    pub height: u64,
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
type Line = [i64; 5];

// gets the minumum size given the maximum bbox
// this only works diagonal so 
fn get_min_size(bboxes: Vec<BBox>) -> Point {
    let area = bboxes.clone().into_iter().fold(0, |mut acc, bbox| {
            acc += bbox.width * bbox.height;
            return acc;
    });

    #[cfg(debug_assertions)]
    {
        println!("min size {}", area.sqrt())
    }

    use num::integer::Roots;
    return (area.sqrt(), area.sqrt());
}

// higher order function for sorting
fn area_ord(area_a: u64, area_b: u64) -> std::cmp::Ordering {
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
pub fn area_protect<T>(i: T) -> T
where T : num::PrimInt + std::iter::Sum {
   if i == num::zero::<T>() {
        return num::one::<T>();
   } 
   return i;
}
pub fn area_protect_abs<T>(i: T) -> T
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
fn lines_sort(mut xlines: Vec<Line>, carryover: bool) -> Vec<Line> {
    xlines.sort_by(|line_a, line_b| {
        let mut area_a = (line_a[1] - line_a[0]).abs() as u64;
        let mut area_b = (line_b[1] - line_b[0]).abs() as u64;
        if !carryover {
            area_a *= line_a[2] as u64;
            area_b *= line_b[2] as u64;
        } else {
            area_a *= area_protect_abs(line_a[2]) as u64;
            area_b *= area_protect_abs(line_b[2]) as u64;
        }
        return area_ord(area_a, area_b);
    });

    return xlines;
}

fn search_lines(rect: BBox, xlines: &mut Vec<Line>,
                placements:&mut Vec<Placement>) -> Option<usize> {
    // do not carry over for exact
    let sort_lines = lines_sort(xlines.clone(), false);
    for (n, line) in sort_lines.iter().enumerate() {
        // calculate widths 
        let width : u64 = 
            placements.clone().into_iter().filter(|pl| pl.line_idx == n)
            .fold(xlines[n][0] as u64, |mut acc, pl| {
                acc += pl.pos.0;
                return acc;
            });

        // if full
        if width > line[2] as u64 {xlines[n][4] = -1;}
        else if line[4] != -1 { // if not
            xlines[n][0] = width as i64;
            if rect.width <= (line[1] - line[0]).abs() as u64 
                && rect.height <= area_protect_abs(line[2]) as u64 {
                // selected the smallest line for the rectangle due to sort
                return Some(n);
            }
            // check if any with GT width are availble
            if rect.width * rect.height <= 
                (line[1] - line[0]) as u64 * area_protect_abs(line[2]) as u64 {
                // selected the smallest line for the rectangle due to sort
                return Some(n);
            }
        }
    }

    // No lines that fit
    return None;
}

// makes a new line from the rectangle that does not fit
fn create_layer(rect: BBox, xlines: &mut Vec<Line>, xlines_start : Line) -> Line {
    // sort by width
    xlines.sort_by(|line_a, line_b| {
        return area_ord(area_protect_abs(line_a[2] - line_a[1]) as u64,
            area_protect_abs(line_b[2] - line_a[1]) as u64);
    });

    // this returns an Err as None, which must not happen
    let mut longest_line = xlines.last().unwrap().clone();
    if longest_line[1] < xlines_start[1] {
        longest_line[1] = xlines_start[1]
    }

    // set height to rects height
    let u_height : i64 = rect.height.try_into().unwrap();
    let u_width : i64 = rect.width.try_into().unwrap();
    longest_line[0] = 0;
    longest_line[1] += u_width;
    longest_line[2] = u_height;
    longest_line[3] += u_height;
    longest_line[4] = 1;

    #[cfg(debug_assertions)]
    {
        println!("creating new layer {} {} {} {} {}",
                 longest_line[0],
                 longest_line[1],
                 longest_line[2],
                 longest_line[3],
                 longest_line[4]);
        println!("rect size {} {}", rect.width, rect.height);
    }
    xlines.push(longest_line);
    return longest_line;
}

#[derive(Clone)]
struct Placement {
    bbox: BBox,
    pos: Point,
    line_idx: usize,
}

// straitfoward == for struct
impl PartialEq for Placement {
    fn eq(&self, other: &Self) -> bool {
        return self.bbox == other.bbox && self.pos == other.pos && self.line_idx == other.line_idx;
    }
}

// finds the leftmost position for the rectangle given
fn find_leftmost(placements: &mut Vec<Placement>, rect: BBox, 
                 xlines: &mut Vec<Line>, line_idx: usize)
    -> Option<Point> {
    let line = xlines[line_idx].clone();

    // if line full 
    if line[1] - line[0] <= 0 {
        xlines[line_idx][4] = -1;
        return None;
    }
    // doesn't fit with placement
    // search placements in line
    else if let Some(new_pos) = placements.clone()
        .into_iter()
        .filter(|place| if let Some(place) = xlines.clone().get(place.line_idx) { 
                place == &line 
            } 
            else { 
                return false;
            }).fold(Some((line[0] as u64 + rect.width, line[2] as u64)),

            |s, place| {
                if let Some(mut acc) = s {
                    // check if there is a leftmost point already stored
                    // we iterate over places here so 
                    // acc must increment over each place
                    if acc.0 < place.pos.0 {
                        acc.0 += place.pos.0;
                    }
                    return Some(acc);
                }
                else {
                    return None;
                }
            }) 
    {
        if new_pos.0 + rect.width <= line[1] as u64 {
            #[cfg(debug_assertions)]
            {
                println!("find_leftmost pos ({}, {})",
                    new_pos.0, new_pos.1);
            }
            placements.push(Placement {
                bbox: rect.clone(),
                pos: new_pos,
                line_idx
            });
            xlines[line_idx][0] = new_pos.0 as i64;
            xlines[line_idx][1] = (new_pos.0 + rect.width) as i64;
            xlines[line_idx][2] = new_pos.1 as i64;
            return Some(new_pos);
        }
    }

   // pos doesn't fit 
   return None;
}

// finds the wasted space sets them up as inversed point i.e. space leftover
fn wasted_space(placements:&mut Vec<Placement>, xlines: Vec<Line>) 
    -> Vec<Placement> {
    // clone max width
    let width = (xlines[0][2] - xlines[0][1]).clone();

    // group all placements on the same line
    // this imperative could be changed into functional with too much effort
    let mut last_placement = placements.clone().pop().unwrap();
    let mut place_groups: Vec<Vec<Placement>> = 
        vec![vec![last_placement.clone()]];

    for pl in placements.clone() {
        if pl.line_idx == last_placement.line_idx {
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
                                line.pos.0 as i32).abs() as u64,
                                xlines[line.line_idx][3] as u64);
        inversed_point.push(inv_point_place);
    }

    return inversed_point;
}

type Houses = Vec<Vec<Placement>>;

// find adjecent lines and create a house using empty space
fn empty_houses(inv_spaces: Vec<Placement>, xlines: &mut Vec<Line>) 
    -> Houses {
    let mut house_groups: Vec<Vec<Placement>> = Vec::new();

    for space in inv_spaces.windows(2) {
        let line0 = xlines[space[0].line_idx];
        let line1 = xlines[space[1].line_idx];
        // setup
        let mut break_group = false;
        let mut last_group = house_groups.pop().unwrap_or(vec![]);
        let b_height = line0[3] - line1[2];
        let a_height = line1[3] - line0[2];

        // group up in old group
        let last_elem = last_group.last().unwrap_or(
            space.last().unwrap()).clone();
        let last_line = xlines[last_elem.line_idx];
        // inverse if to deal with control flow
        if a_height != last_line[2] {
            break_group = true;
        } else {
            last_group.push(space[0].clone());
        }
        if b_height != last_line[2] {
            break_group = true;
        } else {
            last_group.push(space[1].clone());
        }

        // check if same height at base
        if a_height == line1[3] || b_height == line0[3] {
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
fn merge_houses(houses: Houses, xlines: &mut Vec<Line>) -> Houses {
    // sort lines by area
    fn compare_houses(xlines: &mut Vec<Line>, line_a : &Placement, line_b: &Placement) 
            -> std::cmp::Ordering{
        use std::cmp::Ordering;
        // if height LT
        if xlines[line_a.line_idx][2] <= xlines[line_b.line_idx][2] && 
            xlines[line_a.line_idx][3] <= xlines[line_b.line_idx][3] {
            return Ordering::Less;
        }
        return Ordering::Greater;
    }

    let mut sorted_houses : Houses = houses.into_iter().map(|house| {
        house.clone().sort_by(|line_a, line_b|
                              compare_houses(xlines, line_a, line_b));
        return house;
    }).collect();

    // remove empties:
    sorted_houses = sorted_houses.into_iter()
        .filter(|house| house.len() > 0).collect();

    // if only one house do not merge
    if sorted_houses.len() > 1 {
        // assume houses are sorted
        let mut house_groups = Vec::new();
        let mut house_b = sorted_houses.pop().unwrap();
        for mut house_a in sorted_houses {
            // get biggest line in houses
            let greatest_a = house_a.last().unwrap();
            let greatest_b = house_b.last().unwrap();

            // if adj heights then concat
            if xlines[greatest_b.line_idx][3] - xlines[greatest_b.line_idx][2] ==
                xlines[greatest_a.line_idx][3] {
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

    return sorted_houses;
}

// warning bboxes will not be in previous order
// returns size and the boxes with positions
pub fn packer(bboxes: &mut Vec<BBox>) -> (Point, Vec<(BBox, Point)>) {
    // first step init qual
    // add to line the width of all rects
    let min_size = get_min_size(bboxes.clone());
    let xlines_start: Vec<Line> = vec![[0, min_size.0 as i64, 0,
                                        min_size.1 as i64, 1]];

    let mut xlines: Vec<Line> = xlines_start.clone();

    // we skip second step of algo to allow for the mod
    let mut boxes : Vec<BBox> = bboxes.to_owned();
    // third step sort boxes in decreasing order
    sort_bboxes(&mut boxes);
    
    // we reverse the list to create queue
    boxes.reverse();

    let mut placements: Vec<Placement> = Vec::new();


    // clone boxes to use as queue
    let mut iter_boxes = boxes.clone();
    // forth step select rect
    while !iter_boxes.is_empty() {
        let rect = iter_boxes.pop().unwrap();

        // mode 1 & 2 search, unwrap is mode 3
        let selected_line = search_lines(rect.clone(), 
                                         &mut xlines, &mut placements)
            .unwrap_or_else(|| {
            create_layer(rect.to_owned(), &mut xlines, xlines_start[0]);
            return xlines.len() - 1;
        });

        // fifth step
        // update selected line to selected rect
        // if no line fit create one
        find_leftmost(&mut placements, rect.clone(), 
                      &mut xlines, selected_line).unwrap_or_else(|| {
            create_layer(rect.to_owned(), &mut xlines, xlines_start[0]);
            let selected_line = xlines.len() - 1;
            return find_leftmost(
                &mut placements, rect.to_owned(), 
                    &mut xlines, selected_line).unwrap();
        });
           
        // find the waste space
        let inv_space = wasted_space(&mut placements, xlines.clone()); 

        // if more than one house group them
        if inv_space.len() > 1 {
            // get houses and group them
            let houses = empty_houses(inv_space, &mut xlines);

            // set the merged lines to our available lines
            let grouped_houses = merge_houses(houses, &mut xlines);
            if !grouped_houses.is_empty() {
                // this might cause the error for 152
                xlines = grouped_houses.into_iter().flatten()
                    .map(|x| xlines[x.line_idx]).collect::<Vec<Line>>();
            }
        }
        else {
            // take directly from inv_space
            let mut new_xlines = xlines_start.clone();
            new_xlines.append(&mut  inv_space.into_iter().map(|x| xlines[x.line_idx])
                .collect::<Vec<Line>>());
            xlines = new_xlines;
        }
    }

    // if space len == 1 still returns
    // remove line data to return as unused
    #[cfg(debug_assertions)]
    println!("size predicted {} len {} min_size {}", 
           (xlines[0][1] as u64 * (xlines[0][3] + 
              (xlines.last().unwrap_or(&xlines[0])[1])) as u64),
           xlines.len(), min_size.0 * min_size.1);
    return (
        (xlines[0][1] as u64, 
         xlines[0][3] as u64),
            placements.into_iter()
                .map(|place| ((place.bbox, place.pos)))
                .collect()
    );
}
