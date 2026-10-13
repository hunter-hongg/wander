use crossterm::event::KeyCode;
use rand::RngExt;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use crate::map::{Map, Tile, W, H};

pub(crate) const HEAL_AMOUNT: i32 = 15;
/// 暗影箭的射界下限：距离 <= 3 格时无法射箭。
pub(crate) const MIN_RANGED_RANGE: usize = 4;
/// 暗影射手近身（贴着玩家）时的基础伤害，实际伤害上下浮动 1 点。
pub(crate) const SHADOW_MELEE_BASE: i32 = 2;
/// 暗影射手死后掉盾牌的概率（%），带幸运符时必掉。
pub(crate) const SHIELD_DROP_PERCENT: u32 = 50;
pub(crate) const SHIELD_LUCKY_DROP_PERCENT: u32 = 100;
/// 盾牌耐久：格挡一次命中扣 1 点（落空不扣），扣到 0 盾牌碎裂、恢复掉血。
pub(crate) const SHIELD_DURABILITY: i32 = 45;
/// 背包里最多留存几面盾牌（不含手里正在用的那面）。
pub(crate) const SHIELD_STOCK_MAX: usize = 10;
/// 结算时背包里每面存量盾牌按耐久度折算积分：每 1 点耐久 = 多少积分。
pub(crate) const SHIELD_SCORE_PER_POINT: u32 = 2;
/// 骷髅的攻击无视盾牌；6-7 层守护者的暗箭同样穿透盾牌。
pub(crate) const SKELETON: &str = "骷髅";
/// 盾卫：6-7 层取代骷髅的近战怪，自带一面耐久不全的破盾，被攻击时先拿盾顶。
pub(crate) const SHIELD_GUARD: &str = "盾卫";
/// 每层地牢的血瓶数量（9 间房里只有这些，不再每房一个）。
pub(crate) const POTIONS_PER_FLOOR: usize = 4;
/// 怪物死亡时掉落血瓶的概率（1/2）。
pub(crate) const DROP_CHANCE: u32 = 2;
/// 背包最多容纳多少个血瓶，捡到就往里塞。
pub(crate) const POTION_STACK: usize = 10;
/// 玩家的初始血量上限与攻击力，重开新一局时回到这个水平。
pub(crate) const BASE_MAX_HP: i32 = 40;
pub(crate) const BASE_ATK: i32 = 8;
/// 蕴气丹：服用消耗 10 点血，血量上限提升 10。
pub(crate) const QI_HP_COST: i32 = 10;
pub(crate) const QI_MAX_HP_BONUS: i32 = 10;

/// 商店：开局用上一局赚的积分购买的物品价格。
pub(crate) const PRICE_MANUAL: u32 = 400;
pub(crate) const PRICE_TALISMAN: u32 = 5000;
/// 结算时每通过一关（守护者倒下）的固定通关奖励积分，不受层数倍率和折半影响。
pub(crate) const CLEAR_BONUS_PER_FLOOR: u32 = 500;
/// 骷髅在基础攻击上额外增加的攻击数值（骷髅本就穿盾，更危险）。
pub(crate) const SKELETON_ATK_BONUS: i32 = 2;
/// 骷髅攻击力上限：再强也不超过这个数。
pub(crate) const MAX_SKELETON_ATK: i32 = 15;
pub(crate) const PRICE_POTION: u32 = 200;
/// 血瓶每局限购数量，再买没意义（背包上限也不高）。
pub(crate) const MAX_POTION_BUY: usize = 3;
/// 幸运符：怪物血瓶掉落概率提升至 90%（默认 50%）。
pub(crate) const LUCKY_DROP_PERCENT: u32 = 90;
/// 幸运符：参悟武林秘籍时 25% 概率攻击力 ×4（×4 仅此唯一获得途径）。
pub(crate) const LUCKY_QUAD_CHANCE: u32 = 25;
/// 骷髅掉落骷髅刀的概率（%）；带幸运符时提升至 80%。
pub(crate) const BLADE_DROP_PERCENT: u32 = 20;
pub(crate) const BLADE_LUCKY_DROP_PERCENT: u32 = 80;
/// 骷髅刀效果：装备时攻击固定 +6（0级）或 +8（1级）。
pub(crate) const BLADE_ATK_BONUS: i32 = 6;
pub(crate) const BLADE_ATK_BONUS_LV1: i32 = 8;
/// 骷髅刀攻击范围：1 格（0级）或 2 格（1级）。
pub(crate) const BLADE_RANGE: usize = 1;
pub(crate) const BLADE_RANGE_LV1: usize = 2;
/// 升级所需的同类型武器数量。
pub(crate) const UPGRADE_COST: usize = 20;
/// 结算时一把骷髅刀按多少积分结算（进基础分，随层数倍率与折半）。
pub(crate) const BLADE_SCORE: u32 = 500;
/// 骷髅刀单局在背包里的堆叠上限：拾取只进背包，结算时转库存。
pub(crate) const BLADE_STACK_MAX: usize = 10;
/// 骷髅刀对盾卫的穿透概率（%）：直接无视盾牌造成伤害，幸运符不提升。
pub(crate) const BLADE_PIERCE_CHANCE: u32 = 50;
/// 暗影弓：暗影射手死后 20% 掉落；带幸运符提升至 80%。
pub(crate) const BOW_DROP_PERCENT: u32 = 20;
pub(crate) const BOW_LUCKY_DROP_PERCENT: u32 = 80;
/// 装备暗影弓：攻击范围从 1 格变为 4 格（0级）或 10 格（1级）。
pub(crate) const BOW_RANGE: usize = 4;
pub(crate) const BOW_RANGE_LV1: usize = 10;
/// 装备暗影弓时攻击力折算系数（伤害 = 攻击力 / divisor）。
/// 0级：÷2；1级×3/4 = ÷4 后 ×3，实现为除以 4 再乘 3。
pub(crate) const BOW_DAMAGE_DIVISOR: i32 = 2;
pub(crate) const BOW_DAMAGE_MULT_NUMER: i32 = 3;
pub(crate) const BOW_DAMAGE_MULT_DENOM: i32 = 4;
/// 结算时一把暗影弓按多少积分结算（进基础分，随层数倍率与折半）。
pub(crate) const BOW_SCORE: u32 = 1000;
/// 暗影弓单局在背包里的堆叠上限：拾取只进背包，结算时转库存。
pub(crate) const BOW_STACK_MAX: usize = 10;
/// 盾卫自带破盾的耐久区间：比满耐久（SHIELD_DURABILITY）残破得多，挡几下就碎。
pub(crate) const GUARD_SHIELD_MIN: i32 = 5;
pub(crate) const GUARD_SHIELD_MAX: i32 = 20;
/// 盾卫攻击力上限：它的强项是举盾，输出比骷髅低一档。
pub(crate) const MAX_GUARD_ATK: i32 = 13;
/// 6-7 层守护者的远程暗箭：固定伤害 BOSS_DARK_ARROW_DMG，上下波动 1 点。
pub(crate) const BOSS_DARK_ARROW_DMG: i32 = 4;
/// 6-7 层守护者的锁定范围（格）：20 格外也能锁定玩家并射暗箭。
pub(crate) const BOSS_LOCK_RANGE: usize = 25;
/// 地牢共几层：杀掉本层守护者即可下楼，清完第 7 层才通关；结算倍率 = 当前层。
pub(crate) const MAX_FLOOR: u8 = 7;
/// 成就名：HUD 专门一行提示（基础/高级分行区分），结算时按成就给积分（见 BASIC/ADVANCED_ACH_SCORE）。
/// 简单成就
pub(crate) const ACH_KILL_ONE: &str = "杀死怪物";
pub(crate) const ACH_GET_SHIELD: &str = "拾取盾牌";
pub(crate) const ACH_NO_FEAR: &str = "不再畏惧";
pub(crate) const ACH_COMPENDIUM: &str = "怪物图鉴";
/// 高级成就
pub(crate) const ACH_FULLY_ARMED: &str = "全副武装";
pub(crate) const ACH_SHIELD_SWAP: &str = "除新迎旧";
pub(crate) const ACH_PINNED: &str = "两面夹击";
pub(crate) const ACH_FULL_LOAD: &str = "满载而归";
/// HUD 基础/高级成就行的固定展示顺序
pub(crate) const BASIC_ACHIEVEMENTS: [&str; 4] = [
    ACH_KILL_ONE,
    ACH_GET_SHIELD,
    ACH_NO_FEAR,
    ACH_COMPENDIUM,
];
pub(crate) const ADVANCED_ACHIEVEMENTS: [&str; 4] = [
    ACH_FULLY_ARMED,
    ACH_SHIELD_SWAP,
    ACH_PINNED,
    ACH_FULL_LOAD,
];
/// 结算时每个基础成就的固定积分（进基础分，随层数倍率与折半；结算在惩罚之前）。
pub(crate) const BASIC_ACH_SCORE: u32 = 100;
/// 结算时每个高级成就的固定积分（进基础分，随层数倍率与折半；结算在惩罚之前）。
pub(crate) const ADVANCED_ACH_SCORE: u32 = 500;
/// 八格成就全满（基础 4 + 高级 4）的额外奖励积分。
pub(crate) const FULL_ACH_BONUS: u32 = 1000;
/// 全副武装达成条件：血量 50/50、攻击 ≥16、血瓶满 5、盾牌满 45/45
pub(crate) const FULLY_ARMED_HP: i32 = 50;
pub(crate) const FULLY_ARMED_ATK: i32 = 16;
/// 不再畏惧：盾挡住射手的箭攒到几发达成
pub(crate) const NO_FEAR_ARROWS: usize = 3;
/// 守护者的名字（怪物图鉴要单列的 BOSS 种）
pub(crate) const BOSS: &str = "地牢守护者";
/// 全部怪物种类：怪物图鉴成就要每种杀一只（含 BOSS）
pub(crate) const ALL_SPECIES: [&str; 7] = ["哥布林", "巨鼠", "蜘蛛", SKELETON, SHIELD_GUARD, "暗影射手", BOSS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Behavior {
    Melee,
    Ranged,
    /// 6-7 层守护者：贴脸近战，站得远就射穿透盾牌的暗箭。
    Hybrid,
}

/// 一局结束状态：Voluntary/Victory/Defeat 时回商店结算。
/// 结算倍率 = 当前层（1层×1 … 7层×7），死亡和使用幸运符各折半一次。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum End {
    None,
    Voluntary, // 杀掉本层守护者后主动结束，按当前层倍率结算
    Victory,   // 清完第 7 层守护者，整局通关
    Defeat,    // 死了，按到达层结算但折半（安慰分）
}

#[derive(Clone)]
pub(crate) struct Entity {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) ch: char,
    pub(crate) name: &'static str,
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) atk: i32,
    pub(crate) sight: usize,
    pub(crate) behavior: Behavior,
    pub(crate) is_boss: bool,
    /// 怪物自带盾牌的耐久（只有盾卫 > 0）：挨打先扣这里的耐久，扣光才掉血。
    pub(crate) shield: i32,
}

pub(crate) struct Item {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) kind: ItemKind,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ItemKind {
    Heal,
    Manual, // 武林秘籍：参悟后攻击力 +100%
    QiPill, // 蕴气丹：耗 10 血，血量上限 +10
    Shield, // 盾牌：格挡怪物的攻击（骷髅与守护者暗箭除外），每次命中消耗 1 点耐久
    Bow, // 暗影弓：暗影射手 20% 掉落（幸运符 80%），装备后攻击范围 4 格、伤害 ÷2
}

/// 道具在地图上的显示字符。
pub(crate) fn item_glyph(kind: ItemKind) -> char {
    match kind {
        ItemKind::Heal => '+',
        ItemKind::Manual => 'M',
        ItemKind::QiPill => 'Q',
        ItemKind::Shield => 'S',
        ItemKind::Bow => 'B',
    }
}

/// 玩家永久库存的武器数量与等级：每局结算时把背包里的武器累计进来，长期收藏展示。
/// 库存当前仅供查看，不带进新一局（新局背包从空开始）。
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub(crate) struct Ownership {
    pub(crate) skeleton_blade: usize, // 库存里的骷髅刀数量
    pub(crate) shadow_bow: usize,     // 库存里的暗影弓数量
    pub(crate) blade_level: u8,       // 骷髅刀等级（0 或 1）
    pub(crate) bow_level: u8,         // 暗影弓等级（0 或 1）
}

impl Ownership {
    pub(crate) fn path() -> PathBuf {
        if let Ok(p) = std::env::var("WANDER_OWNED_FILE") {
            return PathBuf::from(p);
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wander")
            .join("ownership.json")
    }

    pub(crate) fn load() -> Self {
        let path = Self::path();
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Ownership::default()
        }
    }

    pub(crate) fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, serde_json::to_string_pretty(self).unwrap_or_default());
    }
}

/// 积分记录，持久化到本地目录（~/.config/wander/scores.json）。
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub(crate) struct Score {
    pub(crate) credits: u32, // 积分余额：通关赚得，开局商店可花
    pub(crate) total_score: u32, // 历史累计赚分
    pub(crate) last_score: u32,
    pub(crate) last_kills: u32,
    pub(crate) last_potions: usize,
    pub(crate) last_shields: usize,
    pub(crate) last_blades: usize, // 上一局结算时的骷髅刀数量
}

impl Score {
    pub(crate) fn path() -> PathBuf {
        // 测试可用 WANDER_SCORE_FILE 指到临时文件，别污染真实配置
        if let Ok(p) = std::env::var("WANDER_SCORE_FILE") {
            return PathBuf::from(p);
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wander")
            .join("scores.json")
    }

    pub(crate) fn load() -> Self {
        let path = Self::path();
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Score::default()
        }
    }

    pub(crate) fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, serde_json::to_string_pretty(self).unwrap_or_default());
    }

    pub(crate) fn add_run(&mut self, kills: u32, potions: usize, shields: usize, blades: usize, score: u32) -> u32 {
        // 积分 = (kills * 100 + 剩余血瓶 * 50 + 存量盾牌耐久折算 + 骷髅刀×500) × 层数倍率，死亡和幸运符各折半（见 Game::settlement_score）
        self.last_score = score;
        self.last_kills = kills;
        self.last_potions = potions;
        self.last_shields = shields;
        self.last_blades = blades;
        self.total_score += score;
        self.credits += score;
        self.save();
        score
    }
}

pub(crate) struct Game {
    pub(crate) map: Map,
    pub(crate) player: Entity,
    pub(crate) monsters: Vec<Entity>,
    pub(crate) items: Vec<Item>,
    pub(crate) potions: usize, // 背包里的血瓶，按 1 喝
    pub(crate) manuals: usize, // 背包里的武林秘籍，按 2 参悟
    pub(crate) qi_pills: usize, // 背包里的蕴气丹，按 3 服用
    pub(crate) shield: i32, // 手里盾牌的耐久，0 = 没有盾牌（有盾时格挡来袭伤害，命中 -1 耐久）
    pub(crate) shield_stock: Vec<i32>, // 背包里留存的盾牌，每面记下当时收进去的耐久（最多 SHIELD_STOCK_MAX 面；盾碎了自动取耐久最高的一面换上）
    pub(crate) blade_count: usize, // 背包里骷髅刀数量（最多 BLADE_STACK_MAX 把；拾取只进背包，结算转库存）
    pub(crate) blade_equipped: bool, // 当前手持骷髅刀（攻击固定 +BLADE_ATK_BONUS）
    pub(crate) blade_level: u8,      // 骷髅刀等级（0 或 1）
    pub(crate) bow_count: usize, // 背包里暗影弓数量（最多 BOW_STACK_MAX 把；拾取只进背包，结算转库存）
    pub(crate) bow_equipped: bool, // 当前手持暗影弓（攻击范围 BOW_RANGE 格、伤害 ÷BOW_DAMAGE_DIVISOR）
    pub(crate) bow_level: u8,      // 暗影弓等级（0 或 1）
    pub(crate) weapon_menu: bool, // 武器切换窗口打开着（模态，不消耗回合）
    pub(crate) kills: u32,
    pub(crate) floor: u8, // 当前层 1..=MAX_FLOOR
    pub(crate) boss_down: bool, // 本层守护者已倒（可下楼，或主动结束按当前层结算）
    pub(crate) end: End, // 本局结束状态：!= None 时显示结算界面
    pub(crate) credits: u32, // 积分余额（存档缓存，结算后刷新）
    pub(crate) lucky: bool, // 本局用了幸运符：血瓶掉率 90%，秘籍 25% 概率攻击 ×4，盾牌必掉，结算积分折半
    pub(crate) achievements: HashSet<&'static str>, // 本局达成的成就（仅图一乐，HUD 基础/高级分行展示，随新局重置）
    pub(crate) arrows_blocked: usize, // 本局盾挡下的射手暗箭总数（不再畏惧成就攒到 NO_FEAR_ARROWS 达成）
    pub(crate) species_killed: HashSet<&'static str>, // 本局击杀过的怪物种类（怪物图鉴成就用）
    pub(crate) msg: String,
    pub(crate) vw: usize, // 实际渲染宽度（受终端窗口限制）
    pub(crate) vh: usize, // 实际渲染高度（受终端窗口限制）
}

/// 开局商店购得的物品，带到新一局里。
#[derive(Clone, Copy, Default)]
pub(crate) struct Purchase {
    pub(crate) manual: bool, // 武林秘籍
    pub(crate) talisman: bool, // 幸运符
    pub(crate) potions: usize, // 血瓶
}

impl Game {
    /// 应用开局商店的购买：秘籍/幸运符/血瓶直接进背包或生效。
    pub(crate) fn apply_purchase(&mut self, p: &Purchase) {
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

    pub(crate) fn new(vw: usize, vh: usize, ownership: &Ownership) -> Self {
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
                shield: 0,
            },
            monsters,
            items,
            potions: 0,
            manuals: 0,
            qi_pills: 0,
            shield: 0,
            shield_stock: Vec::new(),
            // 背包本局从空开始，但库存里有的武器会「带上一件可用的」：开局自动装备，卸下后也能随时装回。
            // 库存剩余的数量仍只作收藏展示，不带进本局背包（本局拾取才继续往上叠，受堆叠上限约束）。
            // 开局装备的武器等级继承自库存。
            blade_count: (ownership.skeleton_blade > 0) as usize,
            blade_equipped: ownership.shadow_bow == 0 && ownership.skeleton_blade > 0,
            blade_level: ownership.blade_level,
            bow_count: (ownership.shadow_bow > 0) as usize,
            bow_equipped: ownership.shadow_bow > 0,
            bow_level: ownership.bow_level,
            weapon_menu: false,
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
                "地牢共 7 层，越深越难。h/j/k/l 移动，按 a 攻击八方向 1 格内的怪物。杀掉本层守护者 D 后按 d 下楼或 g 结算。+ 血瓶 / M 武林秘籍 / Q 蕴气丹 / S 盾牌 / † 骷髅刀（骷髅掉落）/ B 暗影弓（暗影射手 20% 掉落，幸运符 80%）。w 切换武器，菜单里可升级武器（需 20 把同类武器）。6-7 层骷髅绝迹，近战换成举破盾的盾卫 t，守护者 D 还会在 20 格外射透盾的暗箭"
            ),
            vw: vw.min(W),
            // 5 行头部（标题/物品/工具/基础成就/高级成就）+ 地图 + 1 行消息 + 1 行按键提示
            vh: (vh.saturating_sub(7)).min(H),
        };
        game.map.recompute_vision(game.player.x, game.player.y, 8);
        game
    }

    /// 本局结束后把背包里的武器数量累计进永久库存（原来没有就当作从 0 开始累加）。
    pub(crate) fn save_ownership(&self) {
        let mut ownership = Ownership::load();
        ownership.skeleton_blade += self.blade_count;
        ownership.shadow_bow += self.bow_count;
        ownership.blade_level = self.blade_level;
        ownership.bow_level = self.bow_level;
        ownership.save();
    }

    pub(crate) fn try_move_player(&mut self, dx: i32, dy: i32) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return; // 死了或本局结束了不能动，只能 g 回商店
        }
        let nx = self.player.x as i32 + dx;
        let ny = self.player.y as i32 + dy;
        if nx < 0 || ny < 0 || nx >= W as i32 || ny >= H as i32 {
            return;
        }
        let (nx, ny) = (nx as usize, ny as usize);

        // 撞到怪物 -> 不能走过去，自动对范围内怪物攻击一次（等价于按 a）
        if self.monsters.iter().any(|m| m.x == nx && m.y == ny) {
            self.try_attack();
            return;
        }

        if self.map.is_walkable(nx, ny) {
            self.player.x = nx;
            self.player.y = ny;
            self.pickup();
            self.end_turn();
        }
    }

    /// 击杀结算（共用）：血瓶、盾牌、骷髅刀、暗影弓掉落 + 成就记账。
    /// 调用前提：怪物 hp 已 <= 0，尚未从 monsters 移除。
    pub(crate) fn settle_kill(&mut self, m: &Entity, rng: &mut impl RngExt) {
        self.kills += 1;
        // 血瓶：幸运符 90%，默认 1/2
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
        // 骷髅死后：20% 掉骷髅刀，带幸运符提升至 80%；只进背包不自动装备，叠到上限后不再捡。
        if m.name == SKELETON {
            let blade_drop = if self.lucky {
                rng.random_range(0..100) < BLADE_LUCKY_DROP_PERCENT
            } else {
                rng.random_range(0..100) < BLADE_DROP_PERCENT
            };
            if blade_drop {
                if self.blade_count >= BLADE_STACK_MAX {
                    self.msg.push_str(&format!(
                        "（骷髅掉落了骷髅刀，但背包已满 {}/{}，无法拾取）",
                        self.blade_count, BLADE_STACK_MAX
                    ));
                } else {
                    self.blade_count += 1;
                    self.msg.push_str(&format!(
                        "（骷髅掉落了骷髅刀，收入背包 {}/{}，按 w 装备）",
                        self.blade_count, BLADE_STACK_MAX
                    ));
                }
            }
        }
        // 暗影射手死后：20% 掉暗影弓，带幸运符提升至 80%；掉落在地上，走过去拾取自动装备
        if m.name == "暗影射手" {
            let bow_drop = if self.lucky {
                rng.random_range(0..100) < BOW_LUCKY_DROP_PERCENT
            } else {
                rng.random_range(0..100) < BOW_DROP_PERCENT
            };
            if bow_drop {
                self.items.push(Item { x: m.x, y: m.y, kind: ItemKind::Bow });
                self.msg.push_str("（暗影射手掉落了一把暗影弓）");
            }
        }
        self.unlock(ACH_KILL_ONE); // 杀死怪物：击杀一只即达成
        self.register_kill(m.name); // 怪物图鉴：记账击杀过的种类
    }

    /// 按 a：范围攻击。默认对前后左右 + 斜方向共 8 个方向、距离 1 格内的所有怪物各造成一次伤害；
    /// 装备暗影弓后范围变为 BOW_RANGE 格（同样 8 方向方形范围），伤害 ÷2。
    /// 攻击不位移，但消耗一个回合（怪物会行动）。
    pub(crate) fn try_attack(&mut self) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return; // 死了或本局结束了不能攻击
        }
        let range = self.attack_range();
        let (px, py) = (self.player.x, self.player.y);
        let atk = self.attack_power();
        let mut rng = rand::rng();
        // 切比雪夫距离 <= range（8 方向方形范围），排除自己所在格
        let target_idx: Vec<usize> = self
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                let dx = (m.x as i32 - px as i32).unsigned_abs();
                let dy = (m.y as i32 - py as i32).unsigned_abs();
                dx <= range as u32 && dy <= range as u32
            })
            .map(|(i, _)| i)
            .collect();
        if target_idx.is_empty() {
            self.msg = format!("{} 范围内没有可以攻击的怪物", self.attack_range());
            self.end_turn();
            return;
        }
        let mut hits = 0u32;
        // 逆序遍历，保证 remove 后前面索引依然有效
        for i in target_idx.into_iter().rev() {
            let m = self.monsters[i].clone();
            // 骷髅刀对盾卫有 50% 概率直接穿透盾牌造成伤害（幸运符不提升）
            let pierce = self.blade_equipped
                && m.name == SHIELD_GUARD
                && rng.random_range(0..100) < BLADE_PIERCE_CHANCE;
            // 盾卫举着的破盾：先啃盾再掉血，每次命中扣它 1 点耐久（扣光才打得到肉）
            if self.monsters[i].shield > 0 && !pierce {
                self.monsters[i].shield -= 1;
                hits += 1;
                let left = self.monsters[i].shield;
                self.msg = if left > 0 {
                    format!("{}举盾挡下你的攻击（它的盾牌耐久 {}）", m.name, left)
                } else {
                    format!("{}的盾牌被击碎了！", m.name)
                };
            } else if pierce {
                hits += 1;
                self.msg = format!("骷髅刀穿透了{}的盾牌！", m.name);
            } else {
                let raw = roll(&mut rng, atk);
                // 暗影弓伤害减半（0级 ÷2），1级 ×3/4（÷4 再 ×3），但至少 1 点（否则打不到 1 血怪 feels like "弓用不了"）
                let dmg = if self.bow_equipped && self.bow_level >= 1 {
                    (raw * BOW_DAMAGE_MULT_NUMER / BOW_DAMAGE_MULT_DENOM).max(1)
                } else if self.bow_equipped {
                    (raw / BOW_DAMAGE_DIVISOR).max(1)
                } else {
                    raw.max(1)
                };
                self.monsters[i].hp -= dmg;
                hits += 1;
                if self.monsters[i].hp <= 0 {
                    let name = m.name;
                    self.msg = format!("你击杀了{}！", name);
                    if m.is_boss {
                        self.boss_down = true;
                        if self.floor == MAX_FLOOR {
                            self.end = End::Victory; // 第 7 层守护者倒下：整局通关，结算落盘由 run 循环统一做
                        } else {
                            self.msg.push_str(&format!(
                                " 第{}层的守护者倒下了！按 d 下到第{}层（怪物更强、射手更多），按 g 结束本局回商店结算（积分 ×{}）",
                                self.floor,
                                self.floor + 1,
                                self.floor
                            ));
                        }
                    }
                    self.settle_kill(&m, &mut rng);
                    self.monsters.remove(i);
                }
            }
        }
        if hits > 1 {
            self.msg = format!("你攻击了{}只怪物！", hits);
        }
        self.end_turn();
    }

    /// 当前攻击范围（格）：空手 1 格，骷髅刀 0 级 1 格 / 1 级 2 格，暗影弓 0 级 BOW_RANGE 格 / 1 级 BOW_RANGE_LV1 格。
    pub(crate) fn attack_range(&self) -> usize {
        if self.bow_equipped {
            if self.bow_level >= 1 { BOW_RANGE_LV1 } else { BOW_RANGE }
        } else if self.blade_equipped {
            if self.blade_level >= 1 { BLADE_RANGE_LV1 } else { BLADE_RANGE }
        } else {
            1
        }
    }

    /// 踩到道具：不立刻生效，先揣进背包，按 1/2/3 再用；Bow 直接自动装备。
    pub(crate) fn pickup(&mut self) {
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
                ItemKind::Bow => {
                    if self.bow_count >= BOW_STACK_MAX {
                        self.msg = format!(
                            "背包暗影弓已满（{}/{}），这把留在原地",
                            self.bow_count, BOW_STACK_MAX
                        );
                        return;
                    }
                    self.bow_count += 1;
                    self.items.remove(i);
                    self.msg = format!(
                        "捡起暗影弓（背包 {}/{}），按 w 装备",
                        self.bow_count, BOW_STACK_MAX
                    );
                }
            }
        }
    }

    /// 按 1：喝一瓶，满血封顶，不消耗回合（怪物不会因此行动）。
    pub(crate) fn use_potion(&mut self) {
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
    pub(crate) fn use_manual(&mut self) {
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
    pub(crate) fn use_qi_pill(&mut self) {
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

    /// 按 w：切换武器（空手 / 骷髅刀 / 暗影弓），模态窗口选择，不消耗回合。
    pub(crate) fn toggle_weapon_menu(&mut self) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return; // 死了或本局结算了不能切换武器
        }
        self.weapon_menu = !self.weapon_menu;
        if self.weapon_menu {
            self.msg = "切换武器：[1] 空手  [2] 骷髅刀（如有）  [3] 暗影弓（如有）  [4] 升级骷髅刀（需 20 把）  [5] 升级暗影弓（需 20 把）  按 w 关闭".to_string();
        } else {
            self.msg = "关闭武器菜单".to_string();
        }
    }

    /// 装备骷髅刀：攻击固定 +BLADE_ATK_BONUS，升级后 +BLADE_ATK_BONUS_LV1，不消耗回合。
    pub(crate) fn equip_blade(&mut self) {
        if self.blade_count == 0 {
            self.msg = "背包里没有骷髅刀".to_string();
            return;
        }
        if self.blade_equipped {
            self.msg = "你已经装备了骷髅刀".to_string();
            return;
        }
        self.blade_equipped = true;
        self.bow_equipped = false; // 弓与刀互斥
        self.weapon_menu = false;
        let bonus = if self.blade_level >= 1 { BLADE_ATK_BONUS_LV1 } else { BLADE_ATK_BONUS };
        self.msg = format!("装备骷髅刀！攻击力 {} → {}（+{}）", self.player.atk, self.player.atk + bonus, bonus);
    }

    /// 卸下骷髅刀：换回空手，不消耗回合。
    pub(crate) fn unequip_blade(&mut self) {
        self.blade_equipped = false;
        self.bow_equipped = false; // 弓与刀互斥，收刀即也收弓
        self.weapon_menu = false;
        if self.blade_count == 0 {
            self.msg = "你已经没有装备骷髅刀".to_string();
        } else {
            self.msg = format!("卸下骷髅刀！攻击力 {} → {}", self.attack_power(), self.player.atk);
        }
    }

    /// 菜单里按 [1] 换回空手：收掉当前手持的那件（刀或弓），只报一条消息。
    pub(crate) fn switch_to_fists(&mut self) {
        if self.blade_equipped {
            self.unequip_blade();
        } else if self.bow_equipped {
            self.unequip_bow();
        } else {
            self.msg = "你已经是空手了".to_string();
        }
    }

    /// 装备暗影弓：攻击范围 BOW_RANGE 格（1级 BOW_RANGE_LV1）、伤害÷2（0级）或 ×3/4（1级），不消耗回合。
    pub(crate) fn equip_bow(&mut self) {
        if self.bow_count == 0 {
            self.msg = "背包里没有暗影弓".to_string();
            return;
        }
        if self.bow_equipped {
            self.msg = "你已经装备了暗影弓".to_string();
            return;
        }
        self.bow_equipped = true;
        self.blade_equipped = false; // 弓与刀互斥
        self.weapon_menu = false;
        if self.bow_level >= 1 {
            self.msg = format!(
                "装备暗影弓（+1 级）！攻击范围 {} → {} 格，伤害 ÷{} → ×{}/{}（当前攻击力 {}）",
                BOW_RANGE,
                BOW_RANGE_LV1,
                BOW_DAMAGE_DIVISOR,
                BOW_DAMAGE_MULT_NUMER,
                BOW_DAMAGE_MULT_DENOM,
                self.attack_power()
            );
        } else {
            self.msg = format!(
                "装备暗影弓！攻击范围 {} → {} 格，伤害 ÷{}（当前攻击力 {}）",
                1,
                BOW_RANGE,
                BOW_DAMAGE_DIVISOR,
                self.attack_power()
            );
        }
    }

    /// 卸下暗影弓：换回空手，不消耗回合。
    pub(crate) fn unequip_bow(&mut self) {
        if !self.bow_equipped {
            self.msg = "你已经没有装备暗影弓".to_string();
            return;
        }
        self.bow_equipped = false;
        self.weapon_menu = false;
        self.msg = format!("卸下暗影弓！攻击范围 {} → 1 格，伤害恢复", BOW_RANGE_LV1.max(BOW_RANGE));
    }

    /// 按菜单 [4] 升级骷髅刀：消耗 20 把骷髅刀，将等级提升到 1（仅此一次）。
    pub(crate) fn upgrade_blade(&mut self) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return;
        }
        if self.blade_level >= 1 {
            self.msg = "骷髅刀已经升到最高等级了".to_string();
            return;
        }
        if self.blade_count < UPGRADE_COST {
            self.msg = format!("升级骷髅刀需要 {} 把，背包里只有 {} 把", UPGRADE_COST, self.blade_count);
            return;
        }
        self.blade_count -= UPGRADE_COST;
        self.blade_level = 1;
        self.msg = format!(
            "骷髅刀升级！耗费 {} 把，攻击范围 1 → {} 格，伤害加成 {} → {}",
            UPGRADE_COST, BLADE_RANGE_LV1, BLADE_ATK_BONUS, BLADE_ATK_BONUS_LV1
        );
        // 同步到库存等级（下局也继承）
        let mut ownership = Ownership::load();
        ownership.blade_level = self.blade_level;
        ownership.save();
    }

    /// 按菜单 [5] 升级暗影弓：消耗 20 把暗影弓，将等级提升到 1（仅此一次）。
    pub(crate) fn upgrade_bow(&mut self) {
        if self.player.hp <= 0 || !matches!(self.end, End::None) {
            return;
        }
        if self.bow_level >= 1 {
            self.msg = "暗影弓已经升到最高等级了".to_string();
            return;
        }
        if self.bow_count < UPGRADE_COST {
            self.msg = format!("升级暗影弓需要 {} 把，背包里只有 {} 把", UPGRADE_COST, self.bow_count);
            return;
        }
        self.bow_count -= UPGRADE_COST;
        self.bow_level = 1;
        self.msg = format!(
            "暗影弓升级！耗费 {} 把，攻击范围 {} → {} 格，伤害惩罚 ÷{} → ×{}/{}",
            UPGRADE_COST, BOW_RANGE, BOW_RANGE_LV1, BOW_DAMAGE_DIVISOR, BOW_DAMAGE_MULT_NUMER, BOW_DAMAGE_MULT_DENOM
        );
        // 同步到库存等级（下局也继承）
        let mut ownership = Ownership::load();
        ownership.bow_level = self.bow_level;
        ownership.save();
    }

    /// 计算当前攻击力：基础攻击 + 骷髅刀加成（如果装备）；装备暗影弓时再 ÷2（0级）或 ×3/4（1级）。
    pub(crate) fn attack_power(&self) -> i32 {
        let mut base = self.player.atk;
        if self.blade_equipped {
            base += if self.blade_level >= 1 { BLADE_ATK_BONUS_LV1 } else { BLADE_ATK_BONUS };
        }
        if self.bow_equipped && self.bow_level >= 1 {
            base * BOW_DAMAGE_MULT_NUMER / BOW_DAMAGE_MULT_DENOM
        } else if self.bow_equipped {
            base / BOW_DAMAGE_DIVISOR
        } else {
            base
        }
    }

    /// 处理怪物：能看到玩家就按各自行为追击/射击，否则原地随机游荡。
    pub(crate) fn update_monsters(&mut self) {
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
                            let r = block_with_shield(&mut self.shield, &mut self.shield_stock, m.name, "攻击", false, &mut hits);
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
                // 6-7 层守护者：锁定范围远超普通怪（20 格外也能看到玩家），
                // 远程射出的暗箭穿透盾牌（固定伤害 BOSS_DARK_ARROW_DMG、波动 1）；贴脸则抡近战。
                Behavior::Hybrid => {
                    if dist >= MIN_RANGED_RANGE {
                        // 站定射暗箭（25% 射偏）
                        if rng.random_range(0..4) == 0 {
                            hits.push(format!("{}的暗箭从{dist}格外飞来，擦身而过", m.name));
                        } else {
                            // 暗箭穿盾：不扣玩家盾牌耐久，直接掉血
                            let r = block_with_shield(
                                &mut self.shield,
                                &mut self.shield_stock,
                                m.name,
                                "暗箭",
                                true,
                                &mut hits,
                            );
                            if r.refilled {
                                shield_refill = true;
                            }
                            if !r.blocked {
                                let dmg = BOSS_DARK_ARROW_DMG + rng.random_range(-1..=1);
                                self.player.hp -= dmg;
                                hits.push(format!(
                                    "{}在{dist}格外锁定你，暗箭穿透盾牌射中{}血（剩余{}）",
                                    m.name, dmg, self.player.hp
                                ));
                            }
                        }
                    } else if dist <= 1 {
                        // 贴身：无法射箭，改为近战猛击（25% 落空）
                        if rng.random_range(0..4) == 0 {
                            hits.push(format!("{}抡了个空", m.name));
                        } else {
                            let r = block_with_shield(&mut self.shield, &mut self.shield_stock, m.name, "重击", false, &mut hits);
                            boss_hit = true;
                            if r.refilled {
                                shield_refill = true;
                            }
                            if !r.blocked {
                                let dmg = roll(&mut rng, m.atk);
                                self.player.hp -= dmg;
                                hits.push(format!("{}近身重击砸了你{}血（剩余{}）", m.name, dmg, self.player.hp));
                            }
                        }
                    } else {
                        // 2-3 格：射不出箭也懒得后退，直接逼近玩家准备抡人
                        let (step_x, step_y) = (
                            (px as i32 - m.x as i32).signum(),
                            (py as i32 - m.y as i32).signum(),
                        );
                        let nx = m.x as i32 + step_x;
                        let ny = m.y as i32 + step_y;
                        if let Some((ux, uy)) = try_move(&self.map, &occupied, px, py, nx, ny) {
                            m.x = ux;
                            m.y = uy;
                        }
                    }
                }
                Behavior::Ranged => {
                    if dist >= MIN_RANGED_RANGE {
                        // 站定射击（25% 射偏，射偏不扣耐久）
                        if rng.random_range(0..4) == 0 {
                            hits.push(format!("{}的暗影箭擦身而过", m.name));
                        } else {
                            let r = block_with_shield(&mut self.shield, &mut self.shield_stock, m.name, "暗影箭", false, &mut hits);
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
                        let r = block_with_shield(&mut self.shield, &mut self.shield_stock, m.name, "暗影一击", false, &mut hits);
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

    pub(crate) fn end_turn(&mut self) {
        self.update_monsters();
        self.map.recompute_vision(self.player.x, self.player.y, 8);
        self.check_fully_armed(); // 状态更新：回合结束，看看是否全副武装
    }

    /// 下楼：杀掉本层守护者后按 d，生成下一层地图（怪物更强、射手更多）。
    pub(crate) fn descend(&mut self) {
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
            "你下到第{}/{}层：怪物更强了，射手也更多。杀掉深处的守护者继续向下",
            self.floor, MAX_FLOOR
        );
        self.map.recompute_vision(self.player.x, self.player.y, 8);
    }

    /// 背包里存量盾牌的积分合计：每面按「当时收进去的耐久 × SHIELD_SCORE_PER_POINT」折算。
    pub(crate) fn shield_stock_score(&self) -> u32 {
        self.shield_stock
            .iter()
            .map(|d| *d as u32 * SHIELD_SCORE_PER_POINT)
            .sum()
    }

    /// 骷髅刀积分：背包里每把 500 分。
    pub(crate) fn blade_score(&self) -> u32 {
        self.blade_count as u32 * BLADE_SCORE
    }

    /// 暗影弓积分：背包里每把 1000 分。
    pub(crate) fn bow_score(&self) -> u32 {
        self.bow_count as u32 * BOW_SCORE
    }

    /// 成就积分：每个基础成就 100、每个高级成就 500；八格全满再 +1000。
    /// 进基础分，随层数倍率与折半（结算在惩罚之前）。
    pub(crate) fn achievement_score(&self) -> u32 {
        let basic = self.unlocked_achievements(&BASIC_ACHIEVEMENTS).len() as u32;
        let advanced = self.unlocked_achievements(&ADVANCED_ACHIEVEMENTS).len() as u32;
        let mut score = basic * BASIC_ACH_SCORE + advanced * ADVANCED_ACH_SCORE;
        if basic == BASIC_ACHIEVEMENTS.len() as u32
            && advanced == ADVANCED_ACHIEVEMENTS.len() as u32
        {
            score += FULL_ACH_BONUS;
        }
        score
    }

    /// 八格成就是否全满（基础 4 + 高级 4 都达成）。
    pub(crate) fn all_achievements_unlocked(&self) -> bool {
        let total = BASIC_ACHIEVEMENTS.len() + ADVANCED_ACHIEVEMENTS.len();
        self.achievements.len() == total
            && BASIC_ACHIEVEMENTS.iter().all(|a| self.achievements.contains(a))
            && ADVANCED_ACHIEVEMENTS.iter().all(|a| self.achievements.contains(a))
    }

    /// 本局通过的关数：之前各层都算通过；当前层守护者倒下（或整局通关）再 +1。
    pub(crate) fn floors_cleared(&self) -> u32 {
        let current = self.boss_down || matches!(self.end, End::Victory);
        (self.floor as u32 - 1) + current as u32
    }

    /// 通关奖励：每通过一关 +500 积分。
    pub(crate) fn clear_bonus(&self) -> u32 {
        self.floors_cleared() * CLEAR_BONUS_PER_FLOOR
    }

    /// 本局结算分：(击杀 × 100 + 剩余血瓶 × 50 + 存量盾牌耐久折算 + 骷髅刀×500 + 暗影弓×1000 + 成就积分)
    /// × 当前层倍率；死亡、幸运符各折半一次（成就积分在惩罚之前进基础分，一并折半），
    /// 最后再加上固定的通关奖励（每关 +500，不折半）。
    pub(crate) fn settlement_score(&self) -> u32 {
        let base = self.kills * 100
            + self.potions as u32 * 50
            + self.shield_stock_score()
            + self.blade_score()
            + self.bow_score()
            + self.achievement_score();
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
    pub(crate) fn can_restart(&self) -> bool {
        self.player.hp <= 0 || self.boss_down
    }

    /// 解锁成就：首次记入本局达成集合，重复解锁是空操作；结算时按达成数给积分（见 achievement_score）。
    pub(crate) fn unlock(&mut self, name: &'static str) {
        let _ = self.achievements.insert(name);
    }

    /// 全副武装：血量 50/50、攻击 ≥16、血瓶满 5、盾牌满 45/45。
    /// 任何状态更新（移动/待机回合结束、喝药、参悟、服丹）后调一次。
    pub(crate) fn check_fully_armed(&mut self) {
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
    pub(crate) fn unlocked_achievements(&self, list: &[&'static str]) -> Vec<&'static str> {
        list.iter().copied().filter(|a| self.achievements.contains(a)).collect()
    }

    /// 记一刀击杀的怪物种类（怪物图鉴进度）：ALL_SPECIES 每种都杀过就解锁。
    pub(crate) fn register_kill(&mut self, name: &'static str) {
        let _ = self.species_killed.insert(name);
        if ALL_SPECIES.iter().all(|s| self.species_killed.contains(s)) {
            self.unlock(ACH_COMPENDIUM);
        }
    }

    pub(crate) fn render(&self) -> String {
        let mut out = String::new();
        // ANSI 颜色代码：\x1b[3Xm
        const COLOR_PLAYER: &str = "\x1b[32m";  // 绿色
        const COLOR_MONSTER: &str = "\x1b[31m"; // 红色
        const COLOR_BOSS: &str = "\x1b[38;5;208m"; // 橙色（ANSI 208）
        const COLOR_ITEM_HEAL: &str = "\x1b[33m"; // 黄色
        const COLOR_ITEM_MANUAL: &str = "\x1b[36m"; // 青色
        const COLOR_ITEM_QI: &str = "\x1b[95m";   // 亮紫色
        const COLOR_ITEM_SHIELD: &str = "\x1b[94m"; // 亮蓝色
        const COLOR_ITEM_BLADE: &str = "\x1b[38;5;214m"; // 橙黄色（ANSI 214）
        const COLOR_ITEM_BOW: &str = "\x1b[38;5;202m"; // 金黄色（ANSI 202）
        const COLOR_RESET: &str = "\x1b[0m";
        const COLOR_GOLD: &str = "\x1b[33m"; // 金色用于结算

        if self.weapon_menu && matches!(self.end, End::None) {
            self.render_weapon_menu(&mut out, COLOR_ITEM_BLADE, COLOR_ITEM_BOW, COLOR_RESET);
            return out;
        }

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
            "物品：血瓶 x{}  秘籍 x{}  蕴气丹 x{}  盾牌存量 x{}/{}  骷髅刀 x{}/{}  暗影弓 x{}/{}  积分 {}\r\n",
            self.potions,
            self.manuals,
            self.qi_pills,
            self.shield_stock.len(),
            SHIELD_STOCK_MAX,
            self.blade_count,
            BLADE_STACK_MAX,
            self.bow_count,
            BOW_STACK_MAX,
            self.credits,
        ));
        // 第三行：工具栏（盾牌及耐久 + 武器）
        let weapon_desc = if self.bow_equipped {
            if self.bow_level >= 1 {
                "暗影弓+1".to_string()
            } else {
                "暗影弓".to_string()
            }
        } else if self.blade_equipped {
            let bonus = if self.blade_level >= 1 { BLADE_ATK_BONUS_LV1 } else { BLADE_ATK_BONUS };
            let rng = if self.blade_level >= 1 { BLADE_RANGE_LV1 } else { BLADE_RANGE };
            if self.blade_level >= 1 {
                format!("骷髅刀+1（范围{}、+{}攻击）", rng, bonus)
            } else {
                format!("骷髅刀（范围{}、+{}攻击）", rng, bonus)
            }
        } else {
            "空手（范围1）".to_string()
        };
        let weapon_color = if self.bow_equipped {
            COLOR_ITEM_BOW
        } else if self.blade_equipped {
            COLOR_ITEM_BLADE
        } else {
            COLOR_RESET
        };
        if self.shield > 0 {
            out.push_str(&format!(
                "{}盾牌 {}/{}  武器：{}{}{COLOR_RESET}\r\n",
                COLOR_ITEM_SHIELD,
                self.shield,
                SHIELD_DURABILITY,
                weapon_color,
                weapon_desc,
            ));
        } else {
            out.push_str(&format!(
                "盾牌：无  武器：{}{}{COLOR_RESET}\r\n",
                weapon_color,
                weapon_desc,
            ));
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
                        ItemKind::Bow => COLOR_ITEM_BOW,
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
            "{}\r\nq 退出  g 结束本局/回商店（死亡或守护者倒下后）  d 下楼（守护者倒下后）  w 切换武器  a 攻击  1 血瓶  2 秘籍  3 蕴气丹  空格待机\r\n",
            self.msg
        ));
        out
    }

    pub(crate) fn render_weapon_menu(&self, out: &mut String, blade_color: &str, bow_color: &str, reset: &str) {
        out.push_str(&format!("切换武器（按 w 关闭）\r\n\r\n"));
        let has_blade = self.blade_count > 0;
        let has_bow = self.bow_count > 0;
        let eq_blade = self.blade_equipped;
        let eq_bow = self.bow_equipped;
        let check = |b: bool| if b { "[x]" } else { "[ ]" };
        out.push_str(&format!(
            "{} [1] 空手（范围 1） {}\r\n",
            check(!eq_blade && !eq_bow),
            if !eq_blade && !eq_bow { "(当前)" } else { "" }
        ));
        let blade_bonus = if self.blade_level >= 1 { BLADE_ATK_BONUS_LV1 } else { BLADE_ATK_BONUS };
        let blade_r = if self.blade_level >= 1 { BLADE_RANGE_LV1 } else { BLADE_RANGE };
        out.push_str(&format!(
            "{}{} [2] 骷髅刀 x{} (+{} 攻击，范围 {}) {}{}\r\n",
            if has_blade { blade_color } else { "\x1b[90m" }, // 灰色表示没有
            check(eq_blade),
            self.blade_count,
            blade_bonus,
            blade_r,
            if eq_blade { "(当前)" } else { if !has_blade { "(未拾取)" } else { "" } },
            reset
        ));
        let bow_div = if self.bow_level >= 1 {
            format!("×{}/{}", BOW_DAMAGE_MULT_NUMER, BOW_DAMAGE_MULT_DENOM)
        } else {
            format!("÷{}", BOW_DAMAGE_DIVISOR)
        };
        let bow_r = if self.bow_level >= 1 { BOW_RANGE_LV1 } else { BOW_RANGE };
        out.push_str(&format!(
            "{}{} [3] 暗影弓 x{}（范围 {}、伤害{}） {}{}\r\n",
            if has_bow { bow_color } else { "\x1b[90m" },
            check(eq_bow),
            self.bow_count,
            bow_r,
            bow_div,
            if eq_bow { "(当前)" } else { if !has_bow { "(未拾取)" } else { "" } },
            reset
        ));
        // 升级选项 [4] 和 [5]
        let blade_upgradable = self.blade_count >= UPGRADE_COST && self.blade_level < 1;
        let blade_status = if self.blade_level >= 1 { "(已升级)" } else { "" };
        out.push_str(&format!(
            "{} [4] 升级骷髅刀 x{}/{} → 1 级 (范围 {}→{}, +{}→{}) {}{}\r\n",
            if self.blade_level >= 1 { "\x1b[90m" } else { if blade_upgradable { "\x1b[33m" } else { "\x1b[90m" } },
            self.blade_count,
            UPGRADE_COST,
            BLADE_RANGE,
            BLADE_RANGE_LV1,
            BLADE_ATK_BONUS,
            BLADE_ATK_BONUS_LV1,
            blade_status,
            reset
        ));
        let bow_upgradable = self.bow_count >= UPGRADE_COST && self.bow_level < 1;
        let bow_status = if self.bow_level >= 1 { "(已升级)" } else { "" };
        out.push_str(&format!(
            "{} [5] 升级暗影弓 x{}/{} → 1 级 (范围 {}→{}, ÷{}→×{}/{}) {}{}\r\n",
            if self.bow_level >= 1 { "\x1b[90m" } else { if bow_upgradable { "\x1b[33m" } else { "\x1b[90m" } },
            self.bow_count,
            UPGRADE_COST,
            BOW_RANGE,
            BOW_RANGE_LV1,
            BOW_DAMAGE_DIVISOR,
            BOW_DAMAGE_MULT_NUMER,
            BOW_DAMAGE_MULT_DENOM,
            bow_status,
            reset
        ));
        out.push_str(&format!("\r\n{}\r\n", self.msg));
    }

    pub(crate) fn render_settlement(&self, out: &mut String, gold: &str, reset: &str) {
        let halved = matches!(self.end, End::Defeat);
        let mult = self.floor as u32;
        let shield_pts = self.shield_stock_score();
        let blade_pts = self.blade_score();
        let bow_pts = self.bow_score();
        let basic_count = self.unlocked_achievements(&BASIC_ACHIEVEMENTS).len();
        let advanced_count = self.unlocked_achievements(&ADVANCED_ACHIEVEMENTS).len();
        let basic_pts = basic_count as u32 * BASIC_ACH_SCORE;
        let advanced_pts = advanced_count as u32 * ADVANCED_ACH_SCORE;
        let ach_pts = self.achievement_score(); // 基础 + 高级 + 八格全满奖励
        let full = (self.kills * 100
            + self.potions as u32 * 50
            + shield_pts
            + blade_pts
            + bow_pts
            + ach_pts)
            * mult;
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
        if self.blade_count > 0 {
            put(out, &row(&ledger(
                &format!("骷髅刀 {} 把", self.blade_count),
                &format!("{} 积分", blade_pts),
                INNER,
            )));
        }
        if self.bow_count > 0 {
            let bow_desc = if self.bow_level >= 1 {
                "暗影弓+1"
            } else {
                "暗影弓"
            };
            put(out, &row(&ledger(
                &format!("{} {} 把", bow_desc, self.bow_count),
                &format!("{} 积分", bow_pts),
                INNER,
            )));
        }
        if basic_count > 0 {
            put(out, &row(&ledger(
                &format!("基础成就 {} 个 × {}", basic_count, BASIC_ACH_SCORE),
                &format!("{} 积分", basic_pts),
                INNER,
            )));
        }
        if advanced_count > 0 {
            put(out, &row(&ledger(
                &format!("高级成就 {} 个 × {}", advanced_count, ADVANCED_ACH_SCORE),
                &format!("{} 积分", advanced_pts),
                INNER,
            )));
        }
        if self.all_achievements_unlocked() {
            put(out, &row(&ledger(
                &format!("八格成就全满 × {}", FULL_ACH_BONUS),
                &format!("+{} 积分", FULL_ACH_BONUS),
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
        put(out, &row(&centered("按 g 回首页开新一局    按 q 退出游戏", INNER)));
        put(out, &format!("╚{bar}╝"));
    }

    /// 测试辅助：在玩家脚边放一件道具，走过去就拾取，返回道具坐标。
    #[cfg(test)]
    pub(crate) fn grab(&mut self, kind: ItemKind) -> (usize, usize) {
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
pub(crate) fn roll(rng: &mut impl RngExt, atk: i32) -> i32 {
    (atk / 2 + rng.random_range(0..atk)).max(1)
}

/// 怪物一次攻击结算盾格挡的结果。
pub(crate) struct ShieldBlock {
    /// 该次攻击被盾挡住（骷髅/守护者暗箭穿盾时为 false，调用方照常掉血）。
    pub(crate) blocked: bool,
    /// 这一击把盾打碎（耐久归零）且自动从背包存量换上了一面（除新迎旧）。
    pub(crate) refilled: bool,
}

/// 怪物的一次命中打过来：手里有盾、且攻击不会穿盾（骷髅、6-7 层守护者的暗箭穿盾）就挡下（命中扣 1 点耐久），
/// 往 hits 里塞一条格挡消息；没盾或穿盾时 blocked = false，调用方照常掉血。
/// 如果这一击把盾打碎（耐久归零），自动从背包 stock 取耐久最高的一面换上（拿出来接着用）。
pub(crate) fn block_with_shield(
    shield: &mut i32,
    stock: &mut Vec<i32>,
    attacker: &str,
    weapon: &str,
    pierce: bool,
    hits: &mut Vec<String>,
) -> ShieldBlock {
    if *shield <= 0 || attacker == SKELETON || pierce {
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

/// 层数怪物血量缩放：1-7 层分别 ×1、×1.5、×2、×2.5、×3、×3.5、×4。
pub(crate) fn floor_hp(base: i32, f: i32) -> i32 {
    base * (2 + (f - 1)) / 2
}

/// 终端显示宽度：CJK、全角标点等占 2 列，其余（含框线 ═║）占 1 列。
pub(crate) fn display_width(s: &str) -> usize {
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
pub(crate) fn centered(s: &str, inner: usize) -> String {
    let pad = inner.saturating_sub(display_width(s));
    let left = pad / 2;
    format!("{}{s}{}", " ".repeat(left), " ".repeat(pad - left))
}

/// 结算面板里的账目行：左侧标签、右侧数值，中间点线补齐到 inner 宽。
pub(crate) fn ledger(label: &str, value: &str, inner: usize) -> String {
    let label = format!("  {label}");
    let vw = display_width(value);
    let dots = inner.saturating_sub(display_width(&label) + vw + 2).max(1);
    format!("{label} {} {value}", ".".repeat(dots))
}

/// 怪物尝试走到 (nx, ny)：可走、没被别的怪占着、不是玩家所在格。
pub(crate) fn try_move(
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

/// 按房间序号和层数生成怪物：前几间放近战小怪，后面放射手，最后一间是本层守护者。
/// 1-2 层 = 4/3 个小怪 + 剩余射手（3/4）；3-5 层近战怪全是骷髅（5/6/7 只递增），射手 3/4/5 只也递增；
/// 6-7 层骷髅绝迹，近战全是盾卫（6 层 6 只、7 层 8 只，越深越多），射手也明显增多（7/8 只），
/// 6-7 层守护者除了近战还能在 20 格外锁定玩家射穿盾暗箭。
/// 怪物血/攻逐层上涨（见 floor_hp 与攻击公式）。房间 0 是玩家出生点，不放怪。
pub(crate) fn populate(floor: u8, map: &Map) -> (Vec<Entity>, Vec<Item>) {
    use rand::seq::SliceRandom;

    let mut rng = rand::rng();
    let mut monsters = Vec::new();
    let n = map.rooms.len();
    let boss_room = n - 1;
    let f = floor as i32;
    // 1-2 层：小怪 4/3 只 + 剩余射手（3/4）；
    // 3-5 层：近战怪全是骷髅 5/6/7 只递增，射手 3/4/5 只递增，每间房可塞多只；
    // 6-7 层：近战全是盾卫 6/8 只递增，射手增至 7/8 只（骷髅不再出现）
    let (melee_count, ranged_count) = if floor <= 2 {
        let m = 4usize.saturating_sub(floor as usize - 1);
        (m, boss_room - 1 - m)
    } else if floor <= 5 {
        (2 + floor as usize, floor as usize) // 3层骷髅5射手3；4层6/4；5层7/5
    } else {
        (3 + floor as usize, floor as usize + 1) // 6层盾卫6射手7；7层盾卫8射手8
    };
    // 已占用格子（避免两只怪生在同一格）：从非出生房的所有可走格里随机挑
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for i in 1..boss_room {
        let (rx, ry, rw, rh) = map.rooms[i];
        for x in (rx + 1)..(rx + rw - 1) {
            for y in (ry + 1)..(ry + rh - 1) {
                if map.is_walkable(x, y) {
                    candidates.push((x, y));
                }
            }
        }
    }
    candidates.shuffle(&mut rng);
    let total = melee_count + ranged_count;
    for i in 0..total {
        let (mx, my) = candidates.pop().expect("可走格不够放怪，地图生成有问题");
        let (name, ch, hp, atk, sight, behavior, shield): (&str, char, i32, i32, usize, Behavior, i32) =
            if i < melee_count {
                // 3-5 层近战怪全是骷髅（除守护者外的近战皆骷髅）；6-7 层全是盾卫；1-2 层循环四种小怪
                let t = i % 4;
                let name = if floor >= 6 {
                    SHIELD_GUARD // 6-7 层骷髅绝迹，近战全是举破盾的盾卫
                } else if floor >= 3 {
                    SKELETON
                } else {
                    ["哥布林", "巨鼠", "蜘蛛", SKELETON][t]
                };
                let ch = match name {
                    SHIELD_GUARD => 't',
                    _ => {
                        if floor >= 3 {
                            'k'
                        } else {
                            ['g', 'r', 's', 'k'][t]
                        }
                    }
                };
                let skeleton_bonus = if name == SKELETON { SKELETON_ATK_BONUS } else { 0 };
                let base_atk = 3 + i as i32 + 2 * (f - 1) + skeleton_bonus;
                let atk = if name == SKELETON {
                    base_atk.min(MAX_SKELETON_ATK)
                } else if name == SHIELD_GUARD {
                    base_atk.min(MAX_GUARD_ATK) // 盾卫靠盾吃饭，攻击封顶低一点
                } else {
                    base_atk
                };
                // 盾卫自带一面残破的盾牌：耐久随机落在 GUARD_SHIELD_MIN..=GUARD_SHIELD_MAX
                let shield = if name == SHIELD_GUARD {
                    rng.random_range(GUARD_SHIELD_MIN..=GUARD_SHIELD_MAX)
                } else {
                    0
                };
                (
                    name,
                    ch,
                    floor_hp(10 + i as i32 * 4, f),
                    atk,
                    8,
                    Behavior::Melee,
                    shield,
                )
            } else {
                let j = i - melee_count;
                (
                    "暗影射手",
                    'o',
                    floor_hp(12 + j as i32 * 3, f),
                    2 + j as i32 / 2 + 2 * (f - 1),
                    12,
                    Behavior::Ranged,
                    0,
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
            shield,
        });
    }

    // 本层守护者：血量随层缩放，攻击每层 +2；6-7 层守护者还会在 20 格外锁定玩家射穿盾暗箭
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
            sight: if floor >= 6 { BOSS_LOCK_RANGE } else { 10 },
            behavior: if floor >= 6 { Behavior::Hybrid } else { Behavior::Melee },
            is_boss: true,
            shield: 0,
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

/// 按键 → (dx, dy)。h/j/k/l = 左/下/上/右，y/u/b/n = 四角。
pub(crate) fn key_delta(code: KeyCode) -> (i32, i32) {
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

#[cfg(test)]
mod tests;
