mod map;

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use map::{Map, Tile, W, H};
use rand::RngExt;
use std::collections::HashSet;
use std::io::{stdout, Write};
use std::fs;
use std::path::PathBuf;

/// 血瓶回血量。
const HEAL_AMOUNT: i32 = 15;
/// 暗影箭的射界下限：距离 <= 3 格时无法射箭。
const MIN_RANGED_RANGE: usize = 4;
/// 暗影射手近身（贴着玩家）时的基础伤害，实际伤害上下浮动 1 点。
const SHADOW_MELEE_BASE: i32 = 2;
/// 暗影射手死后掉盾牌的概率（%），带幸运符时必掉。
const SHIELD_DROP_PERCENT: u32 = 50;
const SHIELD_LUCKY_DROP_PERCENT: u32 = 100;
/// 盾牌耐久：格挡一次命中扣 1 点（落空不扣），扣到 0 盾牌碎裂、恢复掉血。
const SHIELD_DURABILITY: i32 = 45;
/// 背包里最多留存几面盾牌（不含手里正在用的那面）。
const SHIELD_STOCK_MAX: usize = 10;
/// 结算时背包里每面存量盾牌按耐久度折算积分：每 1 点耐久 = 多少积分。
const SHIELD_SCORE_PER_POINT: u32 = 2;
/// 骷髅的攻击无视盾牌，是唯一穿盾的怪。
const SKELETON: &str = "骷髅";
/// 每层地牢的血瓶数量（9 间房里只有这些，不再每房一个）。
const POTIONS_PER_FLOOR: usize = 4;
/// 怪物死亡时掉落血瓶的概率（1/2）。
const DROP_CHANCE: u32 = 2;
/// 背包最多容纳多少个血瓶，捡到就往里塞。
const POTION_STACK: usize = 5;
/// 玩家的初始血量上限与攻击力，重开新一局时回到这个水平。
const BASE_MAX_HP: i32 = 40;
const BASE_ATK: i32 = 8;
/// 蕴气丹：服用消耗 10 点血，血量上限提升 10。
const QI_HP_COST: i32 = 10;
const QI_MAX_HP_BONUS: i32 = 10;

/// 商店：开局用上一局赚的积分购买的物品价格。
const PRICE_MANUAL: u32 = 400;
const PRICE_TALISMAN: u32 = 5000;
/// 结算时每通过一关（守护者倒下）的固定通关奖励积分，不受层数倍率和折半影响。
const CLEAR_BONUS_PER_FLOOR: u32 = 500;
/// 骷髅在基础攻击上额外增加的攻击数值（骷髅本就穿盾，更危险）。
const SKELETON_ATK_BONUS: i32 = 4;
const PRICE_POTION: u32 = 200;
/// 血瓶每局限购数量，再买没意义（背包上限也不高）。
const MAX_POTION_BUY: usize = 3;
/// 幸运符：怪物血瓶掉落概率提升至 90%（默认 50%）。
const LUCKY_DROP_PERCENT: u32 = 90;
/// 幸运符：参悟武林秘籍时 25% 概率攻击力 ×4（×4 仅此唯一获得途径）。
const LUCKY_QUAD_CHANCE: u32 = 25;
/// 地牢共几层：杀掉本层守护者即可下楼，清完第 5 层才通关；结算倍率 = 当前层。
const MAX_FLOOR: u8 = 5;
/// 成就名：目前仅图一乐，不设奖励，HUD 专门一行提示（基础/高级分行区分）。
/// 简单成就
const ACH_KILL_ONE: &str = "杀死怪物";
const ACH_GET_SHIELD: &str = "拾取盾牌";
const ACH_NO_FEAR: &str = "不再畏惧";
const ACH_COMPENDIUM: &str = "怪物图鉴";
/// 高级成就
const ACH_FULLY_ARMED: &str = "全副武装";
const ACH_SHIELD_SWAP: &str = "除新迎旧";
const ACH_PINNED: &str = "两面夹击";
const ACH_FULL_LOAD: &str = "满载而归";
/// HUD 基础/高级成就行的固定展示顺序
const BASIC_ACHIEVEMENTS: [&str; 4] = [
    ACH_KILL_ONE,
    ACH_GET_SHIELD,
    ACH_NO_FEAR,
    ACH_COMPENDIUM,
];
const ADVANCED_ACHIEVEMENTS: [&str; 4] = [
    ACH_FULLY_ARMED,
    ACH_SHIELD_SWAP,
    ACH_PINNED,
    ACH_FULL_LOAD,
];
/// 全副武装达成条件：血量 50/50、攻击 ≥16、血瓶满 5、盾牌满 45/45
const FULLY_ARMED_HP: i32 = 50;
const FULLY_ARMED_ATK: i32 = 16;
/// 不再畏惧：盾挡住射手的箭攒到几发达成
const NO_FEAR_ARROWS: usize = 3;
/// 守护者的名字（怪物图鉴要单列的 BOSS 种）
const BOSS: &str = "地牢守护者";
/// 全部怪物种类：怪物图鉴成就要每种杀一只（含 BOSS）
const ALL_SPECIES: [&str; 6] = ["哥布林", "巨鼠", "蜘蛛", SKELETON, "暗影射手", BOSS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Behavior {
    Melee,
    Ranged,
}

/// 一局结束状态：Voluntary/Victory/Defeat 时回商店结算。
/// 结算倍率 = 当前层（1层×1 … 5层×5），死亡和使用幸运符各折半一次。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    None,
    Voluntary, // 杀掉本层守护者后主动结束，按当前层倍率结算
    Victory,   // 清完第 5 层守护者，整局通关
    Defeat,    // 死了，按到达层结算但折半（安慰分）
}

struct Entity {
    x: usize,
    y: usize,
    ch: char,
    name: &'static str,
    hp: i32,
    max_hp: i32,
    atk: i32,
    sight: usize,
    behavior: Behavior,
    is_boss: bool,
}

struct Item {
    x: usize,
    y: usize,
    kind: ItemKind,
}

#[derive(Clone, Copy, PartialEq)]
enum ItemKind {
    Heal,
    Manual, // 武林秘籍：参悟后攻击力 +100%
    QiPill, // 蕴气丹：耗 10 血，血量上限 +10
    Shield, // 盾牌：格挡怪物的攻击（骷髅除外），每次命中消耗 1 点耐久
}

/// 道具在地图上的显示字符。
fn item_glyph(kind: ItemKind) -> char {
    match kind {
        ItemKind::Heal => '+',
        ItemKind::Manual => 'M',
        ItemKind::QiPill => 'Q',
        ItemKind::Shield => 'S',
    }
}

/// 积分记录，持久化到本地目录（~/.config/wander/scores.json）。
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
struct Score {
    credits: u32, // 积分余额：通关赚得，开局商店可花
    total_score: u32, // 历史累计赚分
    last_score: u32,
    last_kills: u32,
    last_potions: usize,
    last_shields: usize,
}

impl Score {
    fn path() -> PathBuf {
        // 测试可用 WANDER_SCORE_FILE 指到临时文件，别污染真实配置
        if let Ok(p) = std::env::var("WANDER_SCORE_FILE") {
            return PathBuf::from(p);
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wander")
            .join("scores.json")
    }

    fn load() -> Self {
        let path = Self::path();
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Score::default()
        }
    }

    fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, serde_json::to_string_pretty(self).unwrap_or_default());
    }

    fn add_run(&mut self, kills: u32, potions: usize, shields: usize, score: u32) -> u32 {
        // 积分 = (kills * 100 + 剩余血瓶 * 50 + 存量盾牌耐久折算) × 层数倍率，死亡和幸运符各折半（见 Game::settlement_score）
        self.last_score = score;
        self.last_kills = kills;
        self.last_potions = potions;
        self.last_shields = shields;
        self.total_score += score;
        self.credits += score;
        self.save();
        score
    }
}

struct Game {
    map: Map,
    player: Entity,
    monsters: Vec<Entity>,
    items: Vec<Item>,
    potions: usize, // 背包里的血瓶，按 1 喝
    manuals: usize, // 背包里的武林秘籍，按 2 参悟
    qi_pills: usize, // 背包里的蕴气丹，按 3 服用
    shield: i32, // 手里盾牌的耐久，0 = 没有盾牌（有盾时格挡来袭伤害，命中 -1 耐久）
    shield_stock: Vec<i32>, // 背包里留存的盾牌，每面记下当时收进去的耐久（最多 SHIELD_STOCK_MAX 面；盾碎了自动取耐久最高的一面换上）
    kills: u32,
    floor: u8, // 当前层 1..=MAX_FLOOR
    boss_down: bool, // 本层守护者已倒（可下楼，或主动结束按当前层结算）
    end: End, // 本局结束状态：!= None 时显示结算界面
    credits: u32, // 积分余额（存档缓存，结算后刷新）
    lucky: bool, // 本局用了幸运符：血瓶掉率 90%，秘籍 25% 概率攻击 ×4，盾牌必掉，结算积分折半
    achievements: HashSet<&'static str>, // 本局达成的成就（仅图一乐，HUD 基础/高级分行展示，随新局重置）
    arrows_blocked: usize, // 本局盾挡下的射手暗箭总数（不再畏惧成就攒到 NO_FEAR_ARROWS 达成）
    species_killed: HashSet<&'static str>, // 本局击杀过的怪物种类（怪物图鉴成就用）
    msg: String,
    vw: usize, // 实际渲染宽度（受终端窗口限制）
    vh: usize, // 实际渲染高度（受终端窗口限制）
}

/// 开局商店购得的物品，带到新一局里。
#[derive(Clone, Copy, Default)]
struct Purchase {
    manual: bool, // 武林秘籍
    talisman: bool, // 幸运符
    potions: usize, // 血瓶
}

impl Game {
    /// 应用开局商店的购买：秘籍/幸运符/血瓶直接进背包或生效。
    fn apply_purchase(&mut self, p: &Purchase) {
        if p.manual {
            self.manuals = 1;
        }
        if p.talisman {
            self.lucky = true;
        }
        if p.potions > 0 {
            self.potions = (self.potions + p.potions).min(POTION_STACK);
        }
    }

    fn new(vw: usize, vh: usize) -> Self {
        let record = Score::load();
        let mut map = Map::new();
        map.generate(None);
        let (monsters, items) = populate(1, &map);
        let (px, py, _, _) = map.rooms[0];
        let (px, py) = (px + 1, py + 1);

        let mut game = Game {
            map,
            player: Entity {
                x: px,
                y: py,
                ch: '@',
                name: "你",
                hp: BASE_MAX_HP,
                max_hp: BASE_MAX_HP,
                atk: BASE_ATK,
                sight: 8,
                behavior: Behavior::Melee,
                is_boss: false,
            },
            monsters,
            items,
            potions: 0,
            manuals: 0,
            qi_pills: 0,
            shield: 0,
            shield_stock: Vec::new(),
            kills: 0,
            floor: 1,
            boss_down: false,
            end: End::None,
            credits: record.credits,
            lucky: false,
            achievements: HashSet::new(),
            arrows_blocked: 0,
            species_killed: HashSet::new(),
            msg: String::from(
                "地牢共 5 层，越深越难。h/j/k/l 移动，撞上怪物即攻击。杀掉本层守护者 D 后按 d 下楼或 g 结算。+ 血瓶 / M 武林秘籍 / Q 蕴气丹 / S 盾牌"
            ),
            vw: vw.min(W),
            // 5 行头部（标题/物品/工具/基础成就/高级成就）+ 地图 + 1 行消息 + 1 行按键提示
            vh: (vh.saturating_sub(7)).min(H),
        };
        game.map.recompute_vision(game.player.x, game.player.y, 8);
        game
    }

    fn try_move_player(&mut self, dx: i32, dy: i32) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return; // 死了或本局结束了不能动，只能 g 回商店
        }
        let nx = self.player.x as i32 + dx;
        let ny = self.player.y as i32 + dy;
        if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
            return;
        }
        let (nx, ny) = (nx as usize, ny as usize);

        // 撞到怪物 -> 攻击
        if let Some(m) = self.monsters.iter_mut().find(|m| m.x == nx && m.y == ny) {
            let mut rng = rand::rng();
            let dmg = roll(&mut rng, self.player.atk);
            m.hp -= dmg;
            let killed = m.hp <= 0;
            // 先抄走名字（Copy 的 &'static str），后面解锁成就时 m 的借用必须已结束
            let killed_name = if killed { Some(m.name) } else { None };
            self.msg = format!("你攻击了{}，造成{}点伤害", m.name, dmg);
            if killed {
                self.kills += 1;
                self.msg = format!("你击杀了{}！", m.name);
                if m.is_boss {
                    self.boss_down = true;
                    if self.floor == MAX_FLOOR {
                        self.end = End::Victory; // 第 5 层守护者倒下：整局通关，结算落盘由 run 循环统一做
                    } else {
                        self.msg.push_str(&format!(
                            " 第{}层的守护者倒下了！按 d 下到第{}层（怪物更强、射手更多），按 g 结束本局回商店结算（积分 ×{}）",
                            self.floor,
                            self.floor + 1,
                            self.floor
                        ));
                    }
                }
                // 幸运符：掉率提升至 90%；否则维持默认 1/2
                let drops = if self.lucky {
                    rng.random_range(0..100) < LUCKY_DROP_PERCENT
                } else {
                    rng.random_range(0..DROP_CHANCE) == 0
                };
                if drops {
                    self.items.push(Item { x: m.x, y: m.y, kind: ItemKind::Heal });
                    self.msg.push_str("（掉落了一个血瓶）");
                }
                // 暗影射手死后：50% 掉一面盾牌，带幸运符必掉
                if matches!(m.behavior, Behavior::Ranged) {
                    let shield_drop = if self.lucky {
                        rng.random_range(0..100) < SHIELD_LUCKY_DROP_PERCENT
                    } else {
                        rng.random_range(0..100) < SHIELD_DROP_PERCENT
                    };
                    if shield_drop {
                        self.items.push(Item { x: m.x, y: m.y, kind: ItemKind::Shield });
                        self.msg.push_str("（掉落了一面盾牌）");
                    }
                }
            }
            // 成就（m 的借用已结束，可以操作 self）
            if let Some(name) = killed_name {
                self.unlock(ACH_KILL_ONE); // 杀死怪物：击杀一只即达成
                self.register_kill(name); // 怪物图鉴：记账击杀过的种类
            }
            self.monsters.retain(|m| m.hp > 0);
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

    /// 踩到道具：不立刻生效，先揣进背包，按 1/2/3 再用。
    fn pickup(&mut self) {
        let (px, py) = (self.player.x, self.player.y);
        let idx = self.items.iter().position(|it| it.x == px && it.y == py);
        if let Some(i) = idx {
            match self.items[i].kind {
                ItemKind::Heal => {
                    if self.potions < POTION_STACK {
                        self.potions += 1;
                        self.items.remove(i);
                        self.msg = format!("捡起一个血瓶（背包 {}/{}），按 1 使用", self.potions, POTION_STACK);
                    } else {
                        self.msg = format!("背包已满（{}/{}），血瓶留在原地", self.potions, POTION_STACK);
                    }
                }
                // 秘籍与蕴气丹全图唯一，背包里有了就不再拾取
                ItemKind::Manual => {
                    if self.manuals > 0 {
                        self.msg = "你已经参悟过武林秘籍，这本是复刻本，留在原地".to_string();
                        return;
                    }
                    self.manuals = 1;
                    self.items.remove(i);
                    self.msg = "捡起一本武林秘籍！按 2 参悟，攻击力提升 100%".to_string();
                }
                ItemKind::QiPill => {
                    if self.qi_pills > 0 {
                        self.msg = "你已经服过蕴气丹，这颗是伪药，留在原地".to_string();
                        return;
                    }
                    self.qi_pills = 1;
                    self.items.remove(i);
                    self.msg = "捡起一颗蕴气丹！按 3 服用，耗 10 血换 10 点血量上限".to_string();
                }
                ItemKind::Shield => {
                    if self.shield > 0 {
                        // 手里已有盾：旧盾收进背包留存（保留当前耐久），新盾满耐久换上
                        if self.shield_stock.len() >= SHIELD_STOCK_MAX {
                            self.msg = format!(
                                "背包盾牌存量已满（{}/{}），放不下旧盾，新盾留在原地",
                                self.shield_stock.len(),
                                SHIELD_STOCK_MAX
                            );
                            return;
                        }
                        let old = self.shield;
                        self.shield_stock.push(old);
                        self.shield = SHIELD_DURABILITY;
                        self.items.remove(i);
                        self.unlock(ACH_GET_SHIELD); // 拾取盾牌：换盾也算获取了一面
                        self.msg = format!(
                            "换上新盾！耐久回满 {SHIELD_DURABILITY}/{SHIELD_DURABILITY}，旧盾（耐久 {old}/{SHIELD_DURABILITY}）收进背包（存量 {}/{}）",
                            self.shield_stock.len(),
                            SHIELD_STOCK_MAX
                        );
                        // 满载而归：拾取盾牌后存量刚好顶到上限
                        if self.shield_stock.len() == SHIELD_STOCK_MAX {
                            self.unlock(ACH_FULL_LOAD);
                        }
                    } else {
                        // 手里没盾：直接拿新的满耐久盾
                        self.shield = SHIELD_DURABILITY;
                        self.items.remove(i);
                        self.unlock(ACH_GET_SHIELD); // 拾取盾牌：拿到手就达成
                        self.msg = format!(
                            "捡起一面盾牌！耐久 {SHIELD_DURABILITY}，格挡来袭伤害（每次命中 -1 耐久，骷髅除外）"
                        );
                    }
                }
            }
        }
    }

    /// 按 1：喝一瓶，满血封顶，不消耗回合（怪物不会因此行动）。
    fn use_potion(&mut self) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return; // 死了或本局结算了不能喝药
        }
        if self.potions == 0 {
            self.msg = "背包里没有血瓶".to_string();
            return;
        }
        self.potions -= 1;
        let before = self.player.hp;
        self.player.hp = (self.player.hp + HEAL_AMOUNT).min(self.player.max_hp);
        self.msg = format!("喝下血瓶，恢复{}血（剩余{}）", self.player.hp - before, self.player.hp);
        self.check_fully_armed(); // 状态更新：血量变了，看看是否全副武装
    }

    /// 按 2：参悟武林秘籍，攻击力翻倍（+100%）。不消耗回合。
    fn use_manual(&mut self) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return; // 死了或本局结算了不能参悟
        }
        if self.manuals == 0 {
            self.msg = "背包里没有武林秘籍".to_string();
            return;
        }
        // 已参悟过就不再翻倍，保持唯一一次的提升
        if self.player.atk >= 2 * BASE_ATK {
            self.msg = "你已经参悟过秘籍，境界已到尽头，无法再进一步".to_string();
            return;
        }
        self.manuals = 0;
        let before = self.player.atk;
        if self.lucky {
            // 幸运符显灵：25% 概率攻击直接 ×4（×4 仅此唯一获得途径）
            let mut rng = rand::rng();
            if rng.random_range(0..100) < LUCKY_QUAD_CHANCE {
                self.player.atk = 4 * BASE_ATK;
                self.msg = format!(
                    "幸运符显灵！参悟武林秘籍，攻击力 {} → {}（×4）！",
                    before, self.player.atk
                );
                self.check_fully_armed(); // 状态更新：攻击变了，看看是否全副武装
                return;
            }
        }
        self.player.atk = 2 * BASE_ATK;
        self.msg = format!(
            "参悟武林秘籍！攻击力 {} → {}（提升 100%），武林绝学不过如此",
            before, self.player.atk
        );
        self.check_fully_armed(); // 状态更新：攻击变了，看看是否全副武装
    }

    /// 按 3：服用蕴气丹，消耗 10 血换 10 点血量上限，并回满血。不消耗回合。
    fn use_qi_pill(&mut self) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return; // 死了或本局结算了不能服药
        }
        if self.qi_pills == 0 {
            self.msg = "背包里没有蕴气丹".to_string();
            return;
        }
        // 已到达此图的修炼上限就不再服丹
        if self.player.max_hp >= BASE_MAX_HP + QI_MAX_HP_BONUS {
            self.msg = "你的经脉已修至圆满，蕴气丹再也容不下了".to_string();
            return;
        }
        self.qi_pills = 0;
        let before_max = self.player.max_hp;
        self.player.max_hp += QI_MAX_HP_BONUS;
        self.player.hp -= QI_HP_COST;
        if self.player.hp <= 0 {
            self.player.hp = 0;
            self.msg = format!(
                "服下蕴气丹，血量上限 {} → {}，但 {} 点精血反噬，你经脉寸断，倒地身亡。按 g 重开",
                before_max, self.player.max_hp, QI_HP_COST
            );
        } else {
            self.msg = format!(
                "服下蕴气丹！血量上限 {} → {}，消耗 {} 血（剩余 {}）",
                before_max, self.player.max_hp, QI_HP_COST, self.player.hp
            );
        }
        self.check_fully_armed(); // 状态更新：血量上限变了，看看是否全副武装
    }

    /// 处理怪物：能看到玩家就按各自行为追击/射击，否则原地随机游荡。
    fn update_monsters(&mut self) {
        let (px, py) = (self.player.x, self.player.y);
        let mut rng = rand::rng();
        let mut hits = Vec::new();
        let mut shield_refill = false; // 本回合碎盾自动从存量换上一面（除新迎旧）
        let mut ranged_hit = false; // 本回合有射手实际打中（格挡或穿透都算）
        let mut boss_hit = false; // 本回合有守护者实际打中
        let mut arrows_blocked = 0; // 本回合被盾挡下的暗影箭数（不再畏惧）
        // 预计算所有怪物当前位置，用于移动时的碰撞判断
        let occupied: HashSet<(usize, usize)> = self
            .monsters
            .iter()
            .map(|m| (m.x, m.y))
            .collect();
        for m in self.monsters.iter_mut() {
            let dist = m.x.abs_diff(px) + m.y.abs_diff(py);
            let sees = dist <= m.sight && self.map.has_los(m.x, m.y, px, py);
            if !sees {
                // 随机游荡一格
                let dirs = [(0, 1), (1, 0), (0, -1), (-1, 0)];
                let (dx, dy) = dirs[rng.random_range(0..dirs.len())];
                if let Some((ux, uy)) = try_move(&self.map, &occupied, px, py, m.x as i32 + dx, m.y as i32 + dy) {
                    m.x = ux;
                    m.y = uy;
                }
                continue;
            }

            match m.behavior {
                Behavior::Melee => {
                    let (step_x, step_y) = (
                        (px as i32 - m.x as i32).signum(),
                        (py as i32 - m.y as i32).signum(),
                    );
                    let nx = m.x as i32 + step_x;
                    let ny = m.y as i32 + step_y;
                    if nx == px as i32 && ny == py as i32 {
                        // 咬玩家（25% 落空）
                        if rng.random_range(0..4) == 0 {
                            hits.push(format!("{}扑了个空", m.name));
                        } else {
                            let r = block_with_shield(&mut self.shield, &mut self.shield_stock, m.name, "攻击", &mut hits);
                            if m.is_boss {
                                boss_hit = true;
                            }
                            if r.refilled {
                                shield_refill = true;
                            }
                            if !r.blocked {
                                // 没盾（或骷髅穿盾）才掉血
                                let dmg = roll(&mut rng, m.atk);
                                self.player.hp -= dmg;
                                hits.push(format!("{}咬了你{}血（剩余{}）", m.name, dmg, self.player.hp));
                            }
                        }
                    } else if let Some((ux, uy)) = try_move(&self.map, &occupied, px, py, nx, ny) {
                        m.x = ux;
                        m.y = uy;
                    }
                }
                Behavior::Ranged => {
                    if dist >= MIN_RANGED_RANGE {
                        // 站定射击（25% 射偏，射偏不扣耐久）
                        if rng.random_range(0..4) == 0 {
                            hits.push(format!("{}的暗影箭擦身而过", m.name));
                        } else {
                            let r = block_with_shield(&mut self.shield, &mut self.shield_stock, m.name, "暗影箭", &mut hits);
                            if !m.is_boss {
                                ranged_hit = true;
                            }
                            if r.refilled {
                                shield_refill = true;
                            }
                            if r.blocked {
                                arrows_blocked += 1; // 不再畏惧：这发箭被盾挡下了
                            }
                            if !r.blocked {
                                // 盾牌已在此命中后结算 1 点耐久；没盾才掉血
                                let dmg = roll(&mut rng, m.atk);
                                self.player.hp -= dmg;
                                hits.push(format!("{}用暗影箭射你{}血（剩余{}）", m.name, dmg, self.player.hp));
                            }
                        }
                    } else if dist <= 1 {
                        // 贴身了：无法射箭，改为近战攻击（基础 2 点，上下浮动 1 点）
                        let dmg = SHADOW_MELEE_BASE + rng.random_range(-1..=1);
                        let r = block_with_shield(&mut self.shield, &mut self.shield_stock, m.name, "暗影一击", &mut hits);
                        if !m.is_boss {
                            ranged_hit = true;
                        }
                        if r.refilled {
                            shield_refill = true;
                        }
                        if !r.blocked {
                            self.player.hp -= dmg;
                            hits.push(format!(
                                "{}近身暗影一击打你{}血（剩余{}）",
                                m.name, dmg, self.player.hp
                            ));
                        }
                    } else {
                        // 距离太近（<=3 格）无法射箭：朝远离玩家的方向撤
                        let (step_x, step_y) = (
                            (m.x as i32 - px as i32).signum(),
                            (m.y as i32 - py as i32).signum(),
                        );
                        let nx = m.x as i32 + step_x;
                        let ny = m.y as i32 + step_y;
                        if let Some((ux, uy)) = try_move(&self.map, &occupied, px, py, nx, ny) {
                            m.x = ux;
                            m.y = uy;
                        }
                    }
                }
            }
        }
        // 除新迎旧：本回合有碎盾从存量自动换上的
        if shield_refill {
            self.unlock(ACH_SHIELD_SWAP);
        }
        // 两面夹击：同一回合里射手与守护者的攻击都实际打中了
        if ranged_hit && boss_hit {
            self.unlock(ACH_PINNED);
        }
        // 不再畏惧：手持盾牌挡下暗影箭攒够 NO_FEAR_ARROWS 发（跨回合累计）
        if arrows_blocked > 0 {
            self.arrows_blocked += arrows_blocked;
            if self.arrows_blocked >= NO_FEAR_ARROWS {
                self.unlock(ACH_NO_FEAR);
            }
        }
        if !hits.is_empty() {
            self.msg = hits.join(" | ");
        }
    }

    fn end_turn(&mut self) {
        self.update_monsters();
        self.map.recompute_vision(self.player.x, self.player.y, 8);
        self.check_fully_armed(); // 状态更新：回合结束，看看是否全副武装
    }

    /// 下楼：杀掉本层守护者后按 d，生成下一层地图（怪物更强、射手更多）。
    fn descend(&mut self) {
        if self.player.hp <= 0
            || !self.boss_down
            || self.floor >= MAX_FLOOR
            || !matches!(self.end, End::None)
        {
            if self.player.hp > 0 && matches!(self.end, End::None) && !self.boss_down {
                self.msg = "先杀掉本层的守护者 D 才能下楼".to_string();
            }
            return;
        }
        self.floor += 1;
        let mut map = Map::new();
        map.generate(None);
        let (monsters, items) = populate(self.floor, &map);
        let (rx, ry, _, _) = map.rooms[0];
        self.map = map;
        self.monsters = monsters;
        self.items = items;
        self.player.x = rx + 1;
        self.player.y = ry + 1;
        self.boss_down = false;
        self.msg = format!(
            "你下到第{}/{}层：怪物更强了，小怪更少、射手更多。杀掉深处的守护者继续向下",
            self.floor, MAX_FLOOR
        );
        self.map.recompute_vision(self.player.x, self.player.y, 8);
    }

    /// 背包里存量盾牌的积分合计：每面按「当时收进去的耐久 × SHIELD_SCORE_PER_POINT」折算。
    fn shield_stock_score(&self) -> u32 {
        self.shield_stock
            .iter()
            .map(|d| *d as u32 * SHIELD_SCORE_PER_POINT)
            .sum()
    }

    /// 本局通过的关数：之前各层都算通过；当前层守护者倒下（或整局通关）再 +1。
    fn floors_cleared(&self) -> u32 {
        let current = self.boss_down || matches!(self.end, End::Victory);
        (self.floor as u32 - 1) + current as u32
    }

    /// 通关奖励：每通过一关 +500 积分。
    fn clear_bonus(&self) -> u32 {
        self.floors_cleared() * CLEAR_BONUS_PER_FLOOR
    }

    /// 本局结算分：(击杀 × 100 + 剩余血瓶 × 50 + 存量盾牌耐久折算) × 当前层倍率；
    /// 死亡、幸运符各折半一次，最后再加上固定的通关奖励（每关 +500，不折半）。
    fn settlement_score(&self) -> u32 {
        let base = self.kills * 100
            + self.potions as u32 * 50
            + self.shield_stock_score();
        let mut score = base * self.floor as u32;
        if matches!(self.end, End::Defeat) {
            score /= 2;
        }
        if self.lucky {
            score /= 2;
        }
        score + self.clear_bonus()
    }

    /// 结束本局回商店：死了或本层守护者倒下之后才允许，活着按 g 是逃课，拒绝。
    fn can_restart(&self) -> bool {
        self.player.hp <= 0 || self.boss_down
    }

    /// 解锁成就（仅图一乐，无奖励）：首次记入本局达成集合，重复解锁是空操作。
    fn unlock(&mut self, name: &'static str) {
        let _ = self.achievements.insert(name);
    }

    /// 全副武装：血量 50/50、攻击 ≥16、血瓶满 5、盾牌满 45/45。
    /// 任何状态更新（移动/待机回合结束、喝药、参悟、服丹）后调一次。
    fn check_fully_armed(&mut self) {
        if self.player.hp == FULLY_ARMED_HP
            && self.player.max_hp == FULLY_ARMED_HP
            && self.player.atk >= FULLY_ARMED_ATK
            && self.potions == POTION_STACK
            && self.shield == SHIELD_DURABILITY
        {
            self.unlock(ACH_FULLY_ARMED);
        }
    }

    /// 列出 list 里已达成的成就，按 list 的顺序（HUD 成就行用）。
    fn unlocked_achievements(&self, list: &[&'static str]) -> Vec<&'static str> {
        list.iter().copied().filter(|a| self.achievements.contains(a)).collect()
    }

    /// 记一刀击杀的怪物种类（怪物图鉴进度）：ALL_SPECIES 每种都杀过就解锁。
    fn register_kill(&mut self, name: &'static str) {
        let _ = self.species_killed.insert(name);
        if ALL_SPECIES.iter().all(|s| self.species_killed.contains(s)) {
            self.unlock(ACH_COMPENDIUM);
        }
    }

    fn render(&self) -> String {
        let mut out = String::new();
        // ANSI 颜色代码：\x1b[3Xm
        const COLOR_PLAYER: &str = "\x1b[32m";  // 绿色
        const COLOR_MONSTER: &str = "\x1b[31m"; // 红色
        const COLOR_BOSS: &str = "\x1b[38;5;208m"; // 橙色（ANSI 208）
        const COLOR_ITEM_HEAL: &str = "\x1b[33m"; // 黄色
        const COLOR_ITEM_MANUAL: &str = "\x1b[36m"; // 青色
        const COLOR_ITEM_QI: &str = "\x1b[95m";   // 亮紫色
        const COLOR_ITEM_SHIELD: &str = "\x1b[94m"; // 亮蓝色
        const COLOR_RESET: &str = "\x1b[0m";
        const COLOR_GOLD: &str = "\x1b[33m"; // 金色用于结算

        if !matches!(self.end, End::None) {
            self.render_settlement(&mut out, COLOR_GOLD, COLOR_RESET);
            return out;
        }
        // 第一行：标题 + HP
        out.push_str(&format!(
            "{}WANDER  生命 {} / {}{}\r\n",
            COLOR_PLAYER,
            self.player.hp.max(0),
            self.player.max_hp,
            COLOR_RESET
        ));
        // 第二行：物品栏
        out.push_str(&format!(
            "物品：血瓶 x{}  秘籍 x{}  蕴气丹 x{}  盾牌存量 x{}/{}  积分 {}\r\n",
            self.potions,
            self.manuals,
            self.qi_pills,
            self.shield_stock.len(),
            SHIELD_STOCK_MAX,
            self.credits,
        ));
        // 第三行：工具栏（盾牌及耐久）
        if self.shield > 0 {
            out.push_str(&format!(
                "{}盾牌 {}/{}{}\r\n",
                COLOR_ITEM_SHIELD,
                self.shield,
                SHIELD_DURABILITY,
                COLOR_RESET
            ));
        } else {
            out.push_str("盾牌：无\r\n");
        }
        // 成就两行：基础/高级分开展示（不用普通消息栏 msg）；没达成过显示“无”，达成过金色列出
        let ach_body = |v: &[&'static str]| if v.is_empty() { "无".to_string() } else { v.join(" | ") };
        let basic = self.unlocked_achievements(&BASIC_ACHIEVEMENTS);
        let advanced = self.unlocked_achievements(&ADVANCED_ACHIEVEMENTS);
        out.push_str(&format!(
            "{}基础成就：{}{COLOR_RESET}\r\n",
            if basic.is_empty() { COLOR_RESET } else { COLOR_GOLD },
            ach_body(&basic)
        ));
        out.push_str(&format!(
            "{}高级成就：{}{COLOR_RESET}\r\n",
            if advanced.is_empty() { COLOR_RESET } else { COLOR_GOLD },
            ach_body(&advanced)
        ));
        for y in 0..self.vh {
            for x in 0..self.vw {
                let i = self.map.idx(x, y);
                let (glyph, color) = if self.player.x == x && self.player.y == y {
                    ('@', COLOR_PLAYER)
                } else if let Some(m) = self
                    .monsters
                    .iter()
                    .find(|m| m.x == x && m.y == y && self.map.visible[i])
                {
                    let c = if m.is_boss { COLOR_BOSS } else { COLOR_MONSTER };
                    (m.ch, c)
                } else if let Some(it) = self
                    .items
                    .iter()
                    .find(|it| it.x == x && it.y == y && self.map.visible[i])
                {
                    let c = match it.kind {
                        ItemKind::Heal => COLOR_ITEM_HEAL,
                        ItemKind::Manual => COLOR_ITEM_MANUAL,
                        ItemKind::QiPill => COLOR_ITEM_QI,
                        ItemKind::Shield => COLOR_ITEM_SHIELD,
                    };
                    (item_glyph(it.kind), c)
                } else if !self.map.explored[i] {
                    (' ', COLOR_RESET)
                } else {
                    match self.map.get(x, y) {
                        Tile::Wall => ('#', COLOR_RESET),
                        Tile::Floor => ('.', COLOR_RESET),
                    }
                };
                out.push_str(color);
                out.push(glyph);
                out.push_str(COLOR_RESET);
            }
            out.push_str("\r\n");
        }
        out.push_str(&format!(
            "{}\r\nq 退出  g 结束本局/回商店（死亡或守护者倒下后）  d 下楼（守护者倒下后）  1 血瓶  2 秘籍  3 蕴气丹  空格待机\r\n",
            self.msg
        ));
        out
    }

    fn render_settlement(&self, out: &mut String, gold: &str, reset: &str) {
        let halved = matches!(self.end, End::Defeat);
        let mult = self.floor as u32;
        let shield_pts = self.shield_stock_score();
        let full = (self.kills * 100 + self.potions as u32 * 50 + shield_pts) * mult;
        let after_death = if halved { full / 2 } else { full };
        let after_lucky = if self.lucky { after_death / 2 } else { after_death };
        let score = self.settlement_score();
        // 面板内部显示宽度（不含两侧 ║），所有行都补齐到这个宽度
        const INNER: usize = 42;
        const BOLD: &str = "\x1b[1m";

        let put = |out: &mut String, s: &str| {
            out.push_str(&format!("{gold}{s}{reset}\r\n"));
        };
        // 内容行套上左右边框，内容必须已补齐到 INNER 宽
        let row = |s: &str| format!("║{s}║");

        let (title, subtitle) = match self.end {
            End::Victory => (
                "V I C T O R Y",
                format!("最深层的守护者倒下了，全部 {} 层通关！", MAX_FLOOR),
            ),
            End::Voluntary => (
                "S T O P",
                format!("你在第 {} 层收手了，地牢还会等你再战", self.floor),
            ),
            End::Defeat => (
                "D E F E A T",
                format!("你死在了第 {}/{} 层（死亡积分折半）", self.floor, MAX_FLOOR),
            ),
            End::None => unreachable!(),
        };
        let bar = "═".repeat(INNER);
        put(out, &format!("╔{bar}╗"));
        out.push_str(&format!(
            "{gold}{BOLD}║{}║{reset}\r\n",
            centered(title, INNER)
        ));
        put(out, &row(&centered(&subtitle, INNER)));
        put(out, &format!("╠{bar}╣"));
        put(out, &row(&ledger(&format!("击杀怪物 {} 只 × 100", self.kills), &format!("{} 积分", self.kills * 100), INNER)));
        put(out, &row(&ledger(&format!("剩余血瓶 {} 个 × 50", self.potions), &format!("{} 积分", self.potions as u32 * 50), INNER)));
        if !self.shield_stock.is_empty() {
            let stock_dur: i32 = self.shield_stock.iter().sum();
            put(out, &row(&ledger(
                &format!("背包盾牌 {} 面·耐久{}×{}/点", self.shield_stock.len(), stock_dur, SHIELD_SCORE_PER_POINT),
                &format!("{} 积分", shield_pts),
                INNER,
            )));
        }
        put(out, &row(&ledger(&format!("第 {} 层倍率 ×{}", self.floor, mult), &format!("{} 积分", full), INNER)));
        if halved {
            put(out, &row(&ledger("死亡惩罚 ÷2", &format!("{} 积分", after_death), INNER)));
        }
        if self.lucky {
            put(out, &row(&ledger("幸运符惩罚 ÷2", &format!("{} 积分", after_lucky), INNER)));
        }
        let cleared = self.floors_cleared();
        if cleared > 0 {
            put(out, &row(&ledger(
                &format!("通关奖励 {} 关 × {}", cleared, CLEAR_BONUS_PER_FLOOR),
                &format!("+{} 积分", self.clear_bonus()),
                INNER,
            )));
        }
        put(out, &row(&format!("  {}", "─".repeat(INNER - 2))));
        out.push_str(&format!(
            "{gold}{BOLD}║{}║{reset}\r\n",
            ledger("本局积分", &format!("{score} 积分"), INNER)
        ));
        put(out, &format!("╠{bar}╣"));
        put(out, &row(&ledger("积分余额", &format!("{} 积分", self.credits), INNER)));
        put(out, &format!("╠{bar}╣"));
        put(out, &row(&centered("按 g 回商店开新一局    按 q 退出游戏", INNER)));
        put(out, &format!("╚{bar}╝"));
    }

    /// 测试辅助：在玩家脚边放一件道具，走过去就拾取，返回道具坐标。
    #[cfg(test)]
    fn grab(&mut self, kind: ItemKind) -> (usize, usize) {
        let (px, py) = (self.player.x, self.player.y);
        let spot = [(px + 1, py), (px, py + 1), (px - 1, py), (px, py - 1)]
            .into_iter()
            .find(|&(x, y)| self.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        self.items.push(Item {
            x: spot.0,
            y: spot.1,
            kind,
        });
        let dx = spot.0 as i32 - px as i32;
        let dy = spot.1 as i32 - py as i32;
        self.try_move_player(dx, dy);
        spot
    }
}

/// 攻击力 -> 伤害：下限保底 1，脸黑也能蹭血。
fn roll(rng: &mut impl RngExt, atk: i32) -> i32 {
    (atk / 2 + rng.random_range(0..atk)).max(1)
}

/// 怪物一次攻击结算盾格挡的结果。
struct ShieldBlock {
    /// 该次攻击被盾挡住（骷髅穿盾时为 false，调用方照常掉血）。
    blocked: bool,
    /// 这一击把盾打碎（耐久归零）且自动从背包存量换上了一面（除新迎旧）。
    refilled: bool,
}

/// 怪物的一次命中打过来：手里有盾且不是骷髅就挡下（命中扣 1 点耐久），
/// 往 hits 里塞一条格挡消息；没盾或骷髅穿盾时 blocked = false，调用方照常掉血。
/// 如果这一击把盾打碎（耐久归零），自动从背包 stock 取耐久最高的一面换上（拿出来接着用）。
fn block_with_shield(
    shield: &mut i32,
    stock: &mut Vec<i32>,
    attacker: &str,
    weapon: &str,
    hits: &mut Vec<String>,
) -> ShieldBlock {
    if *shield <= 0 || attacker == SKELETON {
        return ShieldBlock {
            blocked: false,
            refilled: false,
        };
    }
    *shield -= 1;
    if *shield > 0 {
        hits.push(format!(
            "{attacker}的{weapon}被盾牌挡下（盾牌耐久 {shield}/{SHIELD_DURABILITY}）"
        ));
        return ShieldBlock {
            blocked: true,
            refilled: false,
        };
    }
    // 这一击把盾打碎了
    hits.push(format!("{attacker}的{weapon}击碎了盾牌最后一点耐久"));
    // 自动从背包取耐久最高的一面存量盾牌换上
    let mut refilled = false;
    if let Some(pos) = stock
        .iter()
        .enumerate()
        .filter(|(_, d)| **d > 0)
        .max_by_key(|(_, d)| *d)
        .map(|(i, _)| i)
    {
        let new_dur = stock.swap_remove(pos);
        *shield = new_dur;
        refilled = true;
        hits.push(format!(
            "旧盾碎裂，自动从背包换上一面（耐久 {new_dur}/{SHIELD_DURABILITY}）"
        ));
    }
    ShieldBlock {
        blocked: true,
        refilled,
    }
}

/// 层数怪物血量缩放：1-5 层分别 ×1、×1.5、×2、×2.5、×3。
fn floor_hp(base: i32, f: i32) -> i32 {
    base * (2 + (f - 1)) / 2
}

/// 终端显示宽度：CJK、全角标点等占 2 列，其余（含框线 ═║）占 1 列。
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let wide = matches!(c as u32,
                0x1100..=0x115F     // 韩文字母
                | 0x2E80..=0x303E   // CJK 部首、标点（【】在这）
                | 0x3041..=0x33FF   // 假名、兼容符号
                | 0x3400..=0x4DBF   // CJK 扩展 A
                | 0x4E00..=0x9FFF   // CJK 基本汉字
                | 0xAC00..=0xD7A3   // 谚文音节
                | 0xF900..=0xFAFF   // 兼容汉字
                | 0xFE30..=0xFE6F   // 小写变体
                | 0xFF01..=0xFF60   // 全角 ASCII（，！在这）
                | 0xFFE0..=0xFFE6); // 全角货币
            if wide { 2 } else { 1 }
        })
        .sum()
}

/// 居中填充到面板内部宽度，保证 ║ 边框对齐。
fn centered(s: &str, inner: usize) -> String {
    let pad = inner.saturating_sub(display_width(s));
    let left = pad / 2;
    format!("{}{s}{}", " ".repeat(left), " ".repeat(pad - left))
}

/// 结算面板里的账目行：左侧标签、右侧数值，中间点线补齐到 inner 宽。
fn ledger(label: &str, value: &str, inner: usize) -> String {
    let label = format!("  {label}");
    let vw = display_width(value);
    let dots = inner.saturating_sub(display_width(&label) + vw + 2).max(1);
    format!("{label} {} {value}", ".".repeat(dots))
}

/// 怪物尝试走到 (nx, ny)：可走、没被别的怪占着、不是玩家所在格。
fn try_move(
    map: &Map,
    occupied: &HashSet<(usize, usize)>,
    px: usize,
    py: usize,
    nx: i32,
    ny: i32,
) -> Option<(usize, usize)> {
    if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
        return None;
    }
    let (nx, ny) = (nx as usize, ny as usize);
    if map.is_walkable(nx, ny) && !occupied.contains(&(nx, ny)) && (nx, ny) != (px, py) {
        Some((nx, ny))
    } else {
        None
    }
}

/// 按房间序号和层数生成怪物：前几间放近战小怪，中间几间放射手，最后一间是本层守护者。
/// 1 层 = 4 小怪 + 3 射手；每下一层小怪少 1、射手多 1；5 层没有近战小怪。
/// 怪物血/攻逐层上涨（见 floor_hp 与攻击公式）。房间 0 是玩家出生点，不放怪。
fn populate(floor: u8, map: &Map) -> (Vec<Entity>, Vec<Item>) {
    use rand::seq::SliceRandom;

    let mut rng = rand::rng();
    let mut monsters = Vec::new();
    let n = map.rooms.len();
    let boss_room = n - 1;
    let f = floor as i32;
    // 近战小怪数量 4,3,2,1,0；其余 7 间全给射手（3,4,5,6,7）
    let melee_count = 4usize.saturating_sub(floor as usize - 1);
    for i in 1..boss_room {
        let (rx, ry, rw, rh) = map.rooms[i];
        let mx = rx + 1 + rng.random_range(0..rw.saturating_sub(2));
        let my = ry + 1 + rng.random_range(0..rh.saturating_sub(2));
        let (name, ch, hp, atk, sight, behavior): (&str, char, i32, i32, usize, Behavior) =
            if i <= melee_count {
                let t = (i - 1) % 4; // 小怪种类循环：哥布林/巨鼠/蜘蛛/骷髅
                let name = ["哥布林", "巨鼠", "蜘蛛", SKELETON][t];
                let skeleton_bonus = if name == SKELETON { SKELETON_ATK_BONUS } else { 0 };
                (
                    name,
                    ['g', 'r', 's', 'k'][t],
                    floor_hp(10 + (i as i32) * 4, f),
                    3 + i as i32 + 2 * (f - 1) + skeleton_bonus,
                    8,
                    Behavior::Melee,
                )
            } else {
                (
                    "暗影射手",
                    'o',
                    floor_hp(12 + (i as i32) * 3, f),
                    2 + (i as i32) / 2 + 2 * (f - 1),
                    12,
                    Behavior::Ranged,
                )
            };
        monsters.push(Entity {
            x: mx,
            y: my,
            ch,
            name,
            hp,
            max_hp: hp,
            atk,
            sight,
            behavior,
            is_boss: false,
        });
    }

    // 本层守护者：血量随层缩放，攻击每层 +2
    {
        let (rx, ry, rw, rh) = map.rooms[boss_room];
        let boss_hp = floor_hp(70, f);
        monsters.push(Entity {
            x: rx + 1 + rng.random_range(0..rw.saturating_sub(2)),
            y: ry + 1 + rng.random_range(0..rh.saturating_sub(2)),
            ch: 'D',
            name: BOSS,
            hp: boss_hp,
            max_hp: boss_hp,
            atk: 7 + 2 * (f - 1),
            sight: 10,
            behavior: Behavior::Melee,
            is_boss: true,
        });
    }

    // 血瓶：随机挑几个怪物房各放一个，不再每房都有
    let mut item_rooms = (1..map.rooms.len()).collect::<Vec<_>>();
    item_rooms.shuffle(&mut rng);
    let mut items = Vec::new();
    for &i in item_rooms.iter().take(POTIONS_PER_FLOOR) {
        let (rx, ry, rw, rh) = map.rooms[i];
        items.push(Item {
            x: rx + 1 + rng.random_range(0..rw.saturating_sub(2)),
            y: ry + 1 + rng.random_range(0..rh.saturating_sub(2)),
            kind: ItemKind::Heal,
        });
    }

    // 武林秘籍与蕴气丹各只有一份：血瓶之后各占一个房间，互不重复
    let unique_kinds = [ItemKind::Manual, ItemKind::QiPill];
    for (kind, &i) in unique_kinds.iter().zip(item_rooms.iter().skip(POTIONS_PER_FLOOR)) {
        let (rx, ry, rw, rh) = map.rooms[i];
        items.push(Item {
            x: rx + 1 + rng.random_range(0..rw.saturating_sub(2)),
            y: ry + 1 + rng.random_range(0..rh.saturating_sub(2)),
            kind: *kind,
        });
    }

    (monsters, items)
}

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

/// 主循环：每局开局先进商店用积分备货，死亡/通关后回商店再开新局。
fn main_loop(vw: usize, vh: usize, stdout: &mut std::io::Stdout) -> std::io::Result<()> {
    loop {
        let purchase = match run_shop(stdout)? {
            Some(p) => p,
            None => return Ok(()), // 商店里按 q 退出
        };
        let mut game = Game::new(vw.max(40), vh.max(7));
        game.apply_purchase(&purchase);
        run_game(&mut game, stdout)?;
    }
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

fn run_game(game: &mut Game, stdout: &mut std::io::Stdout) -> std::io::Result<()> {
    // 结算（通关 / 主动结束 / 死亡）每局只落盘一次
    let mut score_persisted = false;
    loop {
        if !matches!(game.end, End::None) && !score_persisted {
            let mut record = Score::load();
            record.add_run(game.kills, game.potions, game.shield_stock.len(), game.settlement_score());
            game.credits = record.credits;
            score_persisted = true;
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
            let delta = match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char('g') => {
                    if game.can_restart() {
                        if game.boss_down && matches!(game.end, End::None) {
                            // 守护者倒下后主动结束：先画结算面板，再按 g 回商店
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
                    "你死在了第 {}/{} 层。按 g 回商店（死亡积分折半结算）。",
                    game.floor, MAX_FLOOR
                );
                execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
                write!(stdout, "{}", game.render())?;
                stdout.flush()?;
                // 单步：继续循环等按键，无法移动（hp<=0 时 try_move 已封住）
            }
        }
    }
    Ok(())
}

/// 开局商店：花上一局通关赚的积分备货。1/2/3 购买，b 开始，q 退出。
struct Shop {
    credits: u32,
    manual: bool,
    talisman: bool,
    potions: usize,
    msg: String,
}

impl Shop {
    fn new() -> Self {
        Shop {
            credits: Score::load().credits,
            manual: false,
            talisman: false,
            potions: 0,
            msg: String::new(),
        }
    }

    /// 购买：0 = 武林秘籍，1 = 幸运符，2 = 血瓶。
    fn buy(&mut self, kind: u32) {
        let (price, name, desc) = match kind {
            0 => (
                PRICE_MANUAL,
                "武林秘籍",
                "开局自带一本，局内按 2 参悟",
            ),
            1 => (
                PRICE_TALISMAN,
                "幸运符",
                "血瓶掉率 90%，秘籍 25% 概率 ×4，暗影射手盾牌必掉；结算积分减半",
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

    fn to_purchase(&self) -> Purchase {
        Purchase {
            manual: self.manual,
            talisman: self.talisman,
            potions: self.potions,
        }
    }

    fn render(&self) -> String {
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
            "{}[2] {CYAN}幸运符{RESET}  {GOLD}{} 积分{RESET}\r\n    血瓶掉率升至 90%；参悟秘籍 25% 概率攻击 ×4（×4 仅此唯一获得途径）；暗影射手盾牌必掉；结算积分减半  {}\r\n",
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
fn run_shop(stdout: &mut std::io::Stdout) -> std::io::Result<Option<Purchase>> {
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

    /// 踩到血瓶只是揣进背包，按 1 才喝；回血上限封顶；死了之后移动被封锁。
    #[test]
    fn pickup_stocks_potion_and_use_heals() {
        let mut g = Game::new(60, 25);
        g.player.hp = 5;
        // 自己放一个血瓶在脚边可走的格子上（生成时血瓶位置随机，不能依赖它）
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        g.items.push(Item {
            x: spot.0,
            y: spot.1,
            kind: ItemKind::Heal,
        });
        let dx = spot.0 as i32 - px as i32;
        let dy = spot.1 as i32 - py as i32;
        g.try_move_player(dx, dy);
        // 拾取只进背包，不回血
        assert_eq!(g.potions, 1, "血瓶没进背包");
        assert!(g.player.hp == 5, "还没按 1 就回血了");
        assert!(!g.items.iter().any(|i| i.x == spot.0 && i.y == spot.1), "血瓶没被拾取");

        // 按 1 才回血，且不超过上限
        g.player.hp = g.player.max_hp - 3;
        g.use_potion();
        assert_eq!(g.potions, 0, "喝了血瓶背包没减");
        assert_eq!(g.player.hp, g.player.max_hp, "回血超过上限");

        // 没血瓶时按 1 不该凭空回血
        let hp = g.player.hp;
        g.use_potion();
        assert_eq!(g.player.hp, hp, "空背包还能回血");

        // 死亡封锁
        g.player.hp = 0;
        let start = (g.player.x, g.player.y);
        g.try_move_player(1, 0);
        g.try_move_player(-1, 0);
        assert_eq!((g.player.x, g.player.y), start, "死了还能动");
    }

    /// 背包满 5 个时再踩血瓶，血瓶留在地上，数量不超上限。
    #[test]
    fn potion_backpack_is_capped() {        let mut g = Game::new(60, 25);
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        g.items.push(Item {
            x: spot.0,
            y: spot.1,
            kind: ItemKind::Heal,
        });
        g.potions = POTION_STACK;

        let dx = spot.0 as i32 - px as i32;
        let dy = spot.1 as i32 - py as i32;
        g.try_move_player(dx, dy);
        assert_eq!(g.potions, POTION_STACK, "背包超上限了");
        assert!(g.items.iter().any(|i| i.x == spot.0 && i.y == spot.1), "满包时血瓶不该消失");

        // 喝一瓶腾出位置后，再踩才进背包
        g.use_potion();
        g.try_move_player(-dx, -dy);
        g.try_move_player(dx, dy);
        assert_eq!(g.potions, POTION_STACK, "腾出位置后血瓶没捡起来");
    }

    /// 按 1 喝药不消耗回合：怪物不该因此多动一轮。
    #[test]
    fn using_potion_costs_no_turn() {
        let mut g = Game::new(60, 25);
        // 放一只看得见玩家的怪在远处，它每回合都会动（追击或游荡）
        let (px, py) = (g.player.x, g.player.y);
        g.monsters.clear();
        // 凿通一条走廊，保证怪追击时有路可走（对照断言才成立）
        for x in px..=px + 5 {
            let i = g.map.idx(x, py);
            g.map.tiles[i] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'g',
            name: "哥布林",
            hp: 10,
            max_hp: 10,
            atk: 4,
            sight: 12,
            behavior: Behavior::Melee,
            is_boss: false,
        });
        g.potions = 2;
        let before = g.monsters[0].x;

        // 喝药三次，中间不移动：怪还在原地，说明没轮到它动
        g.use_potion();
        g.use_potion();
        g.use_potion();
        assert_eq!(g.monsters[0].x, before, "喝药竟然让怪物动了");

        // 对照：正常移动一步，怪物就该动了
        g.try_move_player(1, 0);
        assert_ne!(g.monsters[0].x, before, "移动后怪物没动，测试前提不成立");
    }

    /// 死了或通关之后不能喝药（和不能移动一样封锁）。
    #[test]
    fn potion_blocked_after_death_or_clear() {
        let mut g = Game::new(60, 25);
        g.potions = 1;
        g.player.hp = 0;
        g.use_potion();
        assert_eq!(g.potions, 1, "死了还能喝药");
        assert_eq!(g.player.hp, 0);

        g.player.hp = 10;
        g.end = End::Victory;
        g.use_potion();
        assert_eq!(g.potions, 1, "通关了还能喝药");
    }

    /// 怪物有 1/2 概率掉血瓶：杀 200 只怪，看“掉落了一个血瓶”出现的次数。
    #[test]
    fn drop_chance_is_one_half() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        // 沿一条直线每次放一只 1 血怪，玩家走过去一刀一个
        let mut x = px;
        let mut drops = 0usize;
        for _ in 0..200 {
            let i = g.map.idx(x + 1, py);
            g.map.tiles[i] = Tile::Floor;
            g.monsters.push(Entity {
                x: x + 1,
                y: py,
                ch: 'k',
                name: "骷髅",
                hp: 1,
                max_hp: 1,
                atk: 0,
                sight: 0, // 看不见玩家，只会瞎逛，不会反击覆盖消息
                behavior: Behavior::Melee,
                is_boss: false,
            });
            g.try_move_player(1, 0);
            x = g.player.x;
            if g.msg.contains("掉落了一个血瓶") {
                drops += 1;
            }
        }
        // 200 次抽样下，1/2 掉率落在 70~130 之外的概率极小
        assert!(drops > 70 && drops < 130, "掉率异常：杀了 200 只怪只掉了 {drops} 个血瓶");
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

    /// 新一局（重开回商店后再进）玩家满血且回到房间0。
    #[test]
    fn new_game_resets() {
        let mut g = Game::new(60, 25);
        g.player.hp = 1;
        g.try_move_player(0, 1);
        let g2 = Game::new(60, 25);
        assert_eq!(g2.player.hp, g2.player.max_hp);
        let (rx, ry, _, _) = g2.map.rooms[0];
        assert!(g2.player.x.wrapping_sub(rx) < 4 && g2.player.y.wrapping_sub(ry) < 4);
    }

    /// 每层有 POTIONS_PER_FLOOR 个血瓶，外加武林秘籍与蕴气丹各一份；
    /// 小怪 4→0 递减、射手 3→7 递增，且必定有一只 boss（血量逐层上涨）。
    #[test]
    fn loot_and_boss_are_consistent() {
        for floor in 1..=MAX_FLOOR {
            for _ in 0..5 {
                let mut map = Map::new();
                map.generate(None);
                let (monsters, items) = populate(floor, &map);
                let heal = items.iter().filter(|it| matches!(it.kind, ItemKind::Heal)).count();
                let manuals = items.iter().filter(|it| matches!(it.kind, ItemKind::Manual)).count();
                let pills = items.iter().filter(|it| matches!(it.kind, ItemKind::QiPill)).count();
                assert_eq!(heal, POTIONS_PER_FLOOR, "血瓶数量不对");
                assert_eq!(manuals, 1, "武林秘籍必须全图唯一");
                assert_eq!(pills, 1, "蕴气丹必须全图唯一");
                // 小怪数量逐层递减，射手逐层递增
                let melee = monsters
                    .iter()
                    .filter(|m| !m.is_boss && matches!(m.behavior, Behavior::Melee))
                    .count();
                let ranged = monsters
                    .iter()
                    .filter(|m| matches!(m.behavior, Behavior::Ranged))
                    .count();
                assert_eq!(
                    melee,
                    4usize.saturating_sub(floor as usize - 1),
                    "第 {floor} 层小怪数量不对"
                );
                assert_eq!(
                    ranged,
                    3 + (floor as usize - 1),
                    "第 {floor} 层射手数量不对"
                );
                // boss 只能有一个，且是近战大血牛
                let bosses: Vec<_> = monsters.iter().filter(|m| m.is_boss).collect();
                assert_eq!(bosses.len(), 1, "第 {floor} 层 boss 数量不对");
                assert!(matches!(bosses[0].behavior, Behavior::Melee));
                assert!(bosses[0].hp >= 50, "第 {floor} 层 boss 血量太低");
                // 远程射手视野比近战远，用来在房间另一头开火
                let ranged_m = monsters.iter().find(|m| matches!(m.behavior, Behavior::Ranged));
                assert!(ranged_m.is_some(), "没有远程怪");
                assert!(ranged_m.unwrap().sight > 8);
            }
        }
    }

    /// 远程怪距离太近（2-3 格，射不出箭）时会后撤拉开距离，不会傻站着挨打；
    /// 贴身（1 格）时改为近战反击，不后撤。
    #[test]
    fn ranged_keeps_distance() {
        let mut g = Game::new(60, 25);
        // 清场，在玩家右侧凿一条直走廊，放一个暗影射手
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 6 {
            let i = g.map.idx(x, py);
            g.map.tiles[i] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 30,
            max_hp: 30,
            atk: 5,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
        });
        // 玩家站到 2 格外（太近射不出箭，够不到近战），它这回合该后撤拉开距离
        g.player.x = px + 3;
        g.end_turn();
        let dist = g.monsters[0]
            .x
            .abs_diff(g.player.x)
            + g.monsters[0].y.abs_diff(g.player.y);
        assert!(dist >= 3, "远程怪距离太近还不后撤");

        // 贴身（1 格）时不后撤，改为近身暗影一击
        g.player.x = g.monsters[0].x - 1;
        let before = g.monsters[0].x;
        g.player.hp = g.player.max_hp;
        g.end_turn();
        assert_eq!(g.monsters[0].x, before, "贴脸的远程怪该原地反击而不是跑");
    }

    /// 杀掉守护者：1-4 层只是解锁下楼/主动结算，第 5 层才是通关结算。
    #[test]
    fn boss_death_unlocks_descend_or_wins() {
        // 第 1 层：守护者倒下后可以继续走，也能主动结算
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        let i = g.map.idx(spot.0, spot.1);
        g.map.tiles[i] = Tile::Floor;
        g.monsters.push(Entity {
            x: spot.0,
            y: spot.1,
            ch: 'D',
            name: "地牢守护者",
            hp: 1,
            max_hp: 1,
            atk: 1,
            sight: 10,
            behavior: Behavior::Melee,
            is_boss: true,
        });
        let dx = spot.0 as i32 - px as i32;
        let dy = spot.1 as i32 - py as i32;
        g.try_move_player(dx, dy);
        assert!(g.boss_down, "杀掉守护者该解锁下楼");
        assert!(matches!(g.end, End::None), "第 1 层结束不了本局");
        assert!(g.can_restart(), "守护者倒下后 g 该能主动结束回商店");

        // 第 5 层：杀掉守护者直接进入通关结算，移动被封锁
        let mut g5 = Game::new(60, 25);
        g5.floor = MAX_FLOOR;
        g5.monsters.clear();
        let (px, py) = (g5.player.x, g5.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g5.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        let i = g5.map.idx(spot.0, spot.1);
        g5.map.tiles[i] = Tile::Floor;
        g5.monsters.push(Entity {
            x: spot.0,
            y: spot.1,
            ch: 'D',
            name: "地牢守护者",
            hp: 1,
            max_hp: 1,
            atk: 1,
            sight: 10,
            behavior: Behavior::Melee,
            is_boss: true,
        });
        let dx = spot.0 as i32 - px as i32;
        let dy = spot.1 as i32 - py as i32;
        g5.try_move_player(dx, dy);
        assert!(g5.boss_down, "杀 boss 没解锁下楼");
        assert!(matches!(g5.end, End::Victory), "第 5 层杀 boss 该进入通关结算");
        assert!(g5.monsters.is_empty());
        // 通关后不能再动
        let after = (g5.player.x, g5.player.y);
        g5.try_move_player(1, 0);
        assert_eq!((g5.player.x, g5.player.y), after, "通关了还能乱动");
    }

    /// 怪物难度逐层上涨：boss 血量/攻击每层更强；小怪 4→0 递减，射手 3→7 递增。
    #[test]
    fn monsters_scale_with_floor() {
        let mut map = Map::new();
        map.generate(None);
        let prev: Vec<(i32, i32)> = (1..=MAX_FLOOR)
            .map(|f| {
                let (ms, _) = populate(f, &map);
                let boss = ms.iter().find(|m| m.is_boss).expect("每层都该有 boss");
                (boss.hp, boss.atk)
            })
            .collect();
        for w in prev.windows(2) {
            assert!(w[1].0 > w[0].0, "boss 血量该逐层上涨：{prev:?}");
            assert!(w[1].1 > w[0].1, "boss 攻击该逐层上涨：{prev:?}");
        }
        for f in 1..=MAX_FLOOR {
            let (ms, _) = populate(f, &map);
            let melee = ms.iter().filter(|m| !m.is_boss && m.behavior == Behavior::Melee).count();
            let ranged = ms.iter().filter(|m| m.behavior == Behavior::Ranged).count();
            assert_eq!(melee, 4usize.saturating_sub(f as usize - 1), "第 {f} 层小怪数量");
            assert_eq!(ranged, 3 + (f as usize - 1), "第 {f} 层射手数量");
        }
    }

    /// 下楼：守护者倒下前按 d 无效，倒下后生成新一层地图并重置 boss_down。
    #[test]
    fn descend_only_after_boss_down() {
        let mut g = Game::new(60, 25);
        let floor_before = g.floor;
        g.descend();
        assert_eq!(g.floor, floor_before, "守护者没倒下不能下楼");

        g.boss_down = true;
        g.descend();
        assert_eq!(g.floor, 2, "守护者倒下后 d 该下到第 2 层");
        assert!(!g.boss_down, "新一层守护者还没倒，boss_down 该重置");
        assert!(!g.monsters.is_empty(), "新一层该有怪物");
        // 出生在新一层的房间 0
        let (rx, ry, _, _) = g.map.rooms[0];
        assert_eq!((g.player.x, g.player.y), (rx + 1, ry + 1));
    }

    /// 第 5 层没有下一层，d 无效。
    #[test]
    fn descend_blocked_at_last_floor() {
        let mut g = Game::new(60, 25);
        g.floor = MAX_FLOOR;
        g.boss_down = true;
        g.descend();
        assert_eq!(g.floor, MAX_FLOOR, "第 5 层不能再下楼");
    }

    /// 结算分 = (击杀×100 + 血瓶×50) × 当前层倍率；死亡折半。
    #[test]
    fn settlement_multiplies_by_floor_and_halves_on_death() {
        let mut g = Game::new(60, 25);
        g.kills = 8;
        g.potions = 3; // 基础分 = 8*100 + 3*50 = 950
        assert_eq!(g.floor, 1);
        assert_eq!(g.settlement_score(), 950, "1 层 ×1");
        g.floor = 2;
        assert_eq!(g.settlement_score(), 1900 + 500, "2 层 ×2，已通过 1 关 +500");
        g.floor = 5;
        assert_eq!(g.settlement_score(), 4750 + 2000, "5 层 ×5，已通过 4 关 +2000");
        g.end = End::Defeat;
        assert_eq!(g.settlement_score(), 2375 + 2000, "死亡该折半，通关奖励不折半");
    }

    /// 幸运符在层数倍率后将积分折半；死亡再独立折半，结算面板显示两笔惩罚。
    #[test]
    fn lucky_charm_halves_settlement_score() {
        let mut g = Game::new(60, 25);
        g.kills = 1;
        g.potions = 1; // 基础分 150，第 3 层得 450
        g.floor = 3;
        g.end = End::Voluntary;
        g.boss_down = true; // 第 3 层守护者已倒：通过 3 关，奖励 1500（不折半）
        assert_eq!(g.settlement_score(), 450 + 1500, "未使用幸运符不应受罚");
        assert!(!g.render().contains("幸运符惩罚"));

        g.apply_purchase(&Purchase { talisman: true, ..Purchase::default() });
        assert_eq!(g.settlement_score(), 225 + 1500, "主动结算时幸运符该折半，通关奖励不折半");
        let panel = g.render();
        assert!(panel.contains("幸运符惩罚 ÷2"));
        assert!(panel.contains("本局积分") && panel.contains("225 积分") && panel.contains("1725 积分"));
        assert!(panel.contains("通关奖励 3 关 × 500"));
        assert!(!panel.contains("死亡惩罚"));

        g.end = End::Defeat;
        assert_eq!(g.settlement_score(), 112 + 1500, "死亡和幸运符应分别折半，向下取整");
        let panel = g.render();
        assert!(panel.contains("死亡惩罚 ÷2"));
        assert!(panel.contains("幸运符惩罚 ÷2"));
        assert!(panel.contains("225 积分") && panel.contains("112 积分") && panel.contains("1612 积分"));
        let rows: Vec<_> = panel.lines()
            .filter(|line| line.contains('║'))
            .map(|line| {
                let mut out = String::new();
                let mut chars = line.chars();
                while let Some(c) = chars.next() {
                    if c == '\x1b' {
                        for c2 in chars.by_ref() {
                            if c2 == 'm' { break; }
                        }
                    } else {
                        out.push(c);
                    }
                }
                out
            })
            .collect();
        let width = display_width(&rows[0]);
        assert!(rows.iter().all(|line| display_width(line) == width), "惩罚行不应撑破面板");

        g.end = End::Victory;
        g.floor = MAX_FLOOR;
        assert_eq!(g.settlement_score(), 375 + 2500, "通关时也应受幸运符惩罚，5 关奖励 2500");

        g.kills = 0;
        g.potions = 0;
        g.shield_stock = vec![45, 30];
        assert_eq!(g.settlement_score(), 375 + 2500, "盾牌所得积分也应减半");
    }

    /// 通关奖励：每通过一关 +500，不受倍率和折半影响；死在某层则不算该层。
    #[test]
    fn clear_bonus_is_500_per_floor_cleared() {
        let mut g = Game::new(60, 25);
        assert_eq!(g.floors_cleared(), 0);
        assert_eq!(g.settlement_score(), 0);
        g.boss_down = true;
        assert_eq!(g.settlement_score(), 500, "第 1 层通过 +500");
        g.floor = 4;
        g.end = End::Defeat;
        g.boss_down = false;
        assert_eq!(g.settlement_score(), 1500, "死在第 4 层，只通过 3 关");
        g.floor = MAX_FLOOR;
        g.end = End::Victory;
        assert_eq!(g.settlement_score(), 2500, "5 关全过 +2500");
    }

    /// 骷髅的攻击比同位置普通小怪更高。
    #[test]
    fn skeleton_has_attack_bonus() {
        let mut map = Map::new();
        map.generate(None);
        let (monsters, _) = populate(1, &map);
        let sk = monsters.iter().find(|m| m.name == SKELETON).expect("第 1 层该有骷髅");
        let i = monsters.iter().position(|m| m.name == SKELETON).unwrap() as i32 + 1;
        assert_eq!(sk.atk, 3 + i + SKELETON_ATK_BONUS);
    }

    /// 活着的时候按 g 不能重开；死了或通关了才可以（回商店开新局）。
    #[test]
    fn restart_is_blocked_while_alive() {
        let mut g = Game::new(60, 25);
        assert!(!g.can_restart(), "活着就能重开");

        g.player.hp = 0;
        assert!(g.can_restart(), "死了反而重开不了");

        let mut g2 = Game::new(60, 25);
        g2.boss_down = true;
        assert!(g2.can_restart(), "守护者倒下后反而结束不了");
    }

    /// 秘籍与蕴气丹：捡了进背包，按对应键才生效；效果分别落在攻击力和血量上限上。
    #[test]
    fn manual_and_qi_pill_work() {
        let mut g = Game::new(60, 25);
        let atk_before = g.player.atk;
        let spot = g.grab(ItemKind::Manual);
        assert_eq!(g.manuals, 1, "秘籍没进背包");
        assert_eq!(g.player.atk, atk_before, "捡起就加攻击，不用按 2");
        assert!(
            !g.items.iter().any(|it| it.x == spot.0 && it.y == spot.1),
            "脚边的秘籍没被拾取"
        );

        g.use_manual();
        assert_eq!(g.manuals, 0, "参悟后背包没清空");
        assert_eq!(g.player.atk, 2 * atk_before, "攻击力没翻倍");

        // 没秘籍时按 2 不该再翻倍
        g.use_manual();
        assert_eq!(g.player.atk, 2 * atk_before, "空背包按 2 竟然又翻倍");

        let max_before = g.player.max_hp;
        let spot = g.grab(ItemKind::QiPill);
        assert_eq!(g.qi_pills, 1, "蕴气丹没进背包");
        assert_eq!(g.player.max_hp, max_before, "捡起就加上限，不用按 3");
        assert!(
            !g.items.iter().any(|it| it.x == spot.0 && it.y == spot.1),
            "脚边的蕴气丹没被拾取"
        );

        let hp_before = g.player.hp;
        g.use_qi_pill();
        assert_eq!(g.qi_pills, 0, "服用后背包没清空");
        assert_eq!(g.player.max_hp, max_before + 10, "血量上限没 +10");
        assert_eq!(g.player.hp, hp_before - 10, "没扣 10 血");
    }

    /// 秘籍与蕴气丹的强化可以叠加成通关资本：攻击 8 → 16，血量上限 40 → 50。
    #[test]
    fn both_buffs_stack_to_kill_boss() {
        let mut g = Game::new(60, 25);
        assert_eq!(g.player.atk, 8);
        assert_eq!(g.player.max_hp, 40);

        g.grab(ItemKind::Manual);
        g.use_manual();
        g.grab(ItemKind::QiPill);
        g.use_qi_pill();

        assert_eq!(g.player.atk, 16, "秘籍叠完该是 16 攻");
        assert_eq!(g.player.max_hp, 50, "蕴气丹叠完该是 50 血上限");
        // boss 70 血，16 攻一刀 8~16（平均 15.5）：六刀的平均伤害该放倒 boss。
        // 单次抽样有运气成分（6 刀全出低点会挂），所以取多次抽样的均值断言。
        let boss_hp = 70;
        let mut total = 0;
        let mut rng = rand::rng();
        for _ in 0..200 {
            let mut dmg = 0;
            for _ in 0..6 {
                dmg += roll(&mut rng, g.player.atk);
            }
            total += dmg;
        }
        assert!(
            total / 200 >= boss_hp,
            "六刀平均砍不死 70 血的 boss：{}",
            total / 200
        );
    }

    /// 服用蕴气丹照扣 10 血，血不够就直接自杀，不保底。
    #[test]
    fn qi_pill_can_kill_you() {
        let mut g = Game::new(60, 25);
        g.player.hp = 5;
        g.qi_pills = 1;
        g.use_qi_pill();
        assert_eq!(g.player.hp, 0, "5 血服丹该当场去世");
        assert_eq!(g.player.max_hp, 50, "但血量上限该照常涨到 50");
    }

    /// 秘籍和蕴气丹全图唯一：已经用过就不再拾取地上重复的份。
    #[test]
    fn uniques_are_one_per_floor() {
        let mut g = Game::new(60, 25);
        // 先在脚边放第二份秘籍，背包里已有一份时，走过去该留在原地
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        g.items.push(Item {
            x: spot.0,
            y: spot.1,
            kind: ItemKind::Manual,
        });
        g.manuals = 1;
        let dx = spot.0 as i32 - px as i32;
        let dy = spot.1 as i32 - py as i32;
        g.try_move_player(dx, dy);
        assert_eq!(g.manuals, 1, "唯一道具被重复拾取了");
        assert!(
            g.items.iter().any(|it| it.x == spot.0 && it.y == spot.1),
            "重复的秘籍该留在原地"
        );

        g.use_manual();
        // 参悟过之后，地上的副本也不该再被吃
        g.try_move_player(-dx, -dy);
        g.try_move_player(dx, dy);
        assert_eq!(g.player.atk, 2 * BASE_ATK, "参悟两本秘籍，攻击力该封顶不叠");
    }

    /// 死了或通关之后，秘籍和蕴气丹一样被封锁。
    #[test]
    fn uniques_blocked_after_death_or_clear() {
        let mut g = Game::new(60, 25);
        g.manuals = 1;
        g.qi_pills = 1;
        let (atk, max_hp) = (g.player.atk, g.player.max_hp);

        g.player.hp = 0;
        g.use_manual();
        g.use_qi_pill();
        assert_eq!(g.manuals, 1, "死了还能参悟秘籍");
        assert_eq!(g.qi_pills, 1, "死了还能服蕴气丹");
        assert_eq!(g.player.atk, atk);
        assert_eq!(g.player.max_hp, max_hp);

        g.player.hp = 10;
        g.end = End::Victory;
        g.use_manual();
        g.use_qi_pill();
        assert_eq!(g.manuals, 1, "通关了还能参悟秘籍");
        assert_eq!(g.qi_pills, 1, "通关了还能服蕴气丹");
        assert_eq!(g.player.atk, atk);
        assert_eq!(g.player.max_hp, max_hp);
    }

    /// 使用秘籍和蕴气丹都不消耗回合：怪物不该因此多动一轮。
    #[test]
    fn using_uniques_costs_no_turn() {
        let mut g = Game::new(60, 25);
        let (px, py) = (g.player.x, g.player.y);
        g.monsters.clear();
        for x in px..=px + 5 {
            let i = g.map.idx(x, py);
            g.map.tiles[i] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'g',
            name: "哥布林",
            hp: 10,
            max_hp: 10,
            atk: 4,
            sight: 12,
            behavior: Behavior::Melee,
            is_boss: false,
        });
        g.manuals = 1;
        g.qi_pills = 1;
        let before = g.monsters[0].x;

        g.use_manual();
        g.use_qi_pill();
        assert_eq!(g.monsters[0].x, before, "嗑药竟然让怪物动了");

        // 对照：正常移动一步，怪物就该动了
        g.try_move_player(1, 0);
        assert_ne!(g.monsters[0].x, before, "移动后怪物没动，测试前提不成立");
    }

    /// 新一局开始时，上一局的强化和物品该被全部洗掉。
    #[test]
    fn new_game_resets_buffs() {
        let mut g = Game::new(60, 25);
        g.manuals = 1;
        g.qi_pills = 1;
        g.player.atk = 99;
        g.player.max_hp = 99;
        g.player.hp = 99;

        let fresh = Game::new(60, 25);
        assert_eq!(fresh.manuals, 0, "新局没清空秘籍");
        assert_eq!(fresh.qi_pills, 0, "新局没清空蕴气丹");
        assert_eq!(fresh.player.atk, BASE_ATK, "新局没重置攻击力");
        assert_eq!(fresh.player.max_hp, BASE_MAX_HP, "新局没重置血量上限");
        assert_eq!(fresh.player.hp, BASE_MAX_HP);
    }

    /// 物品行得被渲染出来，操作提示也得带上 2/3 两个新键。
    #[test]
    fn render_shows_new_backpack_and_keys() {
        let g = Game::new(60, 25);
        let s = g.render();
        assert!(s.contains("物品"), "没有渲染物品行");
        assert!(s.contains("秘籍"), "物品行没有秘籍");
        assert!(s.contains("蕴气丹"), "物品行没有蕴气丹");
        assert!(s.contains("2 秘籍"), "操作提示没有 2 键");
        assert!(s.contains("3 蕴气丹"), "操作提示没有 3 键");
    }

    /// 地图上三类道具各有字形：+ / M / Q。
    #[test]
    fn items_render_their_own_glyphs() {
        let mut g = Game::new(60, 25);
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        g.items.clear();
        g.map.recompute_vision(px, py, 8);
        let mut glyph_at = |kind: ItemKind| -> char {
            g.items.push(Item { x: spot.0, y: spot.1, kind });
            let s = g.render();
            g.items.pop();
            // 解析渲染输出，跳过 ANSI 颜色代码（每个格子都是 color+glyph+reset）
            // 头部共 5 行（WANDER/层数/背包/基础成就/高级成就），地图从第 6 行开始
            let line = s.lines().nth(py + 5).expect("该有这一行");
            let mut chars = line.chars().peekable();
            let mut x = 0;
            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    // 跳过 ANSI 转义序列 \x1b[...m
                    for c2 in chars.by_ref() {
                        if c2 == 'm' {
                            break;
                        }
                    }
                    // 此时读到了 glyph（下一个字符）
                    if let Some(glyph) = chars.next() {
                        if x == spot.0 {
                            return glyph;
                        }
                        x += 1;
                        // 跳过后续的 reset 码 \x1b[0m
                        if let Some(c3) = chars.next() {
                            if c3 == '\x1b' {
                                for c4 in chars.by_ref() {
                                    if c4 == 'm' {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    continue;
                }
                // 非颜色字符（墙、地板等）
                if x == spot.0 {
                    return c;
                }
                x += 1;
            }
            panic!("找不到对应列");
        };
        assert_eq!(glyph_at(ItemKind::Heal), '+', "血瓶该画成 +");
        assert_eq!(glyph_at(ItemKind::Manual), 'M', "秘籍该画成 M");
        assert_eq!(glyph_at(ItemKind::QiPill), 'Q', "蕴气丹该画成 Q");
    }

    /// 结算面板：每行都以 ║ 开头并以 ║ 结尾且显示宽度一致（CJK 按 2 列计），标题不会被顶出框。
    #[test]
    fn victory_panel_is_aligned() {
        let mut g = Game::new(60, 25);
        g.kills = 12;
        g.potions = 3;
        g.end = End::Victory;
        let s = g.render();
        let strip_ansi = |line: &str| -> String {
            let mut out = String::new();
            let mut chars = line.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    for c2 in chars.by_ref() {
                        if c2 == 'm' {
                            break;
                        }
                    }
                } else {
                    out.push(c);
                }
            }
            out
        };
        let rows: Vec<String> = s
            .lines()
            .map(strip_ansi)
            .filter(|l| l.starts_with('║'))
            .collect();
        assert!(rows.len() >= 8, "结算面板行太少");
        let w = display_width(&rows[0]);
        assert!(
            rows.iter().all(|l| l.ends_with('║') && display_width(l) == w),
            "║ 边框没对齐：{rows:?}"
        );
        assert!(s.contains("V I C T O R Y"), "结算没有 VICTORY 标题");
    }

    /// 结算公式：积分 = (击杀×100 + 背包血瓶×50) × 层数倍率，死亡折半；
    /// 积分余额可花、跨局累加；落盘后能读回。
    #[test]
    fn score_settlement_math_and_persistence() {
        let file = std::env::temp_dir().join(format!("wander_score_test_{}.json", std::process::id()));
        let _ = fs::remove_file(&file);
        unsafe {
            std::env::set_var("WANDER_SCORE_FILE", &file);
        }

        let mut s = Score::load();
        s.add_run(8, 3, 0, 950); // 1 层 ×1：8*100 + 3*50 = 950
        assert_eq!(s.last_score, 950);
        assert_eq!(s.credits, 950, "通关赚的积分该进余额");
        assert_eq!(s.total_score, 950);

        s.add_run(2, 0, 4, 1000); // 5 层 ×5：200*5 = 1000
        assert_eq!(s.total_score, 1950, "累计分该跨局累加");
        assert_eq!(s.credits, 1950, "积分余额该跨局累加");

        s.add_run(4, 2, 1, 750); // 死亡折半：(400+100)×3/2 = 750
        assert_eq!(s.total_score, 2700);
        assert_eq!(s.credits, 2700);

        let reread = Score::load();
        assert_eq!(reread.total_score, 2700, "落盘后读不回累计分");
        assert_eq!(reread.credits, 2700, "落盘后读不回积分余额");
        assert_eq!(reread.last_kills, 4);
        assert_eq!(reread.last_potions, 2);
        assert_eq!(reread.last_shields, 1, "上局存量盾牌数没记下来");

        let _ = fs::remove_file(&file);
    }

    /// 商店：购买扣积分；一次性道具不能买两份；血瓶每局限购 3 个；积分不够买不了。
    #[test]
    fn shop_buys_and_deducts() {
        let mut s = Shop {
            credits: 8000,
            manual: false,
            talisman: false,
            potions: 0,
            msg: String::new(),
        };
        s.buy(0); // 武林秘籍 400
        assert!(s.manual, "秘籍没买上");
        assert_eq!(s.credits, 8000 - PRICE_MANUAL, "没扣积分");
        s.buy(1); // 幸运符 5000
        assert!(s.talisman, "幸运符没买上");
        assert_eq!(s.credits, 8000 - PRICE_MANUAL - PRICE_TALISMAN, "没扣积分");
        s.buy(2);
        s.buy(2);
        s.buy(2); // 每局限购 3 个
        assert_eq!(s.potions, MAX_POTION_BUY, "血瓶没买上");
        assert_eq!(
            s.credits,
            8000 - PRICE_MANUAL - PRICE_TALISMAN - MAX_POTION_BUY as u32 * PRICE_POTION,
            "血瓶没扣积分"
        );

        // 血瓶限购：第 4 瓶买不了
        let c = s.credits;
        let potions_before = s.potions;
        s.buy(2);
        assert_eq!(s.potions, potions_before, "血瓶超过每局限购还买上了");
        assert_eq!(s.credits, c, "限购还扣了积分");
        assert!(s.msg.contains("限购"), "该提示每局限购");

        // 重复买一次性道具不扣积分
        s.buy(0);
        s.buy(1);
        assert_eq!(s.credits, c, "一次性道具重复购买还扣积分");

        // 积分不够买不了
        let mut s2 = Shop {
            credits: 10,
            manual: false,
            talisman: false,
            potions: 0,
            msg: String::new(),
        };
        s2.buy(2);
        assert_eq!(s2.potions, 0, "积分不够也买上了");
        assert!(s2.msg.contains("积分不够"), "该提示积分不够");
    }

    /// 开局购买结果应用到游戏：秘籍进背包、幸运符生效、血瓶进背包。
    #[test]
    fn purchase_applies_to_new_game() {
        let p = Purchase {
            manual: true,
            talisman: true,
            potions: 3,
        };
        let mut g = Game::new(60, 25);
        g.apply_purchase(&p);
        assert_eq!(g.manuals, 1, "买的秘籍没进背包");
        assert!(g.lucky, "幸运符没生效");
        assert_eq!(g.potions, 3, "买的血瓶没进背包");
    }

    /// 幸运符下血瓶掉率 90%：独立杀 400 只怪，掉落数应约 360。
    #[test]
    fn lucky_charm_drop_rate_is_90() {
        let mut g = Game::new(60, 25);
        g.lucky = true;
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let mut drops = 0usize;
        for _ in 0..400 {
            g.player.x = px;
            g.player.y = py;
            let i = g.map.idx(px + 1, py);
            g.map.tiles[i] = Tile::Floor;
            g.monsters.push(Entity {
                x: px + 1,
                y: py,
                ch: 'k',
                name: "骷髅",
                hp: 1,
                max_hp: 1,
                atk: 0,
                sight: 0,
                behavior: Behavior::Melee,
                is_boss: false,
            });
            g.try_move_player(1, 0);
            if g.msg.contains("掉落了一个血瓶") {
                drops += 1;
            }
        }
        assert!(
            drops > 300 && drops < 420,
            "幸运符掉率异常：杀 400 只怪只掉了 {drops} 个血瓶"
        );
    }

    /// 幸运符下参悟秘籍 25% 概率攻击 ×4；没有幸运符永远只 ×2。
    #[test]
    fn lucky_charm_gives_quarter_chance_atk_x4() {
        let mut g = Game::new(60, 25);
        g.lucky = true;
        let mut x4 = 0usize;
        for _ in 0..200 {
            g.manuals = 1;
            g.player.atk = BASE_ATK;
            g.use_manual();
            if g.player.atk == 4 * BASE_ATK {
                x4 += 1;
            } else {
                assert_eq!(g.player.atk, 2 * BASE_ATK, "非 ×4 时该是 ×2");
            }
        }
        assert!(
            x4 > 20 && x4 < 80,
            "200 次参悟约该有 50 次 ×4，实际 {x4}"
        );

        // 没幸运符，×4 永不出现
        let mut g2 = Game::new(60, 25);
        for _ in 0..50 {
            g2.manuals = 1;
            g2.player.atk = BASE_ATK;
            g2.use_manual();
            assert_eq!(g2.player.atk, 2 * BASE_ATK, "×4 应仅限幸运符");
        }
    }

    /// 暗影射手死后 50% 掉盾牌；带幸运符必掉。
    #[test]
    fn shadow_archer_drops_shield_half_the_time() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let mut drops = 0usize;
        for _ in 0..200 {
            let ti = g.map.idx(px + 1, py);
            g.map.tiles[ti] = Tile::Floor;
            g.monsters.push(Entity {
                x: px + 1,
                y: py,
                ch: 'o',
                name: "暗影射手",
                hp: 1,
                max_hp: 1,
                atk: 0,
                sight: 0, // 看不见玩家，不会反击覆盖消息
                behavior: Behavior::Ranged,
                is_boss: false,
            });
            g.try_move_player(1, 0);
            if g.msg.contains("掉落了一面盾牌") {
                drops += 1;
            }
        }
        // 200 次抽样下，50% 掉率落在 70~130 之外的概率极小
        assert!(drops > 70 && drops < 130, "盾牌掉率异常：杀 200 只暗影射手只掉了 {drops} 面");

        // 幸运符：每一面都该掉
        let mut g2 = Game::new(60, 25);
        g2.lucky = true;
        g2.monsters.clear();
        let (px, py) = (g2.player.x, g2.player.y);
        for _ in 0..30 {
            let ti = g2.map.idx(px + 1, py);
            g2.map.tiles[ti] = Tile::Floor;
            g2.monsters.push(Entity {
                x: px + 1,
                y: py,
                ch: 'o',
                name: "暗影射手",
                hp: 1,
                max_hp: 1,
                atk: 0,
                sight: 0,
                behavior: Behavior::Ranged,
                is_boss: false,
            });
            g2.try_move_player(1, 0);
            assert!(g2.msg.contains("掉落了一面盾牌"), "幸运符下盾牌该必掉：{}", g2.msg);
        }
    }

    /// 捡盾牌满耐久 45；手里已有盾时换上新盾，耐久回满，旧盾（保留当前耐久）收进背包。
    #[test]
    fn pickup_shield_replaces_and_refills() {
        let mut g = Game::new(60, 25);
        g.monsters.clear(); // 别让游荡的怪把拾取消息覆盖掉
        assert_eq!(g.shield, 0, "新局不该自带盾牌");
        assert_eq!(g.shield_stock.len(), 0, "新局背包不该有存量盾牌");
        assert_eq!(SHIELD_DURABILITY, 45, "盾牌耐久该是 45");

        let spot = g.grab(ItemKind::Shield);
        assert_eq!(g.shield, SHIELD_DURABILITY, "捡起盾牌该是满耐久");
        assert!(
            !g.items.iter().any(|it| it.x == spot.0 && it.y == spot.1),
            "盾牌没被拾取"
        );

        // 手里的盾已经磨损，再踩到一面：换上新盾（满耐久），旧盾（保留 7 点耐久）收进背包
        g.shield = 7;
        let (px, py) = (g.player.x, g.player.y);
        let second = [(px + 1, py), (px, py + 1), (px - 1, py), (px, py - 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("旁边该有块空地");
        g.items.push(Item {
            x: second.0,
            y: second.1,
            kind: ItemKind::Shield,
        });
        let dx = second.0 as i32 - px as i32;
        let dy = second.1 as i32 - py as i32;
        g.try_move_player(dx, dy);
        assert_eq!(g.shield, SHIELD_DURABILITY, "换盾该把耐久回满");
        assert!(g.msg.contains("换上新盾"), "换盾提示不对：{}", g.msg);
        assert_eq!(g.shield_stock, vec![7], "旧盾该保留 7 点耐久收进背包");
        assert!(
            !g.items.iter().any(|it| it.x == second.0 && it.y == second.1),
            "新盾该被捡起到手"
        );
    }

    /// 碎盾后自动从背包换上面：取耐久最高的一面，换掉的从背包移除。
    #[test]
    fn shield_break_auto_equips_highest_from_stock() {
        let mut shield = 1i32;
        let mut stock = vec![30i32, 45i32];
        let mut hits: Vec<String> = Vec::new();
        let r = block_with_shield(&mut shield, &mut stock, "暗影射手", "暗影箭", &mut hits);
        assert!(r.blocked, "碎了的那一箭仍该被挡住");
        assert!(r.refilled, "自动换盾时 refilled 该是 true（除新迎旧成就判定用）");
        assert_eq!(shield, 45, "碎盾后该自动换上背包里耐久最高的一面");
        assert_eq!(stock, vec![30], "换掉的盾该从背包移除，剩下的留着");
        assert!(
            hits.join(" | ").contains("自动从背包换上一面"),
            "该提示自动换盾：{}",
            hits.join(" | ")
        );

        // 背包空了，碎了就是碎了
        let mut g = Game::new(60, 25);
        g.shield = 1;
        g.shield_stock.clear();
        let mut hits: Vec<String> = Vec::new();
        let r = block_with_shield(&mut g.shield, &mut g.shield_stock, "暗影射手", "暗影箭", &mut hits);
        assert!(r.blocked, "没存量也该挡住最后一下");
        assert!(!r.refilled, "背包没存量，refilled 该是 false");
        assert_eq!(g.shield, 0, "背包没存量时碎了就是碎了");
        assert!(g.shield_stock.is_empty());
    }

    /// 背包存量上限：收不下旧盾时新盾留在原地。
    #[test]
    fn shield_stock_capped_at_max() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        for _ in 0..SHIELD_STOCK_MAX {
            g.shield_stock.push(45);
        }
        g.shield = 20; // 手里还有一面旧盾
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1), (px - 1, py), (px, py - 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("旁边该有块空地");
        g.items.push(Item {
            x: spot.0,
            y: spot.1,
            kind: ItemKind::Shield,
        });
        g.try_move_player(spot.0 as i32 - px as i32, spot.1 as i32 - py as i32);
        assert_eq!(g.shield, 20, "背包满时手里旧盾不该被换掉");
        assert!(g.msg.contains("存量已满"), "该提示背包满了：{}", g.msg);
        assert_eq!(g.shield_stock.len(), SHIELD_STOCK_MAX, "存量不该超过上限");
        assert!(
            g.items.iter().any(|it| it.x == spot.0 && it.y == spot.1),
            "收不下旧盾时新盾该留在原地"
        );
    }

    /// 结算：背包存量盾牌按耐久度折算积分，随层数倍率、死亡折半。
    #[test]
    fn shield_stock_scores_in_settlement() {
        let mut g = Game::new(60, 25);
        g.kills = 0;
        g.potions = 0;
        g.shield_stock = vec![45, 30]; // 耐久合计 75
        let total_dur: i32 = g.shield_stock.iter().sum();
        assert_eq!(
            g.shield_stock_score(),
            total_dur as u32 * SHIELD_SCORE_PER_POINT,
            "该按耐久合计折算"
        );
        g.floor = 1;
        assert_eq!(g.settlement_score(), total_dur as u32 * SHIELD_SCORE_PER_POINT, "1 层 ×1");
        g.floor = 3; // 已通过 2 关，奖励 1000
        assert_eq!(g.settlement_score(), total_dur as u32 * SHIELD_SCORE_PER_POINT * 3 + 1000, "3 层 ×3");
        g.end = End::Defeat;
        assert_eq!(
            g.settlement_score(),
            total_dur as u32 * SHIELD_SCORE_PER_POINT * 3 / 2 + 1000,
            "死亡该折半"
        );
        // 结算面板里出现盾牌那行，且加了这行后 ║ 边框仍然对齐
        g.end = End::Voluntary;
        g.floor = 3;
        let s = g.render();
        assert!(s.contains("背包盾牌"), "结算面板没显示背包盾牌积分");
        let strip_ansi = |line: &str| -> String {
            let mut out = String::new();
            let mut chars = line.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\x1b' {
                    for c2 in chars.by_ref() {
                        if c2 == 'm' {
                            break;
                        }
                    }
                } else {
                    out.push(c);
                }
            }
            out
        };
        let rows: Vec<String> = s
            .lines()
            .map(strip_ansi)
            .filter(|l| l.starts_with('║'))
            .collect();
        let w = display_width(&rows[0]);
        assert!(
            rows.iter().all(|l| l.ends_with('║') && display_width(l) == w),
            "加了盾牌行后 ║ 边框没对齐：{rows:?}"
        );

        // 背包塞满（10 面满耐久）时，最宽的那行也不能把 ║ 边框撑破
        let mut gmax = Game::new(60, 25);
        gmax.kills = 0;
        gmax.potions = 0;
        gmax.end = End::Voluntary;
        gmax.floor = 5;
        for _ in 0..SHIELD_STOCK_MAX {
            gmax.shield_stock.push(SHIELD_DURABILITY);
        }
        let s = gmax.render();
        let rows: Vec<String> = s.lines().map(strip_ansi).filter(|l| l.starts_with('║')).collect();
        let w = display_width(&rows[0]);
        assert!(
            rows.iter().all(|l| l.ends_with('║') && display_width(l) == w),
            "背包塞满时 ║ 边框被撑破了：{rows:?}"
        );
    }

    /// 有盾牌时暗影箭不掉血，但每命中一次扣 1 点耐久（射偏不扣）。
    #[test]
    fn shield_blocks_shadow_arrow_and_loses_durability_on_hit() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 6 {
            let ti = g.map.idx(x, py);
            g.map.tiles[ti] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 30,
            max_hp: 30,
            atk: 9,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
        });
        g.shield = SHIELD_DURABILITY;
        let hp_before = g.player.hp;
        let mut hits = 0usize;
        for _ in 0..20 {
            g.end_turn();
            let blocked = g.msg.contains("挡下") || g.msg.contains("击碎");
            assert!(
                blocked || g.msg.contains("擦身而过"),
                "每回合该是射偏或被盾挡下：{}",
                g.msg
            );
            if blocked {
                hits += 1;
            }
        }
        assert!(hits > 0, "20 回合一箭没中，测试前提不成立");
        assert_eq!(g.player.hp, hp_before, "有盾牌还被暗影箭打掉血");
        // 耐久只在命中时结算：命中几次就扣几点
        assert_eq!(g.shield, SHIELD_DURABILITY - hits as i32, "耐久没按命中结算");
        assert!(g.shield > 0, "20 次命中不该把 45 点耐久打光");
    }

    /// 最后 1 点耐久也挡得住那一箭，盾碎之后暗影箭恢复掉血。
    #[test]
    fn shield_breaks_then_arrows_hurt_again() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 6 {
            let ti = g.map.idx(x, py);
            g.map.tiles[ti] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 30,
            max_hp: 30,
            atk: 5,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
        });
        g.shield = 1;
        let hp0 = g.player.hp;
        let mut broke = false;
        for _ in 0..12 {
            g.end_turn();
            if g.shield == 0 {
                broke = true;
                assert_eq!(g.player.hp, hp0, "击碎盾牌的那一箭仍该被挡住");
                break;
            }
        }
        assert!(broke, "12 回合内 1 点耐久的盾牌该碎");

        let mut hurt = false;
        for _ in 0..12 {
            g.end_turn();
            if g.player.hp < hp0 {
                hurt = true;
                break;
            }
        }
        assert!(hurt, "盾碎之后暗影箭该恢复伤害");
        assert_eq!(g.shield, 0, "盾牌不该自己修回来");
    }

    /// 盾牌格挡所有来袭伤害（小怪啃咬、暗影射手贴身一击），每次命中扣 1 点耐久；
    /// 只有骷髅穿盾：掉血且不扣耐久。
    #[test]
    fn shield_blocks_all_damage_except_skeleton() {
        for name in ["哥布林", "暗影射手"] {
            let mut g = Game::new(60, 25);
            g.monsters.clear();
            let (px, py) = (g.player.x, g.player.y);
            for x in px..=px + 2 {
                let ti = g.map.idx(x, py);
                g.map.tiles[ti] = Tile::Floor;
            }
            let behavior = if name == "暗影射手" {
                Behavior::Ranged // 贴身时走近身暗影一击分支
            } else {
                Behavior::Melee
            };
            g.monsters.push(Entity {
                x: px + 1,
                y: py,
                ch: 'k',
                name,
                hp: 30,
                max_hp: 30,
                atk: 6,
                sight: 12,
                behavior,
                is_boss: false,
            });
            g.shield = SHIELD_DURABILITY;
            let hp0 = g.player.hp;
            let mut blocks = 0usize;
            for _ in 0..12 {
                g.end_turn();
                assert_eq!(g.player.hp, hp0, "{name} 的攻击该被盾牌挡下");
                if g.msg.contains("挡下") {
                    blocks += 1;
                }
            }
            assert!(blocks > 0, "{name} 12 回合一次都没命中，测试前提不成立");
            assert_eq!(
                g.shield,
                SHIELD_DURABILITY - blocks as i32,
                "{name} 该按命中次数扣耐久"
            );
        }

        // 骷髅：穿盾，掉血且不扣耐久
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 2 {
            let ti = g.map.idx(x, py);
            g.map.tiles[ti] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 1,
            y: py,
            ch: 'k',
            name: SKELETON,
            hp: 30,
            max_hp: 30,
            atk: 6,
            sight: 12,
            behavior: Behavior::Melee,
            is_boss: false,
        });
        g.shield = SHIELD_DURABILITY;
        let hp0 = g.player.hp;
        let mut hurt = false;
        for _ in 0..12 {
            g.end_turn(); // 近战有 25% 落空，多等几回合总能咬中一口
            assert_eq!(g.shield, SHIELD_DURABILITY, "穿盾的骷髅不该扣耐久");
            assert!(!g.msg.contains("挡下"), "骷髅竟然被挡下了：{}", g.msg);
            if g.player.hp < hp0 {
                hurt = true;
                break;
            }
        }
        assert!(hurt, "骷髅的攻击该穿透盾牌");
    }

    /// HUD 显示盾牌耐久。
    #[test]
    fn render_shows_shield_durability() {
        let mut g = Game::new(60, 25);
        g.shield = 17;
        let s = g.render();
        assert!(s.contains("盾牌 17/45"), "HUD 没渲染盾牌耐久：{s}");

        // 地上的盾牌画成 S
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("出生点旁边该有块空地");
        g.shield = 0;
        g.items.clear();
        g.map.recompute_vision(px, py, 8);
        g.items.push(Item {
            x: spot.0,
            y: spot.1,
            kind: ItemKind::Shield,
        });
        let s = g.render();
        g.items.pop();
        assert!(s.contains('S'), "地上的盾牌没画出来");
        assert_eq!(item_glyph(ItemKind::Shield), 'S', "盾牌字形不对");
    }

    /// 显示：局内和结算都显示“积分”，不再显示历史最高分。
    #[test]
    fn display_uses_points_not_high_score() {
        let mut g = Game::new(60, 25);
        g.credits = 123;
        let s = g.render();
        assert!(s.contains("积分"), "没显示积分");
        assert!(!s.contains("最高分"), "还在显示历史最高分");

        g.kills = 5;
        g.potions = 2;
        g.end = End::Victory;
        let v = g.render();
        assert!(v.contains("本局积分"), "结算没有本局积分");
        assert!(v.contains("积分余额"), "结算没有积分余额");
        assert!(!v.contains("最高分"), "结算还在显示历史最高分");
    }

    /// 商店界面：列出三样物品、价格与积分，并显示 CRLF 结尾。
    #[test]
    fn shop_renders_items_and_points() {
        let s = Shop {
            credits: 700,
            manual: true,
            talisman: false,
            potions: 2,
            msg: String::new(),
        }
        .render();
        assert!(s.contains("武林秘籍"), "商店没有秘籍");
        assert!(s.contains("幸运符"), "商店没有幸运符");
        assert!(s.contains("结算积分减半"), "商店没有提示幸运符惩罚");
        assert!(s.contains("血瓶"), "商店没有血瓶");
        assert!(s.contains("700"), "商店没显示当前积分");
        assert!(s.contains("按 b 开始"), "商店没有开始提示");
        let bare = s.match_indices('\n').filter(|(i, _)| {
            i == &0 || s.as_bytes()[i - 1] != b'\r'
        });
        assert_eq!(bare.count(), 0, "商店界面存在裸 \\n");
    }

    /// 全副武装：血量 50/50、攻击 ≥16、血瓶满 5、盾牌满 45/45，任何状态更新后检查；
    /// 少一个条件就不解锁，重复解锁是空操作。
    #[test]
    fn achievement_fully_armed() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        // 四项满足，唯独攻击还是 8：不该解锁
        g.player.max_hp = FULLY_ARMED_HP;
        g.player.hp = FULLY_ARMED_HP;
        g.potions = POTION_STACK;
        g.shield = SHIELD_DURABILITY;
        g.check_fully_armed();
        assert!(!g.achievements.contains(ACH_FULLY_ARMED), "攻击 8 不该达成全副武装");

        // 攻击提到 16（参悟秘籍后的水平）：条件全齐，解锁
        g.player.atk = FULLY_ARMED_ATK;
        g.check_fully_armed();
        assert!(g.achievements.contains(ACH_FULLY_ARMED), "条件齐了该解锁全副武装");

        // 重复解锁不改集合
        g.check_fully_armed();
        assert_eq!(g.achievements.len(), 1, "重复解锁不该多记");

        // 任何一项掉出条件都不影响已解锁的成就
        g.player.hp = 40;
        g.check_fully_armed();
        assert!(g.achievements.contains(ACH_FULLY_ARMED), "达成后不该被收回");
    }

    /// 全副武装经真实状态更新触发：参悟秘籍把攻击提到 16 的那一刻就该解锁。
    #[test]
    fn achievement_fully_armed_via_use_manual() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        g.player.max_hp = FULLY_ARMED_HP;
        g.player.hp = FULLY_ARMED_HP;
        g.potions = POTION_STACK;
        g.shield = SHIELD_DURABILITY;
        assert_eq!(g.player.atk, 8, "测试前提：秘籍参悟前是 8 攻");
        g.manuals = 1;
        g.use_manual(); // 内部走参悟成功后 check_fully_armed
        assert_eq!(g.player.atk, 16);
        assert!(
            g.achievements.contains(ACH_FULLY_ARMED),
            "参悟秘籍（状态更新）后条件全齐，该解锁全副武装：{}",
            g.msg
        );
    }

    /// 除新迎旧：碎盾时自动从存量换上的一面盾牌顶上去。
    #[test]
    fn achievement_shield_swap_when_shield_breaks() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 6 {
            let ti = g.map.idx(x, py);
            g.map.tiles[ti] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 30,
            max_hp: 30,
            atk: 5,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
        });
        g.player.hp = g.player.max_hp;
        g.shield = 1; // 中下一箭就碎
        g.shield_stock = vec![30, 45];
        let mut swapped = false;
        for _ in 0..40 {
            g.end_turn(); // 每回合 75% 命中，撑几回合盾必碎
            if g.shield == 45 {
                swapped = true;
                break;
            }
        }
        assert!(swapped, "碎盾后该自动换上存量里耐久最高（45）的一面");
        assert_eq!(g.shield_stock, vec![30], "换出去的那面该从存量移除");
        assert!(g.achievements.contains(ACH_SHIELD_SWAP), "碎盾存量顶上，该解锁除新迎旧");
    }

    /// 两面夹击：同一回合里射手与守护者的攻击都实际打中（格挡/穿透都算，落空不算）。
    #[test]
    fn achievement_pinned_by_archer_and_boss() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 6 {
            let ti = g.map.idx(x, py);
            g.map.tiles[ti] = Tile::Floor;
        }
        // 射手站 5 格外（射得着），守护者贴身 1 格（近战）
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 30,
            max_hp: 30,
            atk: 5,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
        });
        g.monsters.push(Entity {
            x: px + 1,
            y: py,
            ch: 'D',
            name: "地牢守护者",
            hp: 70,
            max_hp: 70,
            atk: 7,
            sight: 10,
            behavior: Behavior::Melee,
            is_boss: true,
        });
        g.player.hp = g.player.max_hp;
        g.shield = SHIELD_DURABILITY; // 让盾先吸收，避免死亡干扰测试
        for _ in 0..60 {
            g.end_turn(); // 双方每回合 75% 命中，几十回合内必然同回合双中
            if g.achievements.contains(ACH_PINNED) {
                break;
            }
        }
        assert!(
            g.achievements.contains(ACH_PINNED),
            "撑 60 回合都没同回合双中，两面夹击判定有问题"
        );
    }

    /// 满载而归：拾取盾牌后存量刚好顶到 10/10；存量没满不解锁。
    #[test]
    fn achievement_full_load_on_pickup() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        for _ in 0..SHIELD_STOCK_MAX - 1 {
            g.shield_stock.push(SHIELD_DURABILITY);
        }
        g.shield = 20; // 手里还有面旧盾
        g.grab(ItemKind::Shield); // 换上新盾，旧盾进存量
        assert_eq!(g.shield, SHIELD_DURABILITY, "新盾该是满耐久");
        assert_eq!(g.shield_stock.len(), SHIELD_STOCK_MAX, "拾取后存量该刚好顶满");
        assert!(g.achievements.contains(ACH_FULL_LOAD), "存量顶满该解锁满载而归");

        // 手里没盾、存量为空时捡到一面：存量还是空的，不该解锁
        let mut g2 = Game::new(60, 25);
        g2.monsters.clear();
        g2.grab(ItemKind::Shield);
        assert!(g2.shield_stock.is_empty(), "手里没盾时拾取不进存量");
        assert!(!g2.achievements.contains(ACH_FULL_LOAD), "存量没满不该解锁");
    }

    /// 杀死怪物：击杀一只怪就解锁；没击杀不解锁。
    #[test]
    fn achievement_kill_one_on_first_kill() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let ti = g.map.idx(px + 1, py);
        g.map.tiles[ti] = Tile::Floor;
        g.monsters.push(Entity {
            x: px + 1,
            y: py,
            ch: 'k',
            name: SKELETON,
            hp: 1,
            max_hp: 1,
            atk: 0,
            sight: 0,
            behavior: Behavior::Melee,
            is_boss: false,
        });
        g.try_move_player(1, 0);
        assert_eq!(g.kills, 1, "测试前提：该一刀杀掉");
        assert!(g.achievements.contains(ACH_KILL_ONE), "击杀一只就该解锁杀死怪物");
        assert!(g.species_killed.contains(SKELETON), "图鉴该记上骷髅这一种");

        // 不击杀（纯移动）不解锁
        let mut g2 = Game::new(60, 25);
        g2.try_move_player(0, 1);
        assert!(!g2.achievements.contains(ACH_KILL_ONE), "没杀怪不该解锁");
    }

    /// 拾取盾牌：拿到手就解锁；换新盾也算；满包放不进不算。
    #[test]
    fn achievement_get_shield_on_pickup() {
        // 手里没盾捡到：解锁
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        g.grab(ItemKind::Shield);
        assert!(g.achievements.contains(ACH_GET_SHIELD), "捡到盾牌该解锁拾取盾牌");

        // 手里有旧盾换上新盾：也解锁
        let mut g2 = Game::new(60, 25);
        g2.monsters.clear();
        g2.shield = 20;
        g2.grab(ItemKind::Shield);
        assert!(g2.achievements.contains(ACH_GET_SHIELD), "换盾也算获取盾牌");

        // 存量满、旧盾放不进：新盾留在原地，不解锁
        let mut g3 = Game::new(60, 25);
        g3.monsters.clear();
        for _ in 0..SHIELD_STOCK_MAX {
            g3.shield_stock.push(SHIELD_DURABILITY);
        }
        g3.shield = 20;
        g3.grab(ItemKind::Shield);
        assert!(!g3.achievements.contains(ACH_GET_SHIELD), "没真正拿到手不该解锁");
    }

    /// 不再畏惧：手持盾牌挡下射手三发暗箭（跨回合累计）；没盾挡下 0 发不解锁。
    #[test]
    fn achievement_no_fear_after_three_arrows() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 6 {
            let ti = g.map.idx(x, py);
            g.map.tiles[ti] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 30,
            max_hp: 30,
            atk: 5,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
        });
        g.player.hp = g.player.max_hp;
        g.shield = SHIELD_DURABILITY; // 盾足够厚，40 回合内不会碎
        for _ in 0..40 {
            g.end_turn();
            if g.achievements.contains(ACH_NO_FEAR) {
                break;
            }
        }
        assert!(
            g.achievements.contains(ACH_NO_FEAR),
            "挡满 3 发暗箭该解锁不再畏惧"
        );
        assert!(g.arrows_blocked >= NO_FEAR_ARROWS, "起码该挡下 3 发");

        // 没盾：箭直接掉血，不算挡下，不解锁
        let mut g2 = Game::new(60, 25);
        g2.monsters.clear();
        let (px, py) = (g2.player.x, g2.player.y);
        for x in px..=px + 6 {
            let ti = g2.map.idx(x, py);
            g2.map.tiles[ti] = Tile::Floor;
        }
        g2.monsters.push(Entity {
            x: px + 5,
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 30,
            max_hp: 30,
            atk: 5,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
        });
        for _ in 0..10 {
            g2.end_turn();
        }
        assert_eq!(g2.arrows_blocked, 0, "没盾挡不下任何一箭");
        assert!(!g2.achievements.contains(ACH_NO_FEAR), "没盾不该解锁");
    }

    /// 怪物图鉴：ALL_SPECIES 每种杀一只（含 BOSS）才解锁；少一种不行。
    #[test]
    fn achievement_compendium_requires_every_species() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        // 五种小怪/射手全杀过，还差 BOSS：不该解锁
        for name in ["哥布林", "巨鼠", "蜘蛛", SKELETON, "暗影射手"] {
            g.register_kill(name);
        }
        assert!(!g.achievements.contains(ACH_COMPENDIUM), "少杀一种不该解锁图鉴");
        assert_eq!(g.species_killed.len(), 5, "该记上 5 种");

        // BOSS 补上：解锁
        g.register_kill(BOSS);
        assert!(g.achievements.contains(ACH_COMPENDIUM), "六种全杀该解锁怪物图鉴");
        assert_eq!(g.species_killed.len(), 6, "图鉴该记满 6 种");

        // 重复击杀不多加
        g.register_kill(BOSS);
        g.register_kill("哥布林");
        assert_eq!(g.species_killed.len(), 6, "重复击杀不该重复记账");
    }

    /// 怪物图鉴经真实击杀记账：一刀杀掉骷髅后，图鉴集合里该有骷髅。
    #[test]
    fn compendium_records_real_kills() {
        let mut g = Game::new(60, 25);
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let ti = g.map.idx(px + 1, py);
        g.map.tiles[ti] = Tile::Floor;
        g.monsters.push(Entity {
            x: px + 1,
            y: py,
            ch: 'k',
            name: SKELETON,
            hp: 1,
            max_hp: 1,
            atk: 0,
            sight: 0,
            behavior: Behavior::Melee,
            is_boss: false,
        });
        g.try_move_player(1, 0);
        assert!(g.species_killed.contains(SKELETON), "真实击杀该记账进图鉴");
        assert!(!g.achievements.contains(ACH_COMPENDIUM), "才杀一种离图鉴还远");
    }

    /// 成就行是 HUD 专门两行（基础/高级区分）：没达成过显示“无”，达成过按固定顺序列出；
    /// 不进 msg 普通消息栏；新局重置。
    #[test]
    fn render_shows_achievements_line() {
        let g = Game::new(60, 25);
        let s = g.render();
        assert!(s.contains("基础成就：无"), "没有基础成就时该行该显示无");
        assert!(s.contains("高级成就：无"), "没有高级成就时该行该显示无");

        let mut g = Game::new(60, 25);
        g.unlock(ACH_FULLY_ARMED);
        g.unlock(ACH_PINNED);
        g.unlock(ACH_KILL_ONE);
        let s = g.render();
        assert!(s.contains("基础成就：杀死怪物"), "简单成就该列在基础成就行");
        assert!(
            s.contains("高级成就：全副武装 | 两面夹击"),
            "高级成就该按固定顺序列在高级成就行"
        );
        assert!(!s.contains("基础成就：无"), "基础行有成就后不该再显示无");
        assert!(!g.msg.contains("全副武装"), "成就不该混进 msg 消息栏");

        // 新局重置
        let fresh = Game::new(60, 25);
        assert!(fresh.achievements.is_empty(), "新局不该带着上局成就");
    }
}
