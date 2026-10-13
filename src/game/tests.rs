use super::*;
use std::collections::VecDeque;
use crate::ui::{render_homepage, Shop};

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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
    fn potion_backpack_is_capped() {        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        // 每次在玩家旁边放一只 1 血怪，按 a 一刀一个
        let mut drops = 0usize;
        for _ in 0..200 {
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
                sight: 0, // 看不见玩家，只会瞎逛，不会反击覆盖消息
                behavior: Behavior::Melee,
                is_boss: false,
                shield: 0,
            });
            g.try_attack();
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
        let g = Game::new(60, 25, &Ownership::default());
        let s = g.render();
        let bare = s.match_indices('\n').filter(|(i, _)| {
            i == &0 || s.as_bytes()[i - 1] != b'\r'
        });
        assert_eq!(bare.count(), 0, "render 存在裸 \\n");
    }

    /// 新一局（重开回商店后再进）玩家满血且回到房间0。
    #[test]
    fn new_game_resets() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.player.hp = 1;
        g.try_move_player(0, 1);
        let g2 = Game::new(60, 25, &Ownership::default());
        assert_eq!(g2.player.hp, g2.player.max_hp);
        let (rx, ry, _, _) = g2.map.rooms[0];
        assert!(g2.player.x.wrapping_sub(rx) < 4 && g2.player.y.wrapping_sub(ry) < 4);
    }

    /// 每层有 POTIONS_PER_FLOOR 个血瓶，外加武林秘籍与蕴气丹各一份；
    /// 1-7 层怪物数量与构成见 populate；必定有一只 boss（血量逐层上涨）。
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
                let expected_melee = if floor <= 2 {
                    4usize.saturating_sub(floor as usize - 1)
                } else if floor <= 5 {
                    2 + floor as usize
                } else {
                    3 + floor as usize // 6层盾卫6只；7层盾卫8只
                };
                let expected_ranged = if floor <= 2 {
                    3 + (floor as usize - 1)
                } else if floor <= 5 {
                    floor as usize
                } else {
                    floor as usize + 1 // 6-7 层射手明显增多：7/8 只
                };
                assert_eq!(melee, expected_melee, "第 {floor} 层小怪数量不对");
                assert_eq!(ranged, expected_ranged, "第 {floor} 层射手数量不对");
                // 3-5 层近战小怪（非 boss）必须全是骷髅
                if floor >= 3 && floor <= 5 {
                    assert!(monsters
                        .iter()
                        .filter(|m| !m.is_boss && matches!(m.behavior, Behavior::Melee))
                        .all(|m| m.name == SKELETON));
                }
                // 6-7 层骷髅绝迹，近战小怪全换成举破盾的盾卫，且 7 层比 6 层多
                if floor >= 6 {
                    let guards = monsters
                        .iter()
                        .filter(|m| !m.is_boss && matches!(m.behavior, Behavior::Melee))
                        .collect::<Vec<_>>();
                    assert!(guards.iter().all(|m| m.name == SHIELD_GUARD));
                    assert!(guards.iter().all(|m| m.name != SKELETON));
                    assert!(
                        guards.iter().all(|m| (GUARD_SHIELD_MIN..=GUARD_SHIELD_MAX).contains(&m.shield)),
                        "盾卫的破盾耐久该落在 {}..={} 之间",
                        GUARD_SHIELD_MIN,
                        GUARD_SHIELD_MAX
                    );
                    if floor == 7 {
                        assert!(guards.len() > 6, "7 层的盾卫该比 6 层多");
                    }
                }
                // boss 只能有一个，且是大血牛
                let bosses: Vec<_> = monsters.iter().filter(|m| m.is_boss).collect();
                assert_eq!(bosses.len(), 1, "第 {floor} 层 boss 数量不对");
                assert!(
                    matches!(bosses[0].behavior, Behavior::Melee | Behavior::Hybrid),
                    "第 {floor} 层 boss 该会近战"
                );
                // 6-7 层守护者远程锁定范围超 20 格，用来在 20 格外锁人射暗箭
                if floor >= 6 {
                    assert!(matches!(bosses[0].behavior, Behavior::Hybrid));
                    assert!(bosses[0].sight > 20, "6-7 层守护者该能锁定 20 格外的玩家");
                } else {
                    assert!(matches!(bosses[0].behavior, Behavior::Melee));
                }
                assert!(bosses[0].hp >= 50, "第 {floor} 层 boss 血量太低");
                // 每层都有远程射手，且视野比近战远，用来在房间另一头开火
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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

    /// 杀掉守护者：1-6 层只是解锁下楼/主动结算，第 7 层才是通关结算。
    #[test]
    fn boss_death_unlocks_descend_or_wins() {
        // 第 1 层：守护者倒下后可以继续走，也能主动结算
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
        });
        g.try_attack();
        assert!(g.boss_down, "杀掉守护者该解锁下楼");
        assert!(matches!(g.end, End::None), "第 1 层结束不了本局");
        assert!(g.can_restart(), "守护者倒下后 g 该能主动结束回商店");

        // 第 7 层：杀掉守护者直接进入通关结算，移动被封锁
        let mut g5 = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
        });
        g5.try_attack();
        assert!(g5.boss_down, "杀 boss 没解锁下楼");
        assert!(matches!(g5.end, End::Victory), "第 7 层杀 boss 该进入通关结算");
        assert!(g5.monsters.is_empty());
        // 通关后不能再动
        let after = (g5.player.x, g5.player.y);
        g5.try_move_player(1, 0);
        assert_eq!((g5.player.x, g5.player.y), after, "通关了还能乱动");
    }

    /// 怪物难度逐层上涨：boss 血量/攻击每层更强；1-7 层怪物数量与构成见 populate。
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
            let expected_melee = if f <= 2 {
                4usize.saturating_sub(f as usize - 1)
            } else if f <= 5 {
                2 + f as usize
            } else {
                3 + f as usize
            };
            assert_eq!(melee, expected_melee, "第 {f} 层小怪数量");
            assert_eq!(
                ranged,
                if f <= 2 {
                    3 + (f as usize - 1)
                } else if f <= 5 {
                    f as usize
                } else {
                    f as usize + 1
                },
                "第 {f} 层射手数量"
            );
            if f >= 3 && f <= 5 {
                assert!(ms
                    .iter()
                    .filter(|m| !m.is_boss && m.behavior == Behavior::Melee)
                    .all(|m| m.name == SKELETON));
            }
            // 6-7 层没有骷髅，近战全是盾卫，7 层比 6 层多
            if f >= 6 {
                let guards = ms
                    .iter()
                    .filter(|m| !m.is_boss && m.behavior == Behavior::Melee)
                    .count();
                assert!(ms.iter().all(|m| m.is_boss || m.name != SKELETON), "6-7 层不该有骷髅");
                assert!(
                    ms.iter()
                        .filter(|m| !m.is_boss && m.behavior == Behavior::Melee)
                        .all(|m| m.name == SHIELD_GUARD),
                    "6-7 层近战该全是盾卫"
                );
                assert_eq!(guards, 3 + f as usize);
            }
        }
    }

    /// 下楼：守护者倒下前按 d 无效，倒下后生成新一层地图并重置 boss_down。
    #[test]
    fn descend_only_after_boss_down() {
        let mut g = Game::new(60, 25, &Ownership::default());
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

    /// 第 7 层没有下一层，d 无效。
    #[test]
    fn descend_blocked_at_last_floor() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.floor = MAX_FLOOR;
        g.boss_down = true;
        g.descend();
        assert_eq!(g.floor, MAX_FLOOR, "第 7 层不能再下楼");
    }

    /// 结算分 = (击杀×100 + 血瓶×50) × 当前层倍率；死亡折半。
    #[test]
    fn settlement_multiplies_by_floor_and_halves_on_death() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        assert_eq!(g.settlement_score(), 525 + 3500, "通关时也应受幸运符惩罚，通关奖励 500×7 关");

        g.kills = 0;
        g.potions = 0;
        g.shield_stock = vec![45, 30];
        assert_eq!(g.settlement_score(), 525 + 3500, "盾牌所得积分也应减半");
    }

    /// 通关奖励：每通过一关 +500，不受倍率和折半影响；死在某层则不算该层。
    #[test]
    fn clear_bonus_is_500_per_floor_cleared() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
        assert_eq!(g.settlement_score(), 3500, "7 关全过 +3500");
    }

    /// 结算时成就积分：基础每个 100、高级每个 500、八格全满再 +1000；
    /// 进基础分，结算在惩罚之前（随层倍率、随死亡折半）。
    #[test]
    fn achievement_score_rewards_before_penalty() {
        let mut g = Game::new(60, 25, &Ownership::default());
        assert_eq!(g.achievement_score(), 0, "无成就不加分");

        // 基础成就每个 100
        g.achievements.insert(ACH_KILL_ONE);
        g.achievements.insert(ACH_GET_SHIELD);
        assert_eq!(g.achievement_score(), 2 * BASIC_ACH_SCORE, "两个基础 200");

        // 高级成就每个 500
        g.achievements.insert(ACH_FULLY_ARMED);
        assert_eq!(
            g.achievement_score(),
            2 * BASIC_ACH_SCORE + ADVANCED_ACH_SCORE,
            "两个基础 + 一个高级"
        );

        // 补齐八格全满：基础 4 + 高级 4，额外 +1000
        g.achievements.insert(ACH_NO_FEAR);
        g.achievements.insert(ACH_COMPENDIUM);
        g.achievements.insert(ACH_SHIELD_SWAP);
        g.achievements.insert(ACH_PINNED);
        g.achievements.insert(ACH_FULL_LOAD);
        assert!(g.all_achievements_unlocked());
        assert_eq!(
            g.achievement_score(),
            4 * BASIC_ACH_SCORE + 4 * ADVANCED_ACH_SCORE + FULL_ACH_BONUS,
            "八格全满再 +1000"
        );

        // 成就积分进基础分：随层倍率、结算在惩罚之前（随死亡折半）
        let ach = g.achievement_score();
        g.kills = 0;
        g.potions = 0;
        g.blade_count = 0;
        g.bow_count = 0;
        g.shield_stock.clear();
        g.boss_down = false; // 隔离通关奖励
        g.floor = 1;
        g.end = End::Voluntary;
        assert_eq!(g.settlement_score(), ach, "1 层 ×1 无惩罚：结算 = 成就积分");
        g.floor = 3;
        assert_eq!(g.settlement_score(), ach * 3 + g.clear_bonus(), "成就积分随层倍率");
        g.end = End::Defeat;
        assert_eq!(g.settlement_score(), ach * 3 / 2 + g.clear_bonus(), "结算在惩罚之前，随死亡折半");

        // 面板对齐：八格全满时成就行不应撑破结算面板
        let panel = g.render();
        assert!(panel.contains("基础成就 4 个") && panel.contains("高级成就 4 个") && panel.contains("八格成就全满"));
        let rows: Vec<String> = panel
            .lines()
            .filter(|line| line.contains('║'))
            .map(|line| {
                let mut out = String::new();
                let mut chars = line.chars();
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
            })
            .collect();
        let width = display_width(&rows[0]);
        assert!(rows.iter().all(|line| display_width(line) == width), "成就行不应撑破面板");
    }

    /// 骷髅的攻击比同位置普通小怪更高。
    #[test]
    fn skeleton_has_attack_bonus() {
        let mut map = Map::new();
        map.generate(None);
        let (monsters, _) = populate(1, &map);
        let sk = monsters.iter().find(|m| m.name == SKELETON).expect("第 1 层该有骷髅");
        // 1 层骷髅是第 4 只近战（生成下标 3），atk = 3 + 3 + 加成
        assert_eq!(sk.atk, 3 + 3 + SKELETON_ATK_BONUS);
    }

    /// 活着的时候按 g 不能重开；死了或通关了才可以（回商店开新局）。
    #[test]
    fn restart_is_blocked_while_alive() {
        let mut g = Game::new(60, 25, &Ownership::default());
        assert!(!g.can_restart(), "活着就能重开");

        g.player.hp = 0;
        assert!(g.can_restart(), "死了反而重开不了");

        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.boss_down = true;
        assert!(g2.can_restart(), "守护者倒下后反而结束不了");
    }

    /// 秘籍与蕴气丹：捡了进背包，按对应键才生效；效果分别落在攻击力和血量上限上。
    #[test]
    fn manual_and_qi_pill_work() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
        g.player.hp = 5;
        g.qi_pills = 1;
        g.use_qi_pill();
        assert_eq!(g.player.hp, 0, "5 血服丹该当场去世");
        assert_eq!(g.player.max_hp, 50, "但血量上限该照常涨到 50");
    }

    /// 秘籍和蕴气丹全图唯一：已经用过就不再拾取地上重复的份。
    #[test]
    fn uniques_are_one_per_floor() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
        g.manuals = 1;
        g.qi_pills = 1;
        g.player.atk = 99;
        g.player.max_hp = 99;
        g.player.hp = 99;

        let fresh = Game::new(60, 25, &Ownership::default());
        assert_eq!(fresh.manuals, 0, "新局没清空秘籍");
        assert_eq!(fresh.qi_pills, 0, "新局没清空蕴气丹");
        assert_eq!(fresh.player.atk, BASE_ATK, "新局没重置攻击力");
        assert_eq!(fresh.player.max_hp, BASE_MAX_HP, "新局没重置血量上限");
        assert_eq!(fresh.player.hp, BASE_MAX_HP);
    }

    /// 物品行得被渲染出来，操作提示也得带上 2/3 两个新键。
    #[test]
    fn render_shows_new_backpack_and_keys() {
        let g = Game::new(60, 25, &Ownership::default());
        let s = g.render();
        assert!(s.contains("物品"), "没有渲染物品行");
        assert!(s.contains("秘籍"), "物品行没有秘籍");
        assert!(s.contains("蕴气丹"), "物品行没有蕴气丹");
        assert!(s.contains("2 秘籍"), "操作提示没有 2 键");
        assert!(s.contains("3 蕴气丹"), "操作提示没有 3 键");
    }

    /// 地图上四类道具各有字形：+ / M / Q / S。
    #[test]
    fn items_render_their_own_glyphs() {
        let mut g = Game::new(60, 25, &Ownership::default());
        let (px, py) = (g.player.x, g.player.y);
        let spots = [(px + 1, py), (px, py + 1)];
        let spot = spots
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
        assert_eq!(glyph_at(ItemKind::Shield), 'S', "盾牌该画成 S");
    }

    /// 结算面板：每行都以 ║ 开头并以 ║ 结尾且显示宽度一致（CJK 按 2 列计），标题不会被顶出框。
    #[test]
    fn victory_panel_is_aligned() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
        s.add_run(8, 3, 0, 0, 950); // 1 层 ×1：8*100 + 3*50 = 950
        assert_eq!(s.last_score, 950);
        assert_eq!(s.credits, 950, "通关赚的积分该进余额");
        assert_eq!(s.total_score, 950);

        s.add_run(2, 0, 4, 0, 1000); // 5 层 ×5：200*5 = 1000
        assert_eq!(s.total_score, 1950, "累计分该跨局累加");
        assert_eq!(s.credits, 1950, "积分余额该跨局累加");

        s.add_run(4, 2, 1, 0, 750); // 死亡折半：(400+100)×3/2 = 750
        assert_eq!(s.total_score, 2700);
        assert_eq!(s.credits, 2700);

        s.add_run(0, 0, 0, 1, 500); // 骷髅刀 500 积分
        assert_eq!(s.total_score, 3200, "骷髅刀积分没累加");
        assert_eq!(s.last_blades, 1, "骷髅刀数没记下来");

        let reread = Score::load();
        assert_eq!(reread.total_score, 3200, "落盘后读不回累计分");
        assert_eq!(reread.credits, 3200, "落盘后读不回积分余额");
        assert_eq!(reread.last_kills, 0);
        assert_eq!(reread.last_potions, 0);
        assert_eq!(reread.last_shields, 0);
        assert_eq!(reread.last_blades, 1, "上局骷髅刀数没记下来");

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
        let mut g = Game::new(60, 25, &Ownership::default());
        g.apply_purchase(&p);
        assert_eq!(g.manuals, 1, "买的秘籍没进背包");
        assert!(g.lucky, "幸运符没生效");
        assert_eq!(g.potions, 3, "买的血瓶没进背包");
    }

    /// 幸运符下血瓶掉率 90%：独立杀 400 只怪，掉落数应约 360。
    #[test]
    fn lucky_charm_drop_rate_is_90() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
                shield: 0,
            });
            g.try_attack();
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g2 = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
                shield: 0,
            });
            g.try_attack();
            if g.msg.contains("掉落了一面盾牌") {
                drops += 1;
            }
        }
        // 200 次抽样下，50% 掉率落在 70~130 之外的概率极小
        assert!(drops > 70 && drops < 130, "盾牌掉率异常：杀 200 只暗影射手只掉了 {drops} 面");

        // 幸运符：每一面都该掉
        let mut g2 = Game::new(60, 25, &Ownership::default());
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
                shield: 0,
            });
            g2.try_attack();
            assert!(g2.msg.contains("掉落了一面盾牌"), "幸运符下盾牌该必掉：{}", g2.msg);
        }
    }

    // TEMP STAT TEST (will remove)
    #[test]
    fn temp_stat_shield_drop_large_sample() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let mut drops = 0usize;
        let n = 2000usize;
        for _ in 0..n {
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
                sight: 0,
                behavior: Behavior::Ranged,
                is_boss: false,
                shield: 0,
            });
            g.try_attack();
            if g.msg.contains("掉落了一面盾牌") {
                drops += 1;
            }
        }
        println!("TEMP_STAT default shield drops: {drops}/{n} = {:.1}%", drops as f64 / n as f64 * 100.0);
        assert!(drops > 900 && drops < 1100, "TEMP default 50% 掉率不在 45~55% 区间：{drops}/{n}");
    }

    // TEMP STAT TEST (will remove)
    #[test]
    fn temp_stat_shield_drop_lucky_large_sample() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.lucky = true;
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let mut drops = 0usize;
        let n = 300usize;
        for _ in 0..n {
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
                sight: 0,
                behavior: Behavior::Ranged,
                is_boss: false,
                shield: 0,
            });
            g.try_attack();
            if g.msg.contains("掉落了一面盾牌") {
                drops += 1;
            }
        }
        println!("TEMP_STAT lucky shield drops: {drops}/{n} = {:.1}%", drops as f64 / n as f64 * 100.0);
        assert_eq!(drops, n, "TEMP 幸运符下盾牌该 100% 掉落");
    }

    // TEMP STAT TEST (will remove)
    #[test]
    fn temp_stat_multi_kill_shield_items() {
        // 模拟真实战斗：一次 a 攻击同时打死 2 只暗影射手，按地图上的 S 计数（消息可能被 hits>1 覆盖）
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for d in [(-1, 0), (1, 0)] {
            let x = px as i32 + d.0;
            let y = py as i32 + d.1;
            if x >= 0 && y >= 0 {
                let i = g.map.idx(x as usize, y as usize);
                g.map.tiles[i] = Tile::Floor;
            }
        }
        let mut drops = 0usize;
        let n = 1000usize;
        for _ in 0..n {
            g.monsters.push(Entity {
                x: px.wrapping_sub(1),
                y: py,
                ch: 'o',
                name: "暗影射手",
                hp: 1,
                max_hp: 1,
                atk: 0,
                sight: 0,
                behavior: Behavior::Ranged,
                is_boss: false,
                shield: 0,
            });
            g.monsters.push(Entity {
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
                shield: 0,
            });
            g.try_attack();
            drops += g.items.iter().filter(|it| it.kind == ItemKind::Shield).count();
            g.items.clear();
        }
        // 每回合杀 2 只 × 1000 回合 = 2000 次击杀，50% 掉率 → 期望 ~1000
        println!("TEMP_STAT multi-kill shield items: {drops}/2000 kills = {:.1}%", drops as f64 / 2000.0 * 100.0);
        assert!(drops > 850 && drops < 1150, "TEMP 多次击杀下盾牌掉率异常：{drops}/2000");
    }

    /// 捡盾牌满耐久 45；手里已有盾时换上新盾，耐久回满，旧盾（保留当前耐久）收进背包。
    #[test]
    fn pickup_shield_replaces_and_refills() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let r = block_with_shield(&mut shield, &mut stock, "暗影射手", "暗影箭", false, &mut hits);
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
        let mut g = Game::new(60, 25, &Ownership::default());
        g.shield = 1;
        g.shield_stock.clear();
        let mut hits: Vec<String> = Vec::new();
        let r = block_with_shield(&mut g.shield, &mut g.shield_stock, "暗影射手", "暗影箭", false, &mut hits);
        assert!(r.blocked, "没存量也该挡住最后一下");
        assert!(!r.refilled, "背包没存量，refilled 该是 false");
        assert_eq!(g.shield, 0, "背包没存量时碎了就是碎了");
        assert!(g.shield_stock.is_empty());
    }

    /// 背包存量上限：收不下旧盾时新盾留在原地。
    #[test]
    fn shield_stock_capped_at_max() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut gmax = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
            let mut g = Game::new(60, 25, &Ownership::default());
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
                shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        assert!(s.contains("增加你的幸运值"), "商店没有幸运符说明");
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
            shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.monsters.clear();
        g2.grab(ItemKind::Shield);
        assert!(g2.shield_stock.is_empty(), "手里没盾时拾取不进存量");
        assert!(!g2.achievements.contains(ACH_FULL_LOAD), "存量没满不该解锁");
    }

    /// 杀死怪物：击杀一只怪就解锁；没击杀不解锁。
    #[test]
    fn achievement_kill_one_on_first_kill() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
        });
        g.try_attack();
        assert_eq!(g.kills, 1, "测试前提：该一刀杀掉");
        assert!(g.achievements.contains(ACH_KILL_ONE), "击杀一只就该解锁杀死怪物");
        assert!(g.species_killed.contains(SKELETON), "图鉴该记上骷髅这一种");

        // 不击杀（纯移动）不解锁
        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.try_move_player(0, 1);
        assert!(!g2.achievements.contains(ACH_KILL_ONE), "没杀怪不该解锁");
    }

    /// 拾取盾牌：拿到手就解锁；换新盾也算；满包放不进不算。
    #[test]
    fn achievement_get_shield_on_pickup() {
        // 手里没盾捡到：解锁
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        g.grab(ItemKind::Shield);
        assert!(g.achievements.contains(ACH_GET_SHIELD), "捡到盾牌该解锁拾取盾牌");

        // 手里有旧盾换上新盾：也解锁
        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.monsters.clear();
        g2.shield = 20;
        g2.grab(ItemKind::Shield);
        assert!(g2.achievements.contains(ACH_GET_SHIELD), "换盾也算获取盾牌");

        // 存量满、旧盾放不进：新盾留在原地，不解锁
        let mut g3 = Game::new(60, 25, &Ownership::default());
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
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
        let mut g2 = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
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
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        // 六种小怪/射手全杀过，还差 BOSS：不该解锁
        for name in ["哥布林", "巨鼠", "蜘蛛", SKELETON, SHIELD_GUARD, "暗影射手"] {
            g.register_kill(name);
        }
        assert!(!g.achievements.contains(ACH_COMPENDIUM), "少杀一种不该解锁图鉴");
        assert_eq!(g.species_killed.len(), 6, "该记上 6 种");

        // BOSS 补上：解锁
        g.register_kill(BOSS);
        assert!(g.achievements.contains(ACH_COMPENDIUM), "七种全杀该解锁怪物图鉴");
        assert_eq!(g.species_killed.len(), 7, "图鉴该记满 7 种");

        // 重复击杀不多加
        g.register_kill(BOSS);
        g.register_kill("哥布林");
        assert_eq!(g.species_killed.len(), 7, "重复击杀不该重复记账");
    }

    /// 怪物图鉴经真实击杀记账：一刀杀掉骷髅后，图鉴集合里该有骷髅。
    #[test]
    fn compendium_records_real_kills() {
        let mut g = Game::new(60, 25, &Ownership::default());
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
            shield: 0,
        });
        g.try_attack();
        assert!(g.species_killed.contains(SKELETON), "真实击杀该记账进图鉴");
        assert!(!g.achievements.contains(ACH_COMPENDIUM), "才杀一种离图鉴还远");
    }

    /// 成就行是 HUD 专门两行（基础/高级区分）：没达成过显示"无"，达成过按固定顺序列出；
    /// 不进 msg 普通消息栏；新局重置。
    #[test]
    fn render_shows_achievements_line() {
        let g = Game::new(60, 25, &Ownership::default());
        let s = g.render();
        assert!(s.contains("基础成就：无"), "没有基础成就时该行该显示无");
        assert!(s.contains("高级成就：无"), "没有高级成就时该行该显示无");

        let mut g = Game::new(60, 25, &Ownership::default());
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
        let fresh = Game::new(60, 25, &Ownership::default());
        assert!(fresh.achievements.is_empty(), "新局不该带着上局成就");
    }

    /// 骷髅刀：骷髅死后 20% 掉落，带幸运符 80% 掉落；背包里有时无法拾取新的。
    #[test]
    fn skeleton_drops_blade_20_percent() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let mut drops = 0usize;
        for _ in 0..200 {
            // 重置 blade 背包状态，每轮都测试掉率
            g.blade_count = 0;
            g.blade_equipped = false;
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
                shield: 0,
            });
            g.try_attack();
            if g.msg.contains("骷髅掉落了骷髅刀") {
                drops += 1;
            }
        }
        // 200 次抽样下，20% 掉率落在 25~55 之外的概率极小
        assert!(drops > 25 && drops < 55, "骷髅刀掉率异常：杀 200 只骷髅只掉了 {drops} 把");

        // 幸运符：80% 掉率
        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.lucky = true;
        g2.monsters.clear();
        let (px, py) = (g2.player.x, g2.player.y);
        let mut drops2 = 0usize;
        for _ in 0..100 {
            g2.blade_count = 0;
            g2.blade_equipped = false;
            let ti = g2.map.idx(px + 1, py);
            g2.map.tiles[ti] = Tile::Floor;
            g2.monsters.push(Entity {
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
                shield: 0,
            });
            g2.try_attack();
            if g2.msg.contains("骷髅掉落了骷髅刀") {
                drops2 += 1;
            }
        }
        // 100 次抽样下，80% 掉率落在 65~95 之外的概率极小
        assert!(drops2 > 65 && drops2 < 95, "幸运符下骷髅刀掉率异常：100 只骷髅只掉了 {drops2} 把");
    }

    /// 骷髅刀拾取：骷髅死后掉落的骷髅刀只进背包，不自动装备；背包堆满后拒绝。
    #[test]
    fn blade_auto_collects_on_skeleton_death() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        // 幸运符下骷髅刀 80% 掉率，多杀几只必掉（每杀压一只 1 血骷髅）
        g.lucky = true;
        let mut got = false;
        for _ in 0..100 {
            g.monsters.clear();
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
                shield: 0,
            });
            g.try_attack();
            if g.blade_count > 0 {
                got = true;
                break;
            }
        }
        assert!(got, "幸运符下杀骷髅必该掉骷髅刀");
        // 骷髅死后掉落的骷髅刀只进背包，不自动装备
        assert!(!g.blade_equipped, "捡到骷髅刀不该自动装备");
        g.blade_count = 0;
        g.blade_equipped = false;

        // 手动装备骷髅刀：攻击 +6
        g.blade_count = 1;
        g.equip_blade();
        assert!(g.blade_equipped, "手动装备骷髅刀失败");
        assert_eq!(g.attack_power(), BASE_ATK + BLADE_ATK_BONUS, "装备骷髅刀后攻击没加成");

        // 背包堆满 BLADE_STACK_MAX 把后，新掉落无法拾取
        g.blade_count = BLADE_STACK_MAX;
        g.blade_equipped = false;
        g.lucky = true;
        let mut dropped_but_rejected = false;
        for _ in 0..50 {
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
                shield: 0,
            });
            g.try_attack();
            if g.msg.contains("无法拾取") {
                dropped_but_rejected = true;
                break;
            }
        }
        assert!(dropped_but_rejected, "背包堆满骷髅刀时，新掉落该被拒绝");
        assert_eq!(g.blade_count, BLADE_STACK_MAX, "拒绝后背包数量不该增长");
    }

    /// 骷髅刀效果：装备时攻击固定 +6，卸下后恢复基础攻击。
    #[test]
    fn blade_bonus_is_fixed() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.blade_count = 1;
        g.blade_equipped = true;
        assert_eq!(g.attack_power(), BASE_ATK + BLADE_ATK_BONUS, "装备骷髅刀攻击没加成");

        g.blade_equipped = false;
        assert_eq!(g.attack_power(), BASE_ATK, "卸下骷髅刀攻击没恢复");

        // 与秘籍叠加：8 → 16（秘籍）→ 22（骷髅刀）
        g.player.atk = 2 * BASE_ATK;
        g.blade_equipped = true;
        assert_eq!(g.attack_power(), 2 * BASE_ATK + BLADE_ATK_BONUS, "与秘籍叠加不对");
    }

    /// 武器切换：按 w 打开菜单，1/2 选择武器，关闭菜单不消耗回合。
    #[test]
    fn weapon_menu_does_not_cost_turn() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
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
            shield: 0,
        });
        g.blade_count = 1;
        let before = g.monsters[0].x;

        // 打开菜单、装备骷髅刀、关闭菜单：怪物不该动
        g.toggle_weapon_menu();
        g.equip_blade();
        assert!(g.monsters[0].x == before, "切换武器让怪物动了");

        // 对照：正常移动一步，怪物就该动了
        g.try_move_player(1, 0);
        assert_ne!(g.monsters[0].x, before, "移动后怪物没动，测试前提不成立");
    }

    /// 骷髅刀结算：一把 500 积分，随层数倍率、死亡折半。
    #[test]
    fn blade_scores_500_in_settlement() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.kills = 0;
        g.potions = 0;
        g.blade_count = 1;
        assert_eq!(g.blade_score(), BLADE_SCORE, "骷髅刀积分不对");
        g.floor = 1;
        g.boss_down = true;
        assert_eq!(g.settlement_score(), BLADE_SCORE + 500, "1 层 ×1 + 通关 1 关");
        g.floor = 3;
        g.boss_down = true;
        assert_eq!(g.settlement_score(), BLADE_SCORE * 3 + 1500, "3 层 ×3，已通过 3 关 +1500");
        g.end = End::Defeat;
        g.boss_down = false;
        assert_eq!(
            g.settlement_score(),
            BLADE_SCORE * 3 / 2 + 1000,
            "死亡该折半，通关奖励不折半"
        );
        // 结算面板里出现骷髅刀那行
        g.end = End::Voluntary;
        g.floor = 3;
        g.boss_down = true;
        let s = g.render();
        assert!(s.contains("骷髅刀"), "结算面板没显示骷髅刀积分");
    }

    /// 攻击力计算：基础攻击 + 骷髅刀加成（如果装备）。
    #[test]
    fn attack_power_includes_blade_bonus() {
        let mut g = Game::new(60, 25, &Ownership::default());
        assert_eq!(g.attack_power(), BASE_ATK, "新局攻击力不对");

        g.blade_count = 1;
        g.blade_equipped = true;
        assert_eq!(g.attack_power(), BASE_ATK + BLADE_ATK_BONUS, "装备骷髅刀后攻击力不对");

        g.player.atk = 2 * BASE_ATK;
        assert_eq!(g.attack_power(), 2 * BASE_ATK + BLADE_ATK_BONUS, "与秘籍叠加后攻击力不对");
    }

    /// 按 a 攻击：范围内 1 格（前后左右+斜方向共 8 格）的怪物受到伤害，不位移。
    #[test]
    fn attack_key_a_deals_damage_in_range() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        // 凿通 3x3 的格子
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 { continue; }
                let x = px as i32 + dx;
                let y = py as i32 + dy;
                if x >= 0 && y >= 0 && x < W as i32 && y < H as i32 {
                    let i = g.map.idx(x as usize, y as usize);
                    g.map.tiles[i] = Tile::Floor;
                }
            }
        }
        // 8 个方向各放一只 1 血怪
        let dirs = [(-1,-1),(-1,0),(-1,1),(0,-1),(0,1),(1,-1),(1,0),(1,1)];
        for (dx, dy) in dirs {
            g.monsters.push(Entity {
                x: (px as i32 + dx) as usize,
                y: (py as i32 + dy) as usize,
                ch: 'g',
                name: "哥布林",
                hp: 1,
                max_hp: 1,
                atk: 0,
                sight: 0,
                behavior: Behavior::Melee,
                is_boss: false,
                shield: 0,
            });
        }
        let pos_before = (g.player.x, g.player.y);
        g.try_attack();
        // 攻击后位置不变
        assert_eq!((g.player.x, g.player.y), pos_before, "攻击不该位移");
        // 8 只 1 血怪全被杀
        assert_eq!(g.monsters.len(), 0, "8 只怪该全被杀掉");
        assert_eq!(g.kills, 8, "该记 8 杀");
        assert!(g.achievements.contains(ACH_KILL_ONE), "该解锁杀死怪物成就");
    }

    /// 按 a 攻击范围外无怪：提示没有目标，消耗回合。
    #[test]
    fn attack_key_a_empty_when_no_target() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        g.try_attack();
        assert!(g.msg.contains("没有可以攻击"), "无目标该提示没有怪物：{}", g.msg);
    }

    /// 暗影弓掉落：暗影射手死后 20% 掉暗影弓（幸运符 80%）。
    #[test]
    fn shadow_archer_drops_bow_20_percent() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let ti = g.map.idx(px + 1, py);
        g.map.tiles[ti] = Tile::Floor;
        let mut drops = 0usize;
        for _ in 0..200 {
            g.bow_count = 0;
            g.bow_equipped = false;
            g.monsters.push(Entity {
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
                shield: 0,
            });
            g.try_attack();
            if g.msg.contains("暗影射手掉落了一把暗影弓") {
                drops += 1;
            }
        }
        // 200 次抽样下，20% 掉率落在 25~55 之外的概率极小
        assert!(drops > 25 && drops < 55, "暗影弓掉率异常：杀 200 只暗影射手只掉了 {drops} 把");

        // 幸运符：80% 掉率
        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.lucky = true;
        g2.monsters.clear();
        let (px, py) = (g2.player.x, g2.player.y);
        let ti = g2.map.idx(px + 1, py);
        g2.map.tiles[ti] = Tile::Floor;
        let mut drops2 = 0usize;
        for _ in 0..100 {
            g2.bow_count = 0;
            g2.bow_equipped = false;
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
                shield: 0,
            });
            g2.try_attack();
            if g2.msg.contains("暗影射手掉落了一把暗影弓") {
                drops2 += 1;
            }
        }
        assert!(drops2 > 65 && drops2 < 95, "幸运符下暗影弓掉率异常：100 只暗影射手只掉了 {drops2} 把");
    }

    /// 拾取暗影弓：走路踩到 B 只进背包，不自动装备；背包堆满后留在原地。
    #[test]
    fn pickup_bow_equips_and_rejects_duplicate() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        let spot = [(px + 1, py), (px, py + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("旁边该有块空地");
        g.items.push(Item { x: spot.0, y: spot.1, kind: ItemKind::Bow });
        let dx = spot.0 as i32 - px as i32;
        let dy = spot.1 as i32 - py as i32;
        g.try_move_player(dx, dy);
        assert_eq!(g.bow_count, 1, "暗影弓没进背包");
        assert!(!g.bow_equipped, "捡起来不该自动装备，要按 w");
        assert!(
            !g.items.iter().any(|it| it.x == spot.0 && it.y == spot.1),
            "地上的弓该被拾取"
        );

        // 手动装备后：攻击范围该是 4
        g.equip_bow();
        assert!(g.bow_equipped, "手动装备弓失败");
        assert_eq!(g.attack_range(), BOW_RANGE, "装备弓后攻击范围该是 4");

        // 背包越满越能继续拾取，直到叠到 BOW_STACK_MAX
        for _ in 2..=BOW_STACK_MAX {
            let next = [(g.player.x + 1, g.player.y), (g.player.x, g.player.y + 1)]
                .into_iter()
                .find(|&(x, y)| g.map.is_walkable(x, y))
                .expect("旁边该有块空地");
            g.items.push(Item { x: next.0, y: next.1, kind: ItemKind::Bow });
            let d2 = next.0 as i32 - g.player.x as i32;
            let d3 = next.1 as i32 - g.player.y as i32;
            g.try_move_player(d2, d3);
        }
        assert_eq!(g.bow_count, BOW_STACK_MAX, "背包容纳数不对");

        // 背包堆满，再踩到第二把弓：留在原地
        let second = [(g.player.x + 1, g.player.y), (g.player.x, g.player.y + 1)]
            .into_iter()
            .find(|&(x, y)| g.map.is_walkable(x, y))
            .expect("旁边该有块空地");
        g.items.push(Item { x: second.0, y: second.1, kind: ItemKind::Bow });
        let dx2 = second.0 as i32 - g.player.x as i32;
        let dy2 = second.1 as i32 - g.player.y as i32;
        g.try_move_player(dx2, dy2);
        assert_eq!(g.bow_count, BOW_STACK_MAX, "背包容纳数不该超过上限");
        assert!(
            g.items.iter().any(|it| it.x == second.0 && it.y == second.1),
            "背包满时第二把弓该留在原地"
        );
    }

    /// 装备暗影弓：攻击范围 1 → 4，伤害 ÷2；卸下后恢复。
    #[test]
    fn bow_equipped_changes_range_and_halves_damage() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        assert_eq!(g.attack_range(), 1, "空手时攻击范围该是 1");

        g.bow_count = 1;
        g.equip_bow();
        assert!(g.bow_equipped, "该装备成功");
        assert_eq!(g.attack_range(), BOW_RANGE, "装备弓后攻击范围该是 4");
        // 伤害 ÷2：基础攻击 8，装备弓后至少应不超过 4
        let atk_power = g.attack_power();
        assert!(atk_power <= BASE_ATK / BOW_DAMAGE_DIVISOR, "装备弓后攻击力该被减半：{atk_power}");

        // 卸下弓：范围恢复 1
        g.unequip_bow();
        assert!(!g.bow_equipped, "该卸下");
        assert_eq!(g.attack_range(), 1, "卸下弓后攻击范围该恢复 1");
        assert_eq!(g.attack_power(), BASE_ATK, "卸下弓后攻击力该恢复基础值");
    }

    /// 暗影弓：4 格外怪物也可攻击到（范围扩大生效）。
    #[test]
    fn bow_reaches_out_to_range_4() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        // 凿通 5x5 的格子
        for dy in -4..=4 {
            for dx in -4..=4 {
                if dx == 0 && dy == 0 { continue; }
                let x = px as i32 + dx;
                let y = py as i32 + dy;
                if x >= 0 && y >= 0 && x < W as i32 && y < H as i32 {
                    let i = g.map.idx(x as usize, y as usize);
                    g.map.tiles[i] = Tile::Floor;
                }
            }
        }
        // 放一只怪在 4 格外
        g.monsters.push(Entity {
            x: px + 4,
            y: py,
            ch: 'g',
            name: "哥布林",
            hp: 1,
            max_hp: 1,
            atk: 0,
            sight: 0,
            behavior: Behavior::Melee,
            is_boss: false,
            shield: 0,
        });
        // 空手时打不到 4 格外
        assert_eq!(g.attack_range(), 1);
        g.try_attack();
        assert_eq!(g.monsters.len(), 1, "空手时 4 格外的怪不该被打到");

        // 刚才那一回合怪物可能随机游荡，搬回 4 格外再验弓的射程
        g.monsters[0].x = px + 4;
        g.monsters[0].y = py;

        // 装备弓后可打到
        g.bow_count = 1;
        g.equip_bow();
        g.try_attack();
        assert_eq!(g.monsters.len(), 0, "装备弓后 4 格外的怪该被打到");
        assert_eq!(g.kills, 1, "该记 1 杀");
    }

    /// 暗影弓结算：一把 1000 积分，随层数倍率、死亡折半。
    #[test]
    fn bow_scores_1000_in_settlement() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.kills = 0;
        g.potions = 0;
        g.blade_count = 0;
        g.bow_count = 1;
        assert_eq!(g.bow_score(), BOW_SCORE, "暗影弓积分不对");
        g.floor = 1;
        g.boss_down = true;
        assert_eq!(g.settlement_score(), BOW_SCORE + 500, "1 层 ×1 + 通关 1 关");
        g.floor = 3;
        g.boss_down = true;
        assert_eq!(g.settlement_score(), BOW_SCORE * 3 + 1500, "3 层 ×3，已通过 3 关 +1500");
        g.end = End::Defeat;
        g.boss_down = false;
        assert_eq!(
            g.settlement_score(),
            BOW_SCORE * 3 / 2 + 1000,
            "死亡该折半，通关奖励不折半"
        );
        // 结算面板里出现暗影弓那行
        g.end = End::Voluntary;
        g.floor = 3;
        g.boss_down = true;
        let s = g.render();
        assert!(s.contains("暗影弓"), "结算面板没显示暗影弓积分");
    }

    /// 武器菜单：3 键切换暗影弓，不消耗回合。
    #[test]
    fn weapon_menu_bow_does_not_cost_turn() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
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
            shield: 0,
        });
        g.bow_count = 1;
        let before = g.monsters[0].x;

        // 打开菜单、装备弓、关闭菜单：怪物不该动
        g.toggle_weapon_menu();
        g.equip_bow();
        assert!(g.monsters[0].x == before, "切换武器让怪物动了");
        assert_eq!(g.attack_range(), BOW_RANGE, "装备弓后范围该是 4");

        // 对照：正常攻击一步，怪物就该动了
        g.try_attack();
        assert_ne!(g.monsters[0].x, before, "攻击后怪物没动，测试前提不成立");
    }

    /// 新游戏重置：弓状态应被清空。
    #[test]
    fn new_game_resets_bow() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.bow_count = 1;
        g.bow_equipped = true;
        let fresh = Game::new(60, 25, &Ownership::default());
        assert_eq!(fresh.bow_count, 0, "新局不该带上局弓");
        assert!(!fresh.bow_equipped, "新局不该装备弓");
    }

    /// 库存武器开局带上一件可用的：自动装备，卸下后仍能装回；库存其余数量只作展示。
    #[test]
    fn inventory_weapons_auto_equip_at_new_game() {
        // 只有骷髅刀库存：开局装备骷髅刀，背包有 1 把可卸可装
        let own = Ownership {
            skeleton_blade: 7,
            shadow_bow: 0,
            ..Default::default()
        };
        let mut g = Game::new(60, 25, &own);
        assert_eq!(g.blade_count, 1, "有库存该带上一件可用的（其余只作展示）");
        assert!(g.blade_equipped, "有骷髅刀库存该开局自动装备");
        assert!(!g.bow_equipped, "没弓库存不该装备弓");
        assert_eq!(g.attack_power(), BASE_ATK + BLADE_ATK_BONUS, "自动装备的骷髅刀该加成攻击");

        // 卸下再装回：背包里始终有那 1 把，不会提示没有骷髅刀
        g.switch_to_fists();
        assert!(!g.blade_equipped, "应已切到空手");
        g.blade_equipped = false;
        g.equip_blade();
        assert!(g.blade_equipped, "从库存带上来的骷髅刀卸下后该能重新装备");

        // 只有暗影弓库存：开局装备暗影弓
        let own2 = Ownership {
            skeleton_blade: 0,
            shadow_bow: 4,
            ..Default::default()
        };
        let mut g2 = Game::new(60, 25, &own2);
        assert_eq!(g2.bow_count, 1, "有弓库存该带上一件可用的");
        assert!(g2.bow_equipped, "有弓库存该开局自动装备");
        assert_eq!(g2.attack_range(), BOW_RANGE, "自动装备的弓该有 4 格范围");
        // 卸下再装回
        g2.switch_to_fists();
        g2.equip_bow();
        assert!(g2.bow_equipped, "从库存带上来的暗影弓卸下后该能重新装备");

        // 弓与刀库存都有：优先装备弓（互斥），刀那件仍在背包可切换
        let own3 = Ownership {
            skeleton_blade: 7,
            shadow_bow: 4,
            ..Default::default()
        };
        let mut g3 = Game::new(60, 25, &own3);
        assert!(g3.bow_equipped, "弓箭库存都有时该装备弓");
        assert!(!g3.blade_equipped, "装备弓时不该同时装备骷髅刀");
        g3.switch_to_fists();
        g3.equip_blade();
        assert!(g3.blade_equipped, "库存带入的骷髅刀也该能切换装备");

        // 库存全空：空手，背包也没有
        let g4 = Game::new(60, 25, &Ownership::default());
        assert_eq!(g4.blade_count, 0);
        assert_eq!(g4.bow_count, 0);
        assert!(!g4.blade_equipped && !g4.bow_equipped, "无库存该空手开局");
    }

    /// 结算后保存库存：本局背包里的骷髅刀/暗影弓数量累计进库存，跨局累加。
    #[test]
    fn save_ownership_persists_weapons() {
        let file = std::env::temp_dir().join(format!("wander_own_test_{}.json", std::process::id()));
        let _ = fs::remove_file(&file);
        unsafe {
            std::env::set_var("WANDER_OWNED_FILE", &file);
        }

        // 初始库存为空
        let own = Ownership::load();
        assert_eq!(own.skeleton_blade, 0, "初始库存不该有骷髅刀");
        assert_eq!(own.shadow_bow, 0, "初始库存不该有暗影弓");

        // 第一局：背包 3 把骷髅刀
        let mut g = Game::new(60, 25, &Ownership::default());
        g.blade_count = 3;
        g.bow_count = 0;
        g.save_ownership();
        let own = Ownership::load();
        assert_eq!(own.skeleton_blade, 3, "骷髅刀没累计进库存");
        assert_eq!(own.shadow_bow, 0, "没带弓不该记库存");

        // 第二局：背包 2 把骷髅刀 + 1 把暗影弓，跨局累加
        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.blade_count = 2;
        g2.bow_count = 1;
        g2.save_ownership();
        let own = Ownership::load();
        assert_eq!(own.skeleton_blade, 5, "骷髅刀库存该跨局累加（3+2）");
        assert_eq!(own.shadow_bow, 1, "暗影弓库存该记上");

        // 清理
        let _ = fs::remove_file(&file);
    }

    /// 首页面板：所有行显示宽度一致（40 列），边框不错位。
    #[test]
    fn homepage_panel_is_aligned() {
        let own = Ownership {
            skeleton_blade: 7,
            shadow_bow: 4,
            ..Default::default()
        };
        let s = render_homepage(&own, 12345);
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
            .filter(|l| {
                l.contains('║')
                    || l.contains('╔')
                    || l.contains('╚')
                    || l.contains('╠')
            })
            .collect();
        assert!(rows.len() >= 10, "首页面板行太少：{}", rows.len());
        let w = display_width(&rows[0]);
        assert!(
            rows.iter().all(|l| display_width(l) == w),
            "首页边框错位：{rows:?}"
        );
    }

    /// 骷髅刀升级：需要 20 把骷髅刀，升级后等级 1，攻击范围 2，伤害加成 +8。
    #[test]
    fn blade_upgrade_requires_20_and_grants_level1() {
        let mut g = Game::new(60, 25, &Ownership::default());
        // 没有 20 把：无法升级
        g.blade_count = UPGRADE_COST - 1;
        g.upgrade_blade();
        assert_eq!(g.blade_level, 0, "没足够骷髅刀不该升级");
        assert!(g.msg.contains("需要"), "应该提示需要更多骷髅刀");

        // 有 20 把：升级成功，消费 20 把
        g.blade_count = UPGRADE_COST;
        g.upgrade_blade();
        assert_eq!(g.blade_level, 1, "升级后等级该是 1");
        assert_eq!(g.blade_count, 0, "升级应消费 20 把骷髅刀");
        assert!(g.msg.contains("升级"), "应该提示升级成功");

        // 已升级到 1 级：不能再升级
        g.blade_count = UPGRADE_COST;
        g.upgrade_blade();
        assert_eq!(g.blade_level, 1, "已经 1 级不该再升级");
        assert_eq!(g.blade_count, UPGRADE_COST, "失败升级不该消费");
    }

    /// 升级后的骷髅刀：攻击范围 2，伤害加成 +8。
    #[test]
    fn upgraded_blade_has_range_2_and_bonus_8() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.blade_count = 1;
        g.blade_level = 1;
        g.blade_equipped = true;
        assert_eq!(g.attack_range(), BLADE_RANGE_LV1, "1 级骷髅刀攻击范围该是 2");
        assert_eq!(g.attack_power(), BASE_ATK + BLADE_ATK_BONUS_LV1, "1 级骷髅刀攻击加成该是 +8");

        // 0 级对比
        g.blade_level = 0;
        assert_eq!(g.attack_range(), BLADE_RANGE, "0 级骷髅刀攻击范围该是 1");
        assert_eq!(g.attack_power(), BASE_ATK + BLADE_ATK_BONUS, "0 级骷髅刀攻击加成该是 +6");
    }

    /// 升级后的骷髅刀能攻击范围 2 的怪物。
    #[test]
    fn upgraded_blade_reaches_range_2() {
        let mut g = Game::new(60, 25, &Ownership::default());
        let (px, py) = (g.player.x, g.player.y);
        // 凿通 5x5 范围
        for dy in -2..=2 {
            for dx in -2..=2 {
                let x = px as i32 + dx;
                let y = py as i32 + dy;
                if x >= 0 && y >= 0 && x < W as i32 && y < H as i32 {
                    let i = g.map.idx(x as usize, y as usize);
                    g.map.tiles[i] = Tile::Floor;
                }
            }
        }

        // 0 级刀：攻击范围 1，无法打到 2 格外
        g.blade_count = 1;
        g.blade_level = 0;
        g.blade_equipped = true;
        assert_eq!(g.attack_range(), BLADE_RANGE);
        g.monsters.clear();
        g.monsters.push(Entity {
            x: px + 2, y: py, ch: 'g', name: "哥布林",
            hp: 1, max_hp: 1, atk: 0, sight: 0,
            behavior: Behavior::Melee, is_boss: false, shield: 0,
        });
        g.try_attack();
        assert_eq!(g.monsters.len(), 1, "0 级骷髅刀不该打到 2 格外");

        // 1 级刀：攻击范围 2，可以打到 2 格外
        g.blade_level = 1;
        assert_eq!(g.attack_range(), BLADE_RANGE_LV1, "1 级骷髅刀攻击范围该是 2");
        g.monsters.clear();
        g.monsters.push(Entity {
            x: px + 2, y: py, ch: 'g', name: "哥布林",
            hp: 1, max_hp: 1, atk: 0, sight: 0,
            behavior: Behavior::Melee, is_boss: false, shield: 0,
        });
        g.try_attack();
        assert_eq!(g.monsters.len(), 0, "1 级骷髅刀该能打到 2 格外");
    }

    /// 暗影弓升级：需要 20 把暗影弓，升级后等级 1，攻击范围 10，伤害 ×3/4。
    #[test]
    fn bow_upgrade_requires_20_and_grants_level1() {
        let mut g = Game::new(60, 25, &Ownership::default());
        // 没有 20 把：无法升级
        g.bow_count = UPGRADE_COST - 1;
        g.upgrade_bow();
        assert_eq!(g.bow_level, 0, "没足够暗影弓不该升级");

        // 有 20 把：升级成功，消费 20 把
        g.bow_count = UPGRADE_COST;
        g.upgrade_bow();
        assert_eq!(g.bow_level, 1, "升级后等级该是 1");
        assert_eq!(g.bow_count, 0, "升级应消费 20 把暗影弓");

        // 已升级到 1 级：不能再升级
        g.bow_count = UPGRADE_COST;
        g.upgrade_bow();
        assert_eq!(g.bow_level, 1, "已经 1 级不该再升级");
        assert_eq!(g.bow_count, UPGRADE_COST, "失败升级不该消费");
    }

    /// 升级后的暗影弓：攻击范围 10，伤害 ×3/4。
    #[test]
    fn upgraded_bow_has_range_10_and_damage_3_4() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.bow_count = 1;
        g.bow_level = 1;
        g.bow_equipped = true;
        assert_eq!(g.attack_range(), BOW_RANGE_LV1, "1 级暗影弓攻击范围该是 10");
        // 1 级暗影弓伤害 = atk * 3 / 4
        let atk_power = g.attack_power();
        assert_eq!(atk_power, BASE_ATK * BOW_DAMAGE_MULT_NUMER / BOW_DAMAGE_MULT_DENOM, "1 级暗影弓伤害该是 ×3/4");

        // 0 级对比
        g.bow_level = 0;
        assert_eq!(g.attack_range(), BOW_RANGE, "0 级暗影弓攻击范围该是 4");
        assert_eq!(g.attack_power(), BASE_ATK / BOW_DAMAGE_DIVISOR, "0 级暗影弓伤害该是 ÷2");
    }

    /// 升级后的暗影弓能攻击范围 10 的怪物。
    #[test]
    fn upgraded_bow_reaches_range_10() {
        let mut g = Game::new(60, 25, &Ownership::default());
        let (px, py) = (g.player.x, g.player.y);
        // 凿通一条直走廊
        for x in px..=px + 10 {
            let i = g.map.idx(x, py);
            g.map.tiles[i] = Tile::Floor;
        }

        // 0 级弓：攻击范围 4，无法打到 10 格外
        g.bow_count = 1;
        g.bow_level = 0;
        g.bow_equipped = true;
        assert_eq!(g.attack_range(), BOW_RANGE);
        g.monsters.clear();
        g.monsters.push(Entity {
            x: px + 10, y: py, ch: 'g', name: "哥布林",
            hp: 1, max_hp: 1, atk: 0, sight: 0,
            behavior: Behavior::Melee, is_boss: false, shield: 0,
        });
        g.try_attack();
        assert_eq!(g.monsters.len(), 1, "0 级暗影弓不该打到 10 格外");

        // 1 级弓：攻击范围 10，可以打到 10 格外
        g.bow_level = 1;
        assert_eq!(g.attack_range(), BOW_RANGE_LV1, "1 级暗影弓攻击范围该是 10");
        g.monsters.clear();
        g.monsters.push(Entity {
            x: px + 10, y: py, ch: 'g', name: "哥布林",
            hp: 1, max_hp: 1, atk: 0, sight: 0,
            behavior: Behavior::Melee, is_boss: false, shield: 0,
        });
        g.try_attack();
        assert_eq!(g.monsters.len(), 0, "1 级暗影弓该能打到 10 格外");
    }

    /// 升级后的暗影弓伤害为 ×3/4（比 0 级 ÷2 高）。
    #[test]
    fn upgraded_bow_damage_is_3_4_not_half() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.bow_count = 1;
        g.bow_equipped = true;

        // 0 级：÷2
        g.bow_level = 0;
        let dmg0 = g.attack_power();
        assert_eq!(dmg0, BASE_ATK / 2, "0 级暗影弓伤害应为 4");

        // 1 级：×3/4 = 6（比 ÷2 的 4 高）
        g.bow_level = 1;
        let dmg1 = g.attack_power();
        assert_eq!(dmg1, BASE_ATK * 3 / 4, "1 级暗影弓伤害应为 6");
        assert!(dmg1 > dmg0, "1 级暗影弓伤害应比 0 级更高");
    }

    /// 升级等级从库存继承到新局。
    #[test]
    fn weapon_levels_inherited_from_ownership() {
        let own = Ownership {
            skeleton_blade: 5,
            shadow_bow: 5,
            blade_level: 1,
            bow_level: 1,
            ..Default::default()
        };
        let g = Game::new(60, 25, &own);
        assert_eq!(g.blade_level, 1, "骷髅刀等级应从库存继承");
        assert_eq!(g.bow_level, 1, "暗影弓等级应从库存继承");

        // 0 级的情况
        let own0 = Ownership {
            skeleton_blade: 5,
            shadow_bow: 5,
            ..Default::default()
        };
        let g0 = Game::new(60, 25, &own0);
        assert_eq!(g0.blade_level, 0, "默认等级应为 0");
        assert_eq!(g0.bow_level, 0, "默认等级应为 0");
    }

    /// 升级等级在结算后保存到库存。
    #[test]
    fn weapon_levels_saved_to_ownership() {
        let file = std::env::temp_dir().join(format!("wander_upgrade_test_{}.json", std::process::id()));
        let _ = fs::remove_file(&file);
        unsafe {
            std::env::set_var("WANDER_OWNED_FILE", &file);
        }

        let mut g = Game::new(60, 25, &Ownership::default());
        g.blade_level = 1;
        g.bow_level = 1;
        g.save_ownership();

        let own = Ownership::load();
        assert_eq!(own.blade_level, 1, "骷髅刀等级应保存");
        assert_eq!(own.bow_level, 1, "暗影弓等级应保存");

        let _ = fs::remove_file(&file);
    }

    /// 武器菜单显示升级选项 [4] 和 [5]。
    #[test]
    fn weapon_menu_shows_upgrade_options() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.toggle_weapon_menu(); // 打开武器菜单
        let s = g.render();
        assert!(s.contains("[4] 升级骷髅刀"), "武器菜单应该显示 [4] 升级骷髅刀");
        assert!(s.contains("[5] 升级暗影弓"), "武器菜单应该显示 [5] 升级暗影弓");
    }

    /// 6-7 层：骷髅绝迹，近战全是自带破盾的盾卫（7 层比 6 层多），
    /// 暗影射手比 3-5 层的 3/4/5 只明显增多。
    #[test]
    fn floor_6_7_swap_skeletons_for_more_archers_and_guards() {
        let mut map = Map::new();
        map.generate(None);
        let counts = |f: u8| {
            let (ms, _) = populate(f, &map);
            (
                ms.iter().filter(|m| !m.is_boss && m.behavior == Behavior::Melee).count(),
                ms.iter().filter(|m| m.behavior == Behavior::Ranged).count(),
            )
        };
        let (_m5, r5) = counts(5);
        let (m6, r6) = counts(6);
        let (m7, r7) = counts(7);

        assert!(r6 > r5 && r7 > r6, "6-7 层射手该比前几层更多：5层{r5} 6层{r6} 7层{r7}");
        assert!(m7 > m6, "7 层盾卫该比 6 层多：6层{m6} 7层{m7}");
        for f in [6u8, 7] {
            let (ms, _) = populate(f, &map);
            assert!(ms.iter().all(|m| m.is_boss || m.name != SKELETON), "第 {f} 层不该再有骷髅");
            let guards: Vec<_> = ms
                .iter()
                .filter(|m| !m.is_boss && m.behavior == Behavior::Melee)
                .collect();
            assert!(guards.iter().all(|m| m.name == SHIELD_GUARD), "第 {f} 层近战该全是盾卫");
            assert!(
                guards
                    .iter()
                    .all(|m| (GUARD_SHIELD_MIN..=GUARD_SHIELD_MAX).contains(&m.shield)),
                "盾卫的破盾耐久该在 {}..={} 之间",
                GUARD_SHIELD_MIN,
                GUARD_SHIELD_MAX
            );
            assert!(guards.iter().all(|m| m.shield > 0), "盾卫该真带着一面破盾");
        }
    }

    /// 盾卫举着破盾：玩家的攻击先啃它的耐久（不掉血），耐久归零后盾碎、才开始掉血。
    #[test]
    fn shield_guard_absorbs_player_hits_until_its_shield_breaks() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 2 {
            let i = g.map.idx(x, py);
            g.map.tiles[i] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 1,
            y: py,
            ch: 't',
            name: SHIELD_GUARD,
            hp: 10,
            max_hp: 10,
            atk: 0,
            sight: 0, // 看不见玩家，只会瞎逛，不反击
            behavior: Behavior::Melee,
            is_boss: false,
            shield: 3,
        });

        // 前三下只掉盾的耐久，怪不掉血（第三下把盾打碎）
        for i in 0..3 {
            g.monsters[0].x = px + 1;
            g.monsters[0].y = py;
            g.try_attack();
            if i < 2 {
                assert!(g.msg.contains("举盾挡下你的攻击"), "该提示举盾挡下：{}", g.msg);
            } else {
                assert!(g.msg.contains("被击碎了"), "第三下该把破盾打碎：{}", g.msg);
            }
        }
        assert_eq!(g.monsters[0].shield, 0, "三下之后破盾该被啃光");
        assert_eq!(g.monsters[0].hp, 10, "举盾时盾卫不该掉血");
        assert!(g.kills == 0, "盾还在的时候不该能杀死它");

        // 盾碎了才开始掉血，几下就能砍死
        for _ in 0..10 {
            if g.monsters.is_empty() {
                break;
            }
            g.monsters[0].x = px + 1;
            g.monsters[0].y = py;
            g.try_attack();
        }
        assert!(g.monsters.is_empty(), "盾碎之后该几下砍死盾卫");
        assert_eq!(g.kills, 1, "该记 1 杀");
        assert!(g.species_killed.contains(SHIELD_GUARD), "图鉴该记上盾卫");
    }

    /// 6-7 层守护者的暗箭：20 格外也能锁定玩家，穿透盾牌（不扣玩家耐久），
    /// 固定伤害 BOSS_DARK_ARROW_DMG、波动 1 点。
    #[test]
    fn boss_dark_arrow_pierces_shield_and_locks_on_beyond_20() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        // 从玩家向右凿一条直走廊，长度超过 20 格
        for x in px..=(px + 25).min(W - 1) {
            let i = g.map.idx(x, py);
            g.map.tiles[i] = Tile::Floor;
        }
        // 守护者站在 22 格外（> 20），用 Hybrid 行为
        g.monsters.push(Entity {
            x: (px + 22).min(W - 1),
            y: py,
            ch: 'D',
            name: BOSS,
            hp: 200,
            max_hp: 200,
            atk: 15,
            sight: BOSS_LOCK_RANGE,
            behavior: Behavior::Hybrid,
            is_boss: true,
            shield: 0,
        });
        let dist = g.monsters[0].x.abs_diff(px);
        assert!(dist > 20, "守护者该站在 20 格外：实际 {dist} 格");

        g.player.hp = g.player.max_hp;
        g.shield = SHIELD_DURABILITY; // 满盾：暗箭该穿透而不是被挡下
        let mut pierces = 0usize;
        let mut dmg_tally = Vec::new();
        for _ in 0..20 {
            let hp0 = g.player.hp;
            g.end_turn();
            if g.player.hp < hp0 {
                pierces += 1;
                let dmg = hp0 - g.player.hp;
                dmg_tally.push(dmg);
                assert!(
                    (BOSS_DARK_ARROW_DMG - 1..=BOSS_DARK_ARROW_DMG + 1).contains(&dmg),
                    "暗箭伤害该是 {}±1，实际 {dmg}",
                    BOSS_DARK_ARROW_DMG
                );
                assert!(g.msg.contains("穿透盾牌"), "暗箭该穿盾：{}", g.msg);
            }
        }
        assert!(pierces > 0, "20 格外的守护者一次都没射中，锁定不生效");
        let avg = dmg_tally.iter().sum::<i32>() / dmg_tally.len() as i32;
        assert!(
            (BOSS_DARK_ARROW_DMG - 1..=BOSS_DARK_ARROW_DMG + 1).contains(&avg),
            "暗箭平均伤害该落在 {}±1 附近，实际 {avg}",
            BOSS_DARK_ARROW_DMG
        );
        // 暗箭穿盾：玩家盾牌耐久一点没扣
        assert_eq!(g.shield, SHIELD_DURABILITY, "穿透盾牌的暗箭不该扣玩家耐久");
        assert_eq!(g.arrows_blocked, 0, "穿盾的暗箭不算被挡下");

        // 对照：普通暗影射手视野只有 12 格，同样距离看不见玩家，箭都射不出来
        let mut g2 = Game::new(60, 25, &Ownership::default());
        g2.monsters.clear();
        let (px, py) = (g2.player.x, g2.player.y);
        for x in px..=(px + 25).min(W - 1) {
            let i = g2.map.idx(x, py);
            g2.map.tiles[i] = Tile::Floor;
        }
        g2.monsters.push(Entity {
            x: (px + 22).min(W - 1),
            y: py,
            ch: 'o',
            name: "暗影射手",
            hp: 200,
            max_hp: 200,
            atk: 9,
            sight: 12,
            behavior: Behavior::Ranged,
            is_boss: false,
            shield: 0,
        });
        g2.player.hp = g2.player.max_hp;
        for _ in 0..20 {
            g2.end_turn();
            assert_eq!(g2.player.hp, g2.player.max_hp, "12 格外普通射手看不见玩家，不该掉血");
        }
    }

    /// 守护者贴身（1 格）时 Hybrid 走近战：伤害跟着它的 atk 走，不再射暗箭。
    #[test]
    fn hybrid_boss_swings_melee_up_close() {
        let mut g = Game::new(60, 25, &Ownership::default());
        g.monsters.clear();
        let (px, py) = (g.player.x, g.player.y);
        for x in px..=px + 2 {
            let i = g.map.idx(x, py);
            g.map.tiles[i] = Tile::Floor;
        }
        g.monsters.push(Entity {
            x: px + 1,
            y: py,
            ch: 'D',
            name: BOSS,
            hp: 200,
            max_hp: 200,
            atk: 15,
            sight: BOSS_LOCK_RANGE,
            behavior: Behavior::Hybrid,
            is_boss: true,
            shield: 0,
        });
        g.player.hp = g.player.max_hp;
        let mut hurt = false;
        for _ in 0..12 {
            let hp0 = g.player.hp;
            g.end_turn();
            if g.player.hp < hp0 {
                hurt = true;
                assert!(!g.msg.contains("暗箭"), "贴身时不该射箭：{}", g.msg);
                assert!(g.msg.contains("重击"), "贴身该抡近战：{}", g.msg);
                break;
            }
        }
        assert!(hurt, "贴身的守护者该能近战打到玩家");
    }

    /// 骷髅刀对盾卫有 50% 概率穿透盾牌（幸运符不提升）：穿透时盾牌耐久不减、直接打到肉。
    /// 验证判定条件：装备刀 + 盾卫 + 随机数落在前半区才穿透；空手/非盾卫/随机数落在后半区都不穿透。
    #[test]
    fn blade_pierces_guard_shield_half_the_time() {
        // 常量本身：50%，且幸运符不提升（常量不含 lucky 分支）
        assert_eq!(BLADE_PIERCE_CHANCE, 50);

        // 穿透判定：装备骷髅刀 + 目标是盾卫 + 随机数 < 50
        let cond = |blade_eq: bool, name: &str, roll: u32| {
            blade_eq && name == SHIELD_GUARD && roll < BLADE_PIERCE_CHANCE
        };
        assert!(cond(true, SHIELD_GUARD, 0), "随机数 0 < 50 应穿透");
        assert!(cond(true, SHIELD_GUARD, 49), "随机数 49 < 50 应穿透");
        assert!(!cond(true, SHIELD_GUARD, 50), "随机数 50 不穿透");
        assert!(!cond(true, SHIELD_GUARD, 99), "随机数 99 不穿透");
        assert!(!cond(false, SHIELD_GUARD, 0), "空手不穿透");
        assert!(!cond(true, "暗影射手", 0), "非盾卫不穿透");
        assert!(!cond(true, "地牢守护者", 0), "守护者（无盾）不穿透");
    }

    /// 暗箭穿盾不扣耐久：守护者的暗箭与骷髅一样无视盾牌，但会正常掉血。
    #[test]
    fn dark_arrow_ignores_shield_durability() {
        let mut shield = SHIELD_DURABILITY;
        let mut stock = Vec::new();
        let mut hits: Vec<String> = Vec::new();
        let r = block_with_shield(&mut shield, &mut stock, BOSS, "暗箭", true, &mut hits);
        assert!(!r.blocked, "穿盾的暗箭不该被挡下");
        assert_eq!(shield, SHIELD_DURABILITY, "穿盾的暗箭不该扣盾牌耐久");

        // 普通射手的暗影箭照样被盾挡下并扣耐久
        let mut shield2 = SHIELD_DURABILITY;
        let mut stock2 = Vec::new();
        let mut hits2: Vec<String> = Vec::new();
        let r2 = block_with_shield(&mut shield2, &mut stock2, "暗影射手", "暗影箭", false, &mut hits2);
        assert!(r2.blocked, "普通暗影箭该被盾挡下");
        assert_eq!(shield2, SHIELD_DURABILITY - 1, "普通暗影箭该扣 1 点耐久");
    }
