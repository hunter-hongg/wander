use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{Clear, ClearType},
};
use std::io::Write;
use crate::game::{display_width, MAX_POTION_BUY, Ownership, PRICE_MANUAL, PRICE_POTION, PRICE_TALISMAN, Purchase, Score};

pub(crate) enum HomepageResult {
    Start, // 按开始 → 进入商店
    Quit,
}

/// 首页屏幕：显示是否拥有骷髅刀和暗影弓，按开始进入商店。
pub(crate) fn render_homepage(ownership: &Ownership, credits: u32) -> String {
    const GOLD: &str = "\x1b[33m";
    const _CYAN: &str = "\x1b[36m"; // 保留供将来使用
    const GREEN: &str = "\x1b[32m";
    const RED: &str = "\x1b[31m";
    const RESET: &str = "\x1b[0m";
    const ORANGE: &str = "\x1b[38;5;214m";
    const BOW_COLOR: &str = "\x1b[38;5;202m";
    const INNER: usize = 38;

    let mut out = String::new();
    let bar = "═".repeat(INNER);

    // 辅助函数：居中显示一行内容
    let centered_row = |content: &str| -> String {
        let pad = INNER.saturating_sub(display_width(content));
        let left = pad / 2;
        let right = pad - left;
        format!(
            "{GOLD}║{RESET}{}{content}{}{GOLD}║{RESET}\r\n",
            " ".repeat(left),
            " ".repeat(right)
        )
    };

    // 辅助函数：左对齐标签 + 右对齐状态
    let weapon_row = |label: &str, label_color: &str, status: &str, status_color: &str| -> String {
        let label_w = display_width(label);
        let status_w = display_width(status);
        let left_pad = 2;
        let right_pad = 2;
        let gap = 4; // 标签与状态之间的间距
        // 内容区域宽度 = INNER = 38
        // 格式串: ║ + left_pad + label + gap + fill + status + right_pad + ║
        // 所以: left_pad + label_w + gap + fill + status_w + right_pad = INNER
        let fill = INNER.saturating_sub(left_pad + label_w + gap + status_w + right_pad);
        format!(
            "{GOLD}║{RESET}  {label_color}{label}{RESET}{}{}{status_color}{status}{RESET}  {GOLD}║{RESET}\r\n",
            " ".repeat(gap),
            " ".repeat(fill)
        )
    };

    out.push_str(&format!("{GOLD}╔{bar}╗{RESET}\r\n"));
    out.push_str(&centered_row("WANDER"));
    out.push_str(&format!("{GOLD}╠{bar}╣{RESET}\r\n"));
    out.push_str(&centered_row(&format!("当前积分：{credits}")));
    out.push_str(&format!("{GOLD}╠{bar}╣{RESET}\r\n"));
    let (blade_text, blade_color) = if ownership.skeleton_blade > 0 {
        (format!("有 · 库存 {} 把 · Lv{}", ownership.skeleton_blade, ownership.blade_level), GREEN)
    } else {
        ("没有".to_string(), RED)
    };
    out.push_str(&weapon_row("† 骷髅刀", ORANGE, &blade_text, blade_color));
    let (bow_text, bow_color) = if ownership.shadow_bow > 0 {
        (format!("有 · 库存 {} 把 · Lv{}", ownership.shadow_bow, ownership.bow_level), GREEN)
    } else {
        ("没有".to_string(), RED)
    };
    out.push_str(&weapon_row("B 暗影弓", BOW_COLOR, &bow_text, bow_color));
    out.push_str(&format!("{GOLD}╠{bar}╣{RESET}\r\n"));
    out.push_str(&centered_row("按 b 开始游戏"));
    out.push_str(&centered_row("按 q 退出游戏"));
    out.push_str(&format!("{GOLD}╚{bar}╝{RESET}\r\n"));
    out
}

/// 首页交互循环：返回 Some(Start) 表示进入商店，None 表示退出游戏。
pub(crate) fn run_homepage(stdout: &mut std::io::Stdout) -> std::io::Result<HomepageResult> {
    let ownership = Ownership::load();
    let credits = Score::load().credits;
    loop {
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        write!(stdout, "{}", render_homepage(&ownership, credits))?;
        stdout.flush()?;

        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        let ev = event::read()?;
        if let Event::Key(k) = ev {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Char('b') | KeyCode::Enter | KeyCode::Char(' ') => {
                    return Ok(HomepageResult::Start);
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(HomepageResult::Quit),
                _ => {}
            }
        }
    }
}

/// 开局商店：花上一局通关赚的积分备货。1/2/3 购买，b 开始，q 退出。
pub(crate) struct Shop {
    pub(crate) credits: u32,
    pub(crate) manual: bool,
    pub(crate) talisman: bool,
    pub(crate) potions: usize,
    pub(crate) msg: String,
}

impl Shop {
    pub(crate) fn new() -> Self {
        Shop {
            credits: Score::load().credits,
            manual: false,
            talisman: false,
            potions: 0,
            msg: String::new(),
        }
    }

    /// 购买：0 = 武林秘籍，1 = 幸运符，2 = 血瓶。
    pub(crate) fn buy(&mut self, kind: u32) {
        let (price, name, desc) = match kind {
            0 => (
                PRICE_MANUAL,
                "武林秘籍",
                "开局自带一本，局内按 2 参悟",
            ),
            1 => (
                PRICE_TALISMAN,
                "幸运符",
                "增加你的幸运值",
            ),
            _ => (PRICE_POTION, "血瓶", "开局自带一瓶，局内按 1 回血"),
        };
        if kind == 0 && self.manual {
            self.msg = "已经备了一本草林秘籍".to_string();
            return;
        }
        if kind == 1 && self.talisman {
            self.msg = "已经备了一张幸运符".to_string();
            return;
        }
        if kind == 2 && self.potions >= MAX_POTION_BUY {
            self.msg = format!(
                "血瓶每局限购 {} 个，你已经买了 x{}",
                MAX_POTION_BUY, self.potions
            );
            return;
        }
        if self.credits < price {
            self.msg = format!(
                "积分不够：{} 要 {} 积分，你只有 {}",
                name, price, self.credits
            );
            return;
        }
        self.credits -= price;
        match kind {
            0 => self.manual = true,
            1 => self.talisman = true,
            _ => self.potions += 1,
        }
        self.msg = format!(
            "购得{}（{} 积分），剩 {} 积分：{}",
            name, price, self.credits, desc
        );
    }

    pub(crate) fn to_purchase(&self) -> Purchase {
        Purchase {
            manual: self.manual,
            talisman: self.talisman,
            potions: self.potions,
        }
    }

    pub(crate) fn render(&self) -> String {
        const GOLD: &str = "\x1b[33m";
        const CYAN: &str = "\x1b[36m";
        const GREEN: &str = "\x1b[32m";
        const RESET: &str = "\x1b[0m";
        let mark = |b: bool| {
            if b {
                format!("{GREEN}✓{RESET}")
            } else {
                format!("{CYAN}○{RESET}")
            }
        };
        let mut out = String::new();
        out.push_str(&format!("{GOLD}商  店{RESET}\r\n"));
        out.push_str(&format!(
            "当前积分：{GOLD}{}{RESET}（结算赚积分，开局在此花）\r\n\r\n",
            self.credits
        ));
        out.push_str(&format!(
            "{}[1] {CYAN}武林秘籍{RESET}  {GOLD}{} 积分{RESET}\r\n    开局自带一本，局内按 2 参悟  {}\r\n",
            mark(self.manual),
            PRICE_MANUAL,
            if self.manual { "已备" } else { "未购" }
        ));
        out.push_str(&format!(
            "{}[2] {CYAN}幸运符{RESET}  {GOLD}{} 积分{RESET}\r\n    增加你的幸运值  {}\r\n",
            mark(self.talisman),
            PRICE_TALISMAN,
            if self.talisman { "已备" } else { "未购" }
        ));
        out.push_str(&format!(
            "{}[3] {CYAN}血瓶 x{}{RESET}  {GOLD}{} 积分/瓶{RESET}\r\n    开局自带，局内按 1 回血  {}\r\n",
            mark(self.potions > 0),
            self.potions,
            PRICE_POTION,
            if self.potions > 0 {
                format!("已备 x{}", self.potions)
            } else {
                "未购".to_string()
            }
        ));
        out.push_str(&format!(
            "\r\n按 1/2/3 购买    按 b 开始游戏    按 q 退出\r\n{}\r\n",
            self.msg
        ));
        out
    }
}

/// 商店交互循环：返回 Some(购买结果) 表示开新局，None 表示退出游戏。
pub(crate) fn run_shop(stdout: &mut std::io::Stdout) -> std::io::Result<Option<Purchase>> {
    let mut shop = Shop::new();
    loop {
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        write!(stdout, "{}", shop.render())?;
        stdout.flush()?;
        if !event::poll(std::time::Duration::from_millis(50))? {
            continue;
        }
        let ev = event::read()?;
        if let Event::Key(k) = ev {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Char('1') => shop.buy(0),
                KeyCode::Char('2') => shop.buy(1),
                KeyCode::Char('3') => shop.buy(2),
                KeyCode::Char('q') | KeyCode::Esc => return Ok(None),
                KeyCode::Char('b') | KeyCode::Enter | KeyCode::Char(' ') => {
                    return Ok(Some(shop.to_purchase()))
                }
                _ => {}
            }
        }
    }
}

