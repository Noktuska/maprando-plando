use sfml::{graphics::IntRect, system::{Vector2f, Vector2i}};

/// Converts a Rect with negative width or height into a normal one
pub fn normalize_rect(rect: IntRect) -> IntRect {
    let w = rect.width;
    let h = rect.height;
    let l = if w < 0 {
        rect.left + w
    } else {
        rect.left
    };
    let t = if h < 0 {
        rect.top + h
    } else {
        rect.top
    };
    IntRect::new(l, t, w.abs(), h.abs())
}

/// Merges a set of (weakly) overlapping rects into a path that traces its perimeter clockwise
pub fn merge_rects(rects: Vec<IntRect>) -> Path {
    if rects.is_empty() {
        return Path::new();
    }

    let mut ret = Vec::new();

    let mut y_coords: Vec<_> = rects.iter().map(
        |x| x.top
    ).chain(rects.iter().map(
        |x| x.top + x.height
    )).collect();
    y_coords.sort();
    y_coords.dedup();

    let mut prev_left_coord = 0;
    let mut prev_right_coord = 0;

    for &y_coord in &y_coords {
        let min_x_left_coord = rects_at_y(y_coord, &rects).iter().map(|x| x.left).min().unwrap();
        let max_x_right_coord = rects_at_y(y_coord, &rects).iter().map(|x| x.left + x.width).max().unwrap();

        if y_coord == y_coords[0] {
            ret.push(Vector2i::new(min_x_left_coord, y_coord));
            ret.push(Vector2i::new(max_x_right_coord, y_coord));
        } else {
            if min_x_left_coord != prev_left_coord {
                ret.insert(0, Vector2i::new(prev_left_coord, y_coord));
                ret.insert(0, Vector2i::new(min_x_left_coord, y_coord));
            } else {
                ret.insert(0, Vector2i::new(min_x_left_coord, y_coord));
            }

            if max_x_right_coord != prev_right_coord {
                ret.push(Vector2i::new(prev_right_coord, y_coord));
                ret.push(Vector2i::new(max_x_right_coord, y_coord));
            } else {
                ret.push(Vector2i::new(max_x_right_coord, y_coord));
            }
        }

        prev_left_coord = min_x_left_coord;
        prev_right_coord = max_x_right_coord;
    }

    Path::from_vec(ret.into_iter().map(|vec| vec.as_other()).collect())
}

fn rects_at_y(y: i32, rects: &Vec<IntRect>) -> Vec<IntRect> {
    let res: Vec<IntRect> = rects.iter().filter(|x| x.top <= y && x.top + x.height > y).cloned().collect();

    if res.is_empty() {
        return rects.iter().filter(|x| x.top <= y && x.top + x.height == y).cloned().collect();
    }
    res
}

/// Subdivides a set of rects into subsets which are disjoint from each other (have no overlaps)
pub fn disjoin_rects(rects: Vec<IntRect>) -> Vec<Vec<IntRect>> {
    let mut res: Vec<Vec<IntRect>> = Vec::new();

    for rect in rects {
        let mut overlayed_idx = Vec::new();

        for (idx, rect_set) in res.iter().enumerate() {
            for other_rect in rect_set {
                if weak_intersect(&rect, other_rect) {
                    overlayed_idx.push(idx);
                    break;
                }
            }
        }

        if overlayed_idx.is_empty() {
            res.push(vec![rect]);
        } else if overlayed_idx.len() == 1 {
            res[overlayed_idx[0]].push(rect);
        } else {
            res[overlayed_idx[0]].push(rect);
            for i in 1..overlayed_idx.len() {
                let idx = overlayed_idx[i];
                let mut other_set = res.remove(idx);
                res[overlayed_idx[0]].append(&mut other_set);
            }
        }
    }

    res
}

/// Returns true if two rects (weakly) intersect. Weakly means an intersection with width or height 0 still counts (but not both)
fn weak_intersect(l: &IntRect, r: &IntRect) -> bool {
    let l_min_x = l.left.min(l.left + l.width);
    let l_max_x = l.left.max(l.left + l.width);
    let l_min_y = l.top.min(l.top + l.height);
    let l_max_y = l.top.max(l.top + l.height);
    let r_min_x = r.left.min(r.left + r.width);
    let r_max_x = r.left.max(r.left + r.width);
    let r_min_y = r.top.min(r.top + r.height);
    let r_max_y = r.top.max(r.top + r.height);
    
    let left = l_min_x.max(r_min_x);
    let top = l_min_y.max(r_min_y);
    let right = l_max_x.min(r_max_x);
    let bottom = l_max_y.min(r_max_y);

    (left < right && top <= bottom) || (left <= right && top < bottom)
}

pub struct Path {
    pub vertices: Vec<Vector2f>
}

impl Path {
    pub fn new() -> Self {
        Self { vertices: Vec::new() }
    }

    pub fn from_vec(vec: Vec<Vector2f>) -> Self {
        Self {
            vertices: vec
        }
    }

    pub fn len(&self) -> f32 {
        if self.vertices.len() <= 1 {
            return 0.0;
        }

        let mut res = 0.0;
        for idx in 1..self.vertices.len() {
            res += (self.vertices[idx] - self.vertices[idx - 1]).length_sq().sqrt();
        }

        res
    }

    pub fn get_point(&self, v: f32) -> Vector2f {
        if self.vertices.is_empty() {
            return Vector2f::new(0.0, 0.0);
        } else if self.vertices.len() == 1 {
            return self.vertices[0].clone();
        }

        let mut v = v.min(self.len()).max(0.0);
        for idx in 1..self.vertices.len() {
            let diff = self.vertices[idx] - self.vertices[idx - 1];
            let len = diff.length_sq().sqrt();
            if len > v {
                return self.vertices[idx - 1] + diff * v / len;
            }
            v -= len;
        }
        self.vertices.last().unwrap().clone()
    }
}