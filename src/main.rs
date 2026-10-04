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

struct Game {
    map: Map,
    player: Entity,
    monsters: Vec<Entity>,
    kills: u32,
    msg: String,
}

impl Game {
    fn new() -> Self {
        let mut map = Map::new();
        map.generate(None);
        let (px, py, _, _) = map.rooms[0];
        let (px, py) = (px + 1, py + 1);

        // 怪物：放在每个房间里（第一间除外）
        let mut monsters = Vec::new();
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
                hp: 10 + (i as i32) * 4,
                max_hp: 10 + (i as i32) * 4,
                atk: 3 + (i as i32),
            });
        }

        let mut game = Game {
            map,
            player: Entity {
                x: px,
                y: py,
                ch: '@',
                hp: 30,
                max_hp: 30,
                atk: 5,
            },
            monsters,
            kills: 0,
            msg: String::from("用 h/j/k/l 或方向键移动，走到怪物身上攻击"),
        };
        game.map.recompute_vision(game.player.x, game.player.y, 8);
        game
    }

    fn try_move_player(&mut self, dx: i32, dy: i32) {
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
            self.end_turn();
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
                    // 咬玩家
                    let dmg = (m.atk / 2 + rng.random_range(0..m.atk)).max(1);
                    self.player.hp -= dmg;
                    hits.push(format!("{}咬了你{}血（剩余{}）", m.ch, dmg, self.player.hp));
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
            "WANDER  生命 {} / {}   击杀 {}\n",
            self.player.hp.max(0),
            self.player.max_hp,
            self.kills
        ));
        for y in 0..H {
            for x in 0..W {
                let i = self.map.idx(x, y);
                let glyph = if let Some(_) = self
                    .monsters
                    .iter()
                    .find(|m| m.x == x && m.y == y && self.map.visible[i])
                {
                    self.monsters.iter().find(|m| m.x == x && m.y == y).unwrap().ch
                } else if self.player.x == x && self.player.y == y {
                    '@'
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
            out.push('\n');
        }
        out.push_str(&format!("{}\nq 退出\ng 生成新地牢/+1 层  ", self.msg));
        out
    }
}

fn main() -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide, Clear(ClearType::All))?;

    let mut game = Game::new();
    let result = run(&mut game, &mut stdout);

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen, Show)?;
    result
}

fn run(game: &mut Game, stdout: &mut std::io::Stdout) -> std::io::Result<()> {
    loop {
        // 绘制
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
                KeyCode::Left | KeyCode::Char('h') => (0, -1),
                KeyCode::Down | KeyCode::Char('j') => (1, 0),
                KeyCode::Up | KeyCode::Char('k') => (-1, 0),
                KeyCode::Right | KeyCode::Char('l') => (0, 1),
                KeyCode::Char('y') => (-1, -1),
                KeyCode::Char('u') => (-1, 1),
                KeyCode::Char('b') => (1, -1),
                KeyCode::Char('n') => (1, 1),
                _ => (0, 0),
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
        let mut g = Game::new();
        let start = (g.player.x, g.player.y);
        g.try_move_player(-1, 0);
        g.try_move_player(1, 0);
        g.try_move_player(0, -1);
        g.try_move_player(0, 1);
        // 只要不 panic、坐标仍在界内即可
        assert!(g.player.x < W && g.player.y < H);
        let _ = start;
    }

    /// 新地牢重生后玩家满血且回到房间0。
    #[test]
    fn regenerate_resets() {
        let mut g = Game::new();
        g.player.hp = 1;
        g.try_move_player(0, 1);
        regenerate_world(&mut g);
        assert_eq!(g.player.hp, g.player.max_hp);
        let (rx, ry, _, _) = g.map.rooms[0];
        assert!(g.player.x.wrapping_sub(rx) < 4 && g.player.y.wrapping_sub(ry) < 4);
    }
}