use rand::RngExt;

pub const W: usize = 60;
pub const H: usize = 22;

#[derive(Clone, Copy, PartialEq)]
pub enum Tile {
    Wall,
    Floor,
}

pub struct Map {
    pub tiles: Vec<Tile>,
    pub visible: Vec<bool>,  // 当前视野内
    pub explored: Vec<bool>, // 曾经看过（实现地图记忆）
    pub rooms: Vec<(usize, usize, usize, usize)>, // (x, y, w, h)
}

impl Map {
    pub fn new() -> Self {
        Map {
            tiles: vec![Tile::Wall; W * H],
            visible: vec![false; W * H],
            explored: vec![false; W * H],
            rooms: Vec::new(),
        }
    }

    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * W + x
    }

    pub fn get(&self, x: usize, y: usize) -> Tile {
        self.tiles[self.idx(x, y)]
    }

    fn set(&mut self, x: usize, y: usize, t: Tile) {
        let i = self.idx(x, y);
        self.tiles[i] = t;
    }

    pub fn is_walkable(&self, x: usize, y: usize) -> bool {
        x < W && y < H && self.get(x, y) == Tile::Floor
    }

    fn carve_room(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, Tile::Floor);
            }
        }
    }

    fn carve_h_corridor(&mut self, x1: usize, x2: usize, y: usize) {
        let (a, b) = (x1.min(x2), x1.max(x2));
        for xx in a..=b {
            self.set(xx, y, Tile::Floor);
        }
    }

    fn carve_v_corridor(&mut self, y1: usize, y2: usize, x: usize) {
        let (a, b) = (y1.min(y2), y1.max(y2));
        for yy in a..=b {
            self.set(x, yy, Tile::Floor);
        }
    }

    /// 简单的地牢：网格划分 + 随机房间大小，再用 L 形走道连通相邻房间。
    pub fn generate(&mut self, _seed: Option<u64>) {
        let mut rng = rand::rng();

        let (gw, gh) = (3usize, 3usize); // 房间网格 3x3
        let cell_w = W / gw;
        let cell_h = H / gh;
        let mut centers: Vec<(usize, usize)> = Vec::new();

        for gy in 0..gh {
            for gx in 0..gw {
                let x = gx * cell_w + rng.random_range(1..=cell_w - 3);
                let y = gy * cell_h + rng.random_range(1..=cell_h - 3);
                let w = rng.random_range(4..=7);
                let h = rng.random_range(3..=5);
                // 保证房间不出界
                let x = x.min(W - w - 1);
                let y = y.min(H - h - 1);
                self.carve_room(x, y, w, h);
                centers.push((x + w / 2, y + h / 2));
                self.rooms.push((x, y, w, h));
            }
        }

        // 按行、按列把相邻房间用 L 形走道连起来
        for i in 1..centers.len() {
            let (px, py) = centers[i - 1];
            let (cx, cy) = centers[i];
            // 同属于相邻格子才连（i 和 i-1 相邻）
            if rng.random_range(0..2) == 0 {
                self.carve_h_corridor(px, cx, py);
                self.carve_v_corridor(py, cy, cx);
            } else {
                self.carve_v_corridor(py, cy, px);
                self.carve_h_corridor(px, cx, cy);
            }
        }

        // 补齐：每个房间都有出口（简单起见：保证至少第一排房间互相连通）
        self.carve_h_corridor(
            centers[0].0,
            centers[2].0,
            centers[0].1,
        );
    }

    /// 布氏线扫描：从 (x0,y0) 到 (x1,y1) 是否视野无阻。
    pub fn has_los(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> bool {
        let (mut x0, mut y0) = (x0 as i32, y0 as i32);
        let (x1, y1) = (x1 as i32, y1 as i32);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
            if (x0 as usize) >= W || (y0 as usize) >= H {
                return false;
            }
            if self.get(x0 as usize, y0 as usize) == Tile::Wall {
                return false;
            }
        }
    }

    /// 每回合重算视野。
    pub fn recompute_vision(&mut self, px: usize, py: usize, radius: usize) {
        for v in self.visible.iter_mut() {
            *v = false;
        }
        for dy in -(radius as i32)..=radius as i32 {
            for dx in -(radius as i32)..=radius as i32 {
                let x = px as i32 + dx;
                let y = py as i32 + dy;
                if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
                    continue;
                }
                let dist2 = dx * dx + dy * dy;
                if dist2 > (radius as i32) * (radius as i32) {
                    continue;
                }
                if self.has_los(px, py, x as usize, y as usize) {
                    let i = self.idx(x as usize, y as usize);
                    self.visible[i] = true;
                    self.explored[i] = true;
                }
            }
        }
    }
}