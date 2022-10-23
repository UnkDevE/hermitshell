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

pub type Point = (i16, i16);

#[derive(Clone)]
pub struct BBox {
    pub glpyh: char,
    pub width: i16,
    pub height: i16,
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
 *  fourth is the opossing width or height left
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
fn area_ord(area_a: i16, area_b: i16) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if area_a < area_b {
        return Ordering::Less;
    } else if area_a == area_b {
        return Ordering::Equal;
    } else {
        return Ordering::Greater;
    }
}

// sorts boxes using area
fn sort_bboxes(bboxes: &mut Vec<BBox>) {
    bboxes.sort_by(|bb_a, bb_b| {
        let area_a = bb_a.width * bb_a.height;
        let area_b = bb_b.width * bb_b.height;
        return area_ord(area_a, area_b);
    });
}

// allows for multiplication to carry through
// for calculation of area
fn mut_abs(i: i16) -> i16 {
    if i == 0 {
        return 1;
    } else {
        return i16::abs(i);
    }
}

fn search_lines(rect: BBox, xlines: &mut Vec<Line>) -> Option<Line> {
    fn lines_sort(xlines: &mut Vec<Line>, carryover: bool) {
        xlines.sort_by(|line_a, line_b| {
            if !carryover {
                let area_a = (line_a[1] - line_a[0]) * line_a[3];
                let area_b = (line_b[1] - line_b[0]) * line_b[3];
                return area_ord(area_a, area_b);
            } else {
                let area_a = (line_a[1] - line_a[0]) * mut_abs(line_a[3]);
                let area_b = (line_b[1] - line_b[0]) * mut_abs(line_b[3]);
                return area_ord(area_a, area_b);
            }
        });
    }

    // do not carry over for exact
    lines_sort(xlines, false);
    for line in xlines.clone() {
        if rect.width == (line[1] - line[0]) && rect.height <= line[3] {
            // selected the smallest line for the rectangle due to sort
            return Some(line);
        }
    }

    lines_sort(xlines, true);
    for line in xlines.clone() {
        // check if any with GT width are availble
        if rect.width * rect.height <= (line[1] - line[0]) * mut_abs(line[3]) {
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
        return area_ord(line_a[1] - line_a[1], line_b[2] - line_a[1]);
    });

    // clone longest to create new
    let mut longest_line = xlines.last().unwrap().clone();

    // set height to rects height
    longest_line[3] = rect.height;
    longest_line[4] = longest_line[4] + rect.height;

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
fn find_leftmost(placements: &mut Vec<Placement>, rect: BBox, line: Line) -> Point {
    // if line is empty
    if line[4] == -1 {
        let new_pos = (line[0] + rect.width, line[3]);
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
            .fold((line[0] + rect.width, line[3]), |mut acc, place| {
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
fn wasted_space(placements: &mut Vec<Placement>, lines: Vec<Line>) -> Vec<Placement> {
    // clone max width
    let width = (lines[0][1] - lines[0][1]).clone();

    // group all placements on the same line
    // this imperative could be changed into functional with too much effort

    let mut last_placement = placements.pop().unwrap();
    let mut place_groups: Vec<Vec<Placement>> = vec![vec![last_placement.clone()]];

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
        inv_point_place.pos = ((width - line.pos.0).abs(), line.line[3]);
        inversed_point.push(inv_point_place);
    }

    return inversed_point;
}

type Houses = Vec<Vec<Placement>>;

// find adjecent lines and create a house using empty space
fn into_houses(inv_space: &mut Vec<Placement>) -> Houses {
    let mut house_groups: Vec<Vec<Placement>> = Vec::new();

    for space in inv_space.chunks(2) {
        // setup
        let mut break_group = false;
        let mut last_group = house_groups.pop().unwrap_or(vec![]);
        let b_height = space[0].line[3] - space[1].line[2];
        let a_height = space[1].line[3] - space[0].line[2];

        // group up in old group
        let last_elem = last_group.last().unwrap().clone();
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

// gets the area of the house
fn house_size(house: Vec<Placement>) -> i16 {
    return house.into_iter().fold(0, |area, place| {
        let width = place.line[2] - place.line[1];
        let new_area = width * place.line[3];
        return area + new_area;
    });
}

// merges the smaller houses with greater ones on their borders
fn merge_houses(houses: Houses) -> Houses {
    // sort lines by area
    fn compare_houses(line_a : &Placement, line_b: &Placement) -> std::cmp::Ordering{
        use std::cmp::Ordering;
        // if height LT
        if line_a.line[2] <= line_b.line[2] && line_a.line[3] <= line_b.line[3] {
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
    let mut xlines: Vec<Line> = Vec::new();

    let min_size = get_min_size(bboxes.clone());

    xlines.push([0, min_size.0, min_size.0, 0, 1]);

    // we skip second step of algo to allow for the mod

    // third step sort boxes in decreasing order
    sort_bboxes(bboxes);
    // we reverse the list to create queue
    bboxes.reverse();

    let mut placements: Vec<Placement> = Vec::new();

    // forth step select rect
    for rect in bboxes {
        // mode 1 & 1 search, unwrap is mode 2
        let selected_line = search_lines(rect.clone(), &mut xlines).unwrap_or_else(|| {
            create_layer(rect.to_owned(), &mut xlines);
            return xlines.last_mut().unwrap().clone();
        });

        // fifth step
        // update selected line to selected rect
        find_leftmost(&mut placements, rect.to_owned(), selected_line);

        // find the waste space
        let mut inv_space = wasted_space(&mut placements, xlines);

        // get houses and group them
        let houses = into_houses(&mut inv_space);
        let grouped_houses = merge_houses(houses);

        // set the merged lines to our available lines
        xlines = grouped_houses
            .into_iter()
            .flatten()
            .map(|x| x.line)
            .collect();
    }

    // remove line data to return as unused
    return (
        (xlines[0][2], xlines.last().unwrap()[4]),
        placements
            .into_iter()
            .map(|place| ((place.bbox, place.pos)))
            .collect(),
    );
}
