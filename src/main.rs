mod map;
mod game;
mod ui;

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{stdout, Write};

use crate::game::{key_delta, End, Game, MAX_FLOOR, Ownership, Score};
use crate::ui::{run_homepage, run_shop, HomepageResult};

fn main() -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide, Clear(ClearType::All))?;

    let (cols, rows) = crossterm::terminal::size()?;
    let (vw, vh) = (cols as usize, rows as usize);

    let result = main_loop(vw, vh, &mut stdout);

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen, Show)?;
    result
}

/// 主循环：进入首页 → 商店 → 游戏；结算后回首页。
fn main_loop(vw: usize, vh: usize, stdout: &mut std::io::Stdout) -> std::io::Result<()> {
    loop {
        match run_homepage(stdout)? {
            HomepageResult::Start => {
                let ownership = Ownership::load();
                let purchase = match run_shop(stdout)? {
                    Some(p) => p,
                    None => continue, // 商店里按 q 回首页
                };
                let mut game = Game::new(vw.max(40), vh.max(7), &ownership);
                game.apply_purchase(&purchase);
                let result = run_game(&mut game, stdout)?;
                match result {
                    GameResult::BackToHome => continue, // 回首页
                }
            }
            HomepageResult::Quit => return Ok(()),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum GameResult {
    BackToHome, // 按 q 退出游戏 → 回首页
}

fn run_game(game: &mut Game, stdout: &mut std::io::Stdout) -> std::io::Result<GameResult> {
    // 结算（通关 / 主动结束 / 死亡）每局只落盘一次
    let mut score_persisted = false;
    loop {
        if !matches!(game.end, End::None) && !score_persisted {
            let mut record = Score::load();
            record.add_run(game.kills, game.potions, game.shield_stock.len(), game.blade_count, game.settlement_score());
            game.credits = record.credits;
            game.save_ownership(); // 保存武器归属
            score_persisted = true;
        }
        // 死亡检测：任何按键（含待机/攻击/移动）后都可能在怪物回合死亡，
        // 需在渲染前统一检测，否则死亡画面会延迟到下次移动才显示。
        if game.player.hp <= 0 && matches!(game.end, End::None) {
            game.end = End::Defeat;
            game.msg = format!(
                "你死在了第 {}/{} 层。按 g 回首页（死亡积分折半结算）。",
                game.floor, MAX_FLOOR
            );
        }
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
            // 武器菜单模态：打开时按键只响应菜单，其他按键忽略（仅在游戏进行中）
            if game.weapon_menu && matches!(game.end, End::None) {
                match k.code {
                    KeyCode::Char('1') => {
                        game.switch_to_fists();
                    }
                    KeyCode::Char('2') => {
                        game.equip_blade();
                    }
                    KeyCode::Char('3') => {
                        game.equip_bow();
                    }
                    KeyCode::Char('4') => {
                        game.upgrade_blade();
                    }
                    KeyCode::Char('5') => {
                        game.upgrade_bow();
                    }
                    KeyCode::Char('w') | KeyCode::Esc => {
                        game.toggle_weapon_menu();
                    }
                    _ => {
                        game.weapon_menu = false;
                        game.msg = "关闭武器菜单".to_string();
                    }
                }
                continue;
            }
            let delta = match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('a') => {
                    game.try_attack();
                    continue;
                }
                KeyCode::Char('g') => {
                    if game.can_restart() {
                        if game.boss_down && matches!(game.end, End::None) {
                            // 守护者倒下后主动结束：先画结算面板，再按 g 回首页
                            game.end = End::Voluntary;
                            game.msg = "你决定收手回商店结算。".to_string();
                            continue;
                        }
                        break; // 死亡/本局结束后回商店开新局
                    }
                    game.msg = "你还活着，不能结束（死亡或本层守护者倒下之后才可以；守护者倒下后按 d 下楼）".to_string();
                    continue;
                }
                KeyCode::Char('d') => {
                    game.descend();
                    continue;
                }
                KeyCode::Char('1') => {
                    game.use_potion();
                    continue;
                }
                KeyCode::Char('2') => {
                    game.use_manual();
                    continue;
                }
                KeyCode::Char('3') => {
                    game.use_qi_pill();
                    continue;
                }
                KeyCode::Char(' ') => {
                    // 原地待机：让怪先动；结算界面不再推进回合
                    if matches!(game.end, End::None) {
                        game.msg = "你原地待机。".to_string();
                        game.end_turn();
                    }
                    continue;
                }
                KeyCode::Char('w') => {
                    game.toggle_weapon_menu();
                    continue;
                }
                _ => key_delta(k.code),
            };
            if delta != (0, 0) {
                game.try_move_player(delta.0, delta.1);
            }
            if game.player.hp <= 0 {
                if matches!(game.end, End::None) {
                    game.end = End::Defeat;
                }
                game.msg = format!(
                    "你死在了第 {}/{} 层。按 g 回首页（死亡积分折半结算）。",
                    game.floor, MAX_FLOOR
                );
                execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
                write!(stdout, "{}", game.render())?;
                stdout.flush()?;
                // 单步：继续循环等按键，无法移动（hp<=0 时 try_move 已封住）
            }
        }
    }
    Ok(GameResult::BackToHome)
}

