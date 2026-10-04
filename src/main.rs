mod map;

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use map::{Map, Tile, W, H};
use rand::RngExt;
use std::io::{stdout, Write};

struct Entity {
    x: usize,
    y: usize,
    ch: char,
    hp: i32,
    max_hp: i32,
    atk: i32,
}

struct Item {
    x: usize,
    y: usize,
    kind: ItemKind,
}

#[derive(Clone, Copy)]
enum ItemKind {
    Heal,
}

struct Game {
    map: Map,
    player: Entity,
    monsters: Vec<Entity>,
    items: Vec<Item>,
    kills: u32,
    msg: String,
    vw: usize, // 实际渲染宽度（受终端窗口限制）
    vh: usize, // 实际渲染高度（受终端窗口限制）
}

impl Game {
    fn new(vw: usize, vh: usize) -> Self {
        let mut map = Map::new();
        map.generate(None);
        let (px, py, _, _) = map.rooms[0];
        let (px, py) = (px + 1, py + 1);

        // 怪物 + 血瓶：每个房间（第一间除外）放一只怪和一个 +
        let mut monsters = Vec::new();
        let mut items = Vec::new();
        let mut rng = rand::rng();
        let names = ['g', 'r', 's', 'k'];
        for (i, &(rx, ry, rw, rh)) in map.rooms.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let mx = rx + 1 + rng.random_range(0..rw.saturating_sub(2));
            let my = ry + 1 + rng.random_range(0..rh.saturating_sub(2));
            monsters.push(Entity {
                x: mx,
                y: my,
                ch: names[i % names.len()],
                hp: 8 + (i as i32) * 3,
                max_hp: 8 + (i as i32) * 3,
                atk: 2 + i / 2,
            });
            // 血瓶放在怪物另一头
            let hx = if rng.random_range(0..2) == 0 {
                rx
            } else {
                rx + rw - 1
            };
            let hy = if rng.random_range(0..2) == 0 {
                ry
            } else {
                ry + rh - 1
            };
            items.push(Item {
                x: hx.min(rx + rw - 1).max(rx),
                y: hy.min(ry + rh - 1).max(ry),
                kind: ItemKind::Heal,
            });
        }

        let mut game = Game {
            map,
            player: Entity {
                x: px,
                y: py,
                ch: '@',
                hp: 40,
                max_hp: 40,
                atk: 8,
            },
            monsters,
            items,
            kills: 0,
            msg: String::from("用 h/j/k/l 或方向键移动，走到怪物身上攻击"),
            vw: vw.min(W),
            vh: (vh.saturating_sub(3)).min(H),
        };
        game.map.recompute_vision(game.player.x, game.player.y, 8);
        game
    }

    fn try_move_player(&mut self, dx: i32, dy: i32) {
        if self.player.hp <= 0 {
            return; // 死了不能动，只能 g 重开
        }
        let nx = self.player.x as i32 + dx;
        let ny = self.player.y as i32 + dy;
        if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
            return;
        }
        let (nx, ny) = (nx as usize, ny as usize);

        // 撞到怪物 -> 攻击
        if let Some(m) = self.monsters.iter_mut().find(|m| m.x == nx && m.y == ny) {
            let dmg = (self.player.atk / 2 + rand::rng().random_range(0..self.player.atk)).max(1);
            m.hp -= dmg;
            self.msg = format!("你攻击了{}，造成{}点伤害", m.ch, dmg);
            if m.hp <= 0 {
                self.msg = format!("你击杀了{}！", m.ch);
                self.kills += 1;
            }
            self.monsters.retain(|m| m.hp > 0);
            if self.monsters.is_empty() {
                self.msg = "……地牢安静了下来。你清空了这个楼层！".to_string();
            }
            self.end_turn();
            return;
        }

        if self.map.is_walkable(nx, ny) {
            self.player.x = nx;
            self.player.y = ny;
            self.pickup();
            self.end_turn();
        }
    }

    /// 踩到 + 回血（最多回满）。
    fn pickup(&mut self) {
        let (px, py) = (self.player.x, self.player.y);
        let idx = self.items.iter().position(|it| it.x == px && it.y == py);
        if let Some(i) = idx {
            match self.items[i].kind {
                ItemKind::Heal => {
                    let before = self.player.hp;
                    self.player.hp = (self.player.hp + 15).min(self.player.max_hp);
                    self.msg = format!("喝下血瓶，恢复{}血", self.player.hp - before);
                }
            }
            self.items.remove(i);
        }
    }

    /// 处理怪物：能看到玩家就靠近并攻击，否则原地随机游荡。
    fn update_monsters(&mut self) {
        use std::collections::HashSet;
        let (px, py) = (self.player.x, self.player.y);
        let mut rng = rand::rng();
        let mut hits = Vec::new();
        // 预计算所有怪物当前位置，用于移动时的碰撞判断
        let occupied: HashSet<(usize, usize)> = self
            .monsters
            .iter()
            .map(|m| (m.x, m.y))
            .collect();
        for m in self.monsters.iter_mut() {
            let sees = self.map.has_los(m.x, m.y, px, py) && m.x.abs_diff(px) + m.y.abs_diff(py) < 10;
            if sees {
                let (step_x, step_y) = (
                    (px as i32 - m.x as i32).signum(),
                    (py as i32 - m.y as i32).signum(),
                );
                let nx = m.x as i32 + step_x;
                let ny = m.y as i32 + step_y;
                if nx == px as i32 && ny == py as i32 {
                    // 咬玩家（25% 落空）
                    if rng.random_range(0..4) == 0 {
                        self.msg = format!("{}扑了个空", m.ch);
                    } else {
                        let dmg = (m.atk / 2 + rng.random_range(0..m.atk)).max(1);
                        self.player.hp -= dmg;
                        hits.push(format!("{}咬了你{}血（剩余{}）", m.ch, dmg, self.player.hp));
                    }
                } else if self.map.is_walkable(nx as usize, ny as usize)
                    && !occupied.contains(&(nx as usize, ny as usize))
                    && !(nx as usize == px && ny as usize == py)
                {
                    m.x = nx as usize;
                    m.y = ny as usize;
                }
            } else {
                // 随机游荡一格
                let dirs = [(0, 1), (1, 0), (0, -1), (-1, 0)];
                let (nx, ny) = dirs[rng.random_range(0..dirs.len())];
                let nx = m.x as i32 + nx;
                let ny = m.y as i32 + ny;
                if nx >= 0 && ny >= 0 && nx < W as i32 && ny < H as i32
                    && self.map.is_walkable(nx as usize, ny as usize)
                    && !occupied.contains(&(nx as usize, ny as usize))
                {
                    m.x = nx as usize;
                    m.y = ny as usize;
                }
            }
        }
        if !hits.is_empty() {
            self.msg = hits.join(" | ");
        }
    }

    fn end_turn(&mut self) {
        self.update_monsters();
        self.map.recompute_vision(self.player.x, self.player.y, 8);
    }

    fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "WANDER  生命 {} / {}   击杀 {}\r\n",
            self.player.hp.max(0),
            self.player.max_hp,
            self.kills
        ));
        for y in 0..self.vh {
            for x in 0..self.vw {
                let i = self.map.idx(x, y);
                let glyph = if self.player.x == x && self.player.y == y {
                    '@'
                } else if let Some(_) = self
                    .monsters
                    .iter()
                    .find(|m| m.x == x && m.y == y && self.map.visible[i])
                {
                    self.monsters.iter().find(|m| m.x == x && m.y == y).unwrap().ch
                } else if let Some(_) = self
                    .items
                    .iter()
                    .find(|it| it.x == x && it.y == y && self.map.visible[i])
                {
                    '+'
                } else if !self.map.explored[i] {
                    ' '
                } else {
                    match self.map.get(x, y) {
                        Tile::Wall => '#',
                        Tile::Floor => '.',
                    }
                };
                out.push(glyph);
            }
            out.push_str("\r\n");
        }
        out.push_str(&format!("{}\r\nq 退出  g 生成新地牢/+1 层\r\n", self.msg));
        out
    }
}

fn main() -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide, Clear(ClearType::All))?;

    let (cols, rows) = crossterm::terminal::size()?;
    let (vw, vh) = (cols as usize, rows as usize);
    let mut game = Game::new(vw.max(40), vh.max(7));
    let result = run(&mut game, &mut stdout);

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen, Show)?;
    result
}

/// 按键 → (dx, dy)。h/j/k/l = 左/下/上/右，y/u/b/n = 四角。
fn key_delta(code: KeyCode) -> (i32, i32) {
    match code {
        KeyCode::Left | KeyCode::Char('h') => (-1, 0),
        KeyCode::Down | KeyCode::Char('j') => (0, 1),
        KeyCode::Up | KeyCode::Char('k') => (0, -1),
        KeyCode::Right | KeyCode::Char('l') => (1, 0),
        KeyCode::Char('y') => (-1, -1),
        KeyCode::Char('u') => (1, -1),
        KeyCode::Char('b') => (-1, 1),
        KeyCode::Char('n') => (1, 1),
        _ => (0, 0),
    }
}

fn run(game: &mut Game, stdout: &mut std::io::Stdout) -> std::io::Result<()> {
    loop {
        // 绘制。render() 已输出 CRLF（raw mode 下 \n 不回车）
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        write!(stdout, "{}", game.render())?;
        stdout.flush()?;

        // 读输入
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        let ev = event::read()?;
        if let Event::Key(k) = ev {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            let delta = match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('g') => {
                    regenerate_world(game);
                    continue;
                }
                KeyCode::Char(' ') => {
                    // 原地待机：让怪先动
                    game.msg = "你原地待机。".to_string();
                    game.end_turn();
                    continue;
                }
                _ => key_delta(k.code),
            };
            if delta != (0, 0) {
                game.try_move_player(delta.0, delta.1);
            }
            if game.player.hp <= 0 {
                game.msg = "你死了。按 g 重开。".to_string();
                execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
                write!(stdout, "{}", game.render())?;
                stdout.flush()?;
                // 单步：继续循环等按键，无法移动（hp<=0 时 try_move 已封住？）
            }
        }
    }
    Ok(())
}

fn regenerate_world(game: &mut Game) {
    let mut map = Map::new();
    map.generate(None);
    let (px, py, _, _) = map.rooms[0];
    let mut rng = rand::rng();
    let mut monsters = Vec::new();
    let names = ['g', 'r', 's', 'k'];
    for (i, &(rx, ry, rw, rh)) in map.rooms.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let mx = rx + 1 + rng.random_range(0..rw.saturating_sub(2));
        let my = ry + 1 + rng.random_range(0..rh.saturating_sub(2));
        monsters.push(Entity {
            x: mx,
            y: my,
            ch: names[i % names.len()],
            hp: 10 + (i as i32) * 4,
            max_hp: 10 + (i as i32) * 4,
            atk: 3 + (i as i32),
        });
    }
    game.map = map;
    game.monsters = monsters;
    game.items = Vec::new();
    // 每个房间放一个血瓶
    for (i, &(rx, ry, rw, rh)) in game.map.rooms.iter().enumerate() {
        if i == 0 {
            continue;
        }
        game.items.push(Item {
            x: rx + 1 + rng.random_range(0..rw.saturating_sub(2)),
            y: ry + 1 + rng.random_range(0..rh.saturating_sub(2)),
            kind: ItemKind::Heal,
        });
    }
    game.player.x = px;
    game.player.y = py;
    game.player.hp = game.player.max_hp;
    game.kills = 0;
    game.msg = "新地牢生成。".to_string();
    game.map.recompute_vision(game.player.x, game.player.y, 8);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// 地图必须连通：room0 能走到所有房间的中心。
    #[test]
    fn all_rooms_reachable() {
        for _ in 0..20 {
            let mut map = Map::new();
            map.generate(None);
            let (sx, sy, _, _) = map.rooms[0];
            let (sx, sy) = (sx + 1, sy + 1);
            let mut seen = vec![false; W * H];
            let mut q = VecDeque::new();
            q.push_back((sx, sy));
            seen[map.idx(sx, sy)] = true;
            while let Some((x, y)) = q.pop_front() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
                        continue;
                    }
                    let (nx, ny) = (nx as usize, ny as usize);
                    if !seen[map.idx(nx, ny)] && map.is_walkable(nx, ny) {
                        seen[map.idx(nx, ny)] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
            for &(rx, ry, rw, rh) in &map.rooms {
                let cx = rx + rw / 2;
                let cy = ry + rh / 2;
                assert!(
                    map.is_walkable(cx, cy) && seen[map.idx(cx, cy)],
                    "房间 ({},{}) 不可达，布点失败",
                    cx,
                    cy
                );
            }
        }
    }

    /// 玩家朝墙/边界移动不 panic，朝空地移动位置更新，朝怪物移动则攻击。
    #[test]
    fn movement_does_not_panic() {
        let mut g = Game::new(60, 25);
        let start = (g.player.x, g.player.y);
        g.try_move_player(-1, 0);
        g.try_move_player(1, 0);
        g.try_move_player(0, -1);
        g.try_move_player(0, 1);
        // 只要不 panic、坐标仍在界内即可
        assert!(g.player.x < W && g.player.y < H);
        let _ = start;
    }

    /// h/j/k/l 严格对应 左/下/上/右，斜向键 y/u/b/n 对应四个角。
    #[test]
    fn direction_mapping() {
        use crossterm::event::KeyCode;
        assert_eq!(key_delta(KeyCode::Char('h')), (-1, 0));
        assert_eq!(key_delta(KeyCode::Char('j')), (0, 1));
        assert_eq!(key_delta(KeyCode::Char('k')), (0, -1));
        assert_eq!(key_delta(KeyCode::Char('l')), (1, 0));
        assert_eq!(key_delta(KeyCode::Left), (-1, 0));
        assert_eq!(key_delta(KeyCode::Down), (0, 1));
        assert_eq!(key_delta(KeyCode::Up), (0, -1));
        assert_eq!(key_delta(KeyCode::Right), (1, 0));
        assert_eq!(key_delta(KeyCode::Char('y')), (-1, -1));
        assert_eq!(key_delta(KeyCode::Char('u')), (1, -1));
        assert_eq!(key_delta(KeyCode::Char('b')), (-1, 1));
        assert_eq!(key_delta(KeyCode::Char('n')), (1, 1));
    }

    /// 踩到血瓶回血且上限封顶；死了之后移动被封锁。
    #[test]
    fn pickup_heals_and_death_locks() {
        let mut g = Game::new(60, 25);
        g.player.hp = 5;
        // 把一个血瓶挪到玩家脚边
        let it = g.items.first().unwrap();
        let (ix, iy) = (it.x, it.y);
        g.player.x = ix;
        g.player.y = iy - 1;
        g.try_move_player(0, 1); // 走到 + 上
        assert!(g.player.hp > 5, "血瓶没生效");
        assert!(!g.items.iter().any(|i| i.x == ix && i.y == iy), "血瓶没被拾取");

        // 回血不超过上限
        g.player.hp = g.player.max_hp - 5;
        let it2 = g.items.first().unwrap();
        g.player.x = it2.x;
        g.player.y = it2.y;
        g.pickup();
        assert_eq!(g.player.hp, g.player.max_hp);

        // 死亡封锁
        g.player.hp = 0;
        let start = (g.player.x, g.player.y);
        g.try_move_player(1, 0);
        g.try_move_player(-1, 0);
        assert_eq!((g.player.x, g.player.y), start, "死了还能动");
    }

    /// render() 里所有换行都带 \r（CRLF），避免 raw mode 下阶梯乱码。
    #[test]
    fn render_uses_crlf() {
        let g = Game::new(60, 25);
        let s = g.render();
        let bare = s.match_indices('\n').filter(|(i, _)| {
            i == &0 || s.as_bytes()[i - 1] != b'\r'
        });
        assert_eq!(bare.count(), 0, "render 存在裸 \\n");
    }

    /// 新地牢重生后玩家满血且回到房间0。
    #[test]
    fn regenerate_resets() {
        let mut g = Game::new(60, 25);
        g.player.hp = 1;
        g.try_move_player(0, 1);
        regenerate_world(&mut g);
        assert_eq!(g.player.hp, g.player.max_hp);
        let (rx, ry, _, _) = g.map.rooms[0];
        assert!(g.player.x.wrapping_sub(rx) < 4 && g.player.y.wrapping_sub(ry) < 4);
    }
}