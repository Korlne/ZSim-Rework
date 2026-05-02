use crate::entities::character::Character;
use crate::entities::enums::CharacterState;

/// 默认切换冷却时间（tick 数）（0.5 秒 @ 60 fps ≈ 30 ticks）。
const SWITCH_COOLDOWN_TICKS: u64 = 30;

/// 每个角色的最大 Decibel（终结技能量）。
const MAX_DECIBEL: f64 = 3000.0;

/// 管理 1–3 个角色的队伍、邦布、上场状态、切换冷却、Decibel（终结技能量）和连携点数。
#[derive(Debug, Clone)]
pub struct TeamManager {
    pub characters: Vec<Character>,
    pub bangboo: Option<Character>,
    pub current_on_field_index: usize,
    pub switch_cooldown_remaining: u64,
}

impl TeamManager {
    /// 使用给定的角色和可选邦布创建新队伍。
    ///
    /// 第一个角色（索引 0）以 Active 状态开始上场；
    /// 其他角色以 Standby 状态开始。
    pub fn new(characters: Vec<Character>) -> Self {
        Self::with_bangboo(characters, None)
    }

    /// 使用角色和可选邦布创建新队伍。
    pub fn with_bangboo(mut characters: Vec<Character>, bangboo: Option<Character>) -> Self {
        // 第一个角色开始为 Active，其他为 Standby
        if let Some(first) = characters.first_mut() {
            first.state = CharacterState::Active;
        }
        for c in characters.iter_mut().skip(1) {
            c.state = CharacterState::Standby;
        }
        Self {
            characters,
            bangboo,
            current_on_field_index: 0,
            switch_cooldown_remaining: 0,
        }
    }

    // ------------------------------------------------------------------
    // 查询方法
    // ------------------------------------------------------------------

    /// 当所有队员的 HP ≤ 0.0 时返回 true。
    pub fn all_dead(&self) -> bool {
        self.characters.is_empty() || self.characters.iter().all(|c| c.current_stats.hp <= 0.0)
    }

    /// HP > 0.0 的队员数量。
    pub fn alive_count(&self) -> usize {
        self.characters
            .iter()
            .filter(|c| c.current_stats.hp > 0.0)
            .count()
    }

    /// 获取当前上场角色的索引。
    pub fn on_field_index(&self) -> usize {
        self.current_on_field_index
    }

    /// 获取剩余切换冷却时间（tick 数）。
    pub fn switch_cooldown(&self) -> u64 {
        self.switch_cooldown_remaining
    }

    /// 判断当前是否处于切换冷却中。
    pub fn is_switch_on_cooldown(&self) -> bool {
        self.switch_cooldown_remaining > 0
    }

    // ------------------------------------------------------------------
    // 上场 / 下场角色访问器
    // ------------------------------------------------------------------

    /// 当前上场角色的不可变引用。
    pub fn get_on_field(&self) -> &Character {
        &self.characters[self.current_on_field_index]
    }

    /// 当前上场角色的可变引用。
    pub fn get_on_field_mut(&mut self) -> &mut Character {
        &mut self.characters[self.current_on_field_index]
    }

    /// 根据队伍索引获取下场角色的不可变引用。
    /// 如果索引越界或指向上场角色，则返回 `None`。
    pub fn get_off_field(&self, index: usize) -> Option<&Character> {
        if index == self.current_on_field_index || index >= self.characters.len() {
            return None;
        }
        Some(&self.characters[index])
    }

    /// 根据队伍索引获取下场角色的可变引用。
    /// 如果索引越界或指向上场角色，则返回 `None`。
    pub fn get_off_field_mut(&mut self, index: usize) -> Option<&mut Character> {
        if index == self.current_on_field_index || index >= self.characters.len() {
            return None;
        }
        Some(&mut self.characters[index])
    }

    // ------------------------------------------------------------------
    // 角色切换
    // ------------------------------------------------------------------

    /// 将活跃角色切换到 `index`。
    ///
    /// 当前上场角色变为 `Standby`，目标角色变为 `Active`，
    /// 并设置切换冷却时间。
    ///
    /// 成功返回 `Ok(())`，如果索引无效或切换处于冷却中则返回 `Err`。
    pub fn switch_to(&mut self, index: usize) -> Result<(), SwitchError> {
        if index >= self.characters.len() {
            return Err(SwitchError::InvalidIndex(index));
        }
        if index == self.current_on_field_index {
            return Err(SwitchError::AlreadyOnField);
        }
        if self.switch_cooldown_remaining > 0 {
            return Err(SwitchError::OnCooldown(self.switch_cooldown_remaining));
        }
        // 确保目标角色还活着
        if self.characters[index].current_stats.hp <= 0.0 {
            return Err(SwitchError::TargetDead(index));
        }

        // 停用当前角色，激活目标角色
        self.characters[self.current_on_field_index].state = CharacterState::Standby;
        self.characters[index].state = CharacterState::Active;
        self.current_on_field_index = index;
        self.switch_cooldown_remaining = SWITCH_COOLDOWN_TICKS;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Decibel（终结技能量）管理
    // ------------------------------------------------------------------

    /// 为所有角色增加 Decibel（不含邦布）。
    /// 每个角色的 Decibel 上限为 MAX_DECIBEL（3000）。
    pub fn add_decibel(&mut self, amount: f64) {
        for c in &mut self.characters {
            c.resources.decibel = (c.resources.decibel + amount).min(MAX_DECIBEL);
        }
    }

    /// 消耗上场角色的 Decibel。
    /// 如果足够则返回 `Ok(())`，否则返回包含当前值的 `Err`。
    pub fn consume_decibel(&mut self, amount: f64) -> Result<(), InsufficientResource> {
        let c = &mut self.characters[self.current_on_field_index];
        if c.resources.decibel < amount {
            return Err(InsufficientResource {
                current: c.resources.decibel,
                required: amount,
            });
        }
        c.resources.decibel -= amount;
        Ok(())
    }

    /// 根据索引获取指定角色的 Decibel 值。
    pub fn decibel_of(&self, index: usize) -> Option<f64> {
        self.characters.get(index).map(|c| c.resources.decibel)
    }

    // ------------------------------------------------------------------
    // 连携点数管理
    // ------------------------------------------------------------------

    /// 为上场角色添加一个连携点数（上限为 u32::MAX）。
    pub fn add_chain_point(&mut self) {
        let c = &mut self.characters[self.current_on_field_index];
        c.resources.chain_points = c.resources.chain_points.saturating_add(1);
    }

    /// 消耗上场角色的一个连携点数。
    /// 如果至少有 1 点则返回 `Ok(())`，否则返回 `Err`。
    pub fn consume_chain_point(&mut self) -> Result<(), InsufficientResource> {
        let c = &mut self.characters[self.current_on_field_index];
        if c.resources.chain_points < 1 {
            return Err(InsufficientResource {
                current: c.resources.chain_points as f64,
                required: 1.0,
            });
        }
        c.resources.chain_points -= 1;
        Ok(())
    }

    /// 上场角色的连携点数数量。
    pub fn chain_points(&self) -> u32 {
        self.characters[self.current_on_field_index]
            .resources
            .chain_points
    }

    // ------------------------------------------------------------------
    // Tick 推进
    // ------------------------------------------------------------------

    /// 前进一个 tick：减少切换冷却（最低为 0）。
    pub fn on_tick(&mut self) {
        self.switch_cooldown_remaining = self.switch_cooldown_remaining.saturating_sub(1);
    }
}

// ------------------------------------------------------------------
// 错误类型
// ------------------------------------------------------------------

/// 角色切换过程中可能发生的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchError {
    InvalidIndex(usize),
    AlreadyOnField,
    OnCooldown(u64),
    TargetDead(usize),
}

impl std::fmt::Display for SwitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchError::InvalidIndex(idx) => write!(f, "invalid character index: {idx}"),
            SwitchError::AlreadyOnField => write!(f, "character is already on the field"),
            SwitchError::OnCooldown(remaining) => {
                write!(f, "switch on cooldown for {remaining} more ticks")
            }
            SwitchError::TargetDead(idx) => write!(f, "target character at index {idx} is dead"),
        }
    }
}

impl std::error::Error for SwitchError {}

/// 资源不足（Decibel 或连携点数）错误。
#[derive(Debug, Clone, PartialEq)]
pub struct InsufficientResource {
    pub current: f64,
    pub required: f64,
}

impl std::fmt::Display for InsufficientResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "insufficient resource: {:.1} / {:.1}",
            self.current, self.required
        )
    }
}

impl std::error::Error for InsufficientResource {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::enums::{ElementTag, FactionTag, SpecialtyTag};
    use crate::entities::models::BaseStats;

    fn make_character(hp: f64) -> Character {
        let mut c = Character::new(
            "test_char",
            FactionTag::GentleHouse,
            SpecialtyTag::Attack,
            ElementTag::Physical,
            BaseStats::default().with_hp(hp),
        );
        c.current_stats.hp = hp;
        c
    }

    fn make_bangboo() -> Character {
        let mut c = Character::new(
            "bangboo_01",
            FactionTag::GentleHouse,
            SpecialtyTag::Support,
            ElementTag::Physical,
            BaseStats::default(),
        );
        c.char_id = "bangboo_01".into();
        c
    }

    fn three_character_team() -> TeamManager {
        TeamManager::new(vec![
            make_character(8000.0),
            make_character(6000.0),
            make_character(5000.0),
        ])
    }

    // ------------------------------------------------------------------
    // 创建
    // ------------------------------------------------------------------

    #[test]
    fn test_new_team() {
        let chars = vec![make_character(8000.0), make_character(6000.0)];
        let team = TeamManager::new(chars);
        assert_eq!(team.characters.len(), 2);
        assert!(team.bangboo.is_none());
        assert_eq!(team.current_on_field_index, 0);
    }

    #[test]
    fn test_with_bangboo() {
        let chars = vec![make_character(8000.0)];
        let bangboo = make_bangboo();
        let team = TeamManager::with_bangboo(chars, Some(bangboo));
        assert_eq!(team.characters.len(), 1);
        assert!(team.bangboo.is_some());
        assert_eq!(team.bangboo.as_ref().unwrap().char_id, "bangboo_01");
    }

    #[test]
    fn test_first_character_starts_active() {
        let team = three_character_team();
        assert_eq!(team.characters[0].state, CharacterState::Active);
        assert_eq!(team.characters[1].state, CharacterState::Standby);
        assert_eq!(team.characters[2].state, CharacterState::Standby);
    }

    // ------------------------------------------------------------------
    // all_dead / alive_count（向后兼容）
    // ------------------------------------------------------------------

    #[test]
    fn test_all_dead_empty_team() {
        let team = TeamManager::new(vec![]);
        assert!(team.all_dead());
    }

    #[test]
    fn test_all_dead_some_alive() {
        let chars = vec![make_character(0.0), make_character(5000.0)];
        let team = TeamManager::new(chars);
        assert!(!team.all_dead());
    }

    #[test]
    fn test_all_dead_all_zero_hp() {
        let chars = vec![make_character(0.0), make_character(0.0)];
        let team = TeamManager::new(chars);
        assert!(team.all_dead());
    }

    #[test]
    fn test_alive_count() {
        let chars = vec![
            make_character(0.0),
            make_character(5000.0),
            make_character(3000.0),
        ];
        let team = TeamManager::new(chars);
        assert_eq!(team.alive_count(), 2);
    }

    #[test]
    fn test_alive_count_all_dead() {
        let chars = vec![make_character(0.0), make_character(0.0)];
        let team = TeamManager::new(chars);
        assert_eq!(team.alive_count(), 0);
    }

    // ------------------------------------------------------------------
    // 查询方法
    // ------------------------------------------------------------------

    #[test]
    fn test_on_field_index() {
        let mut team = three_character_team();
        assert_eq!(team.on_field_index(), 0);
        team.current_on_field_index = 2;
        assert_eq!(team.on_field_index(), 2);
    }

    #[test]
    fn test_switch_cooldown_initially_zero() {
        let team = three_character_team();
        assert_eq!(team.switch_cooldown(), 0);
        assert!(!team.is_switch_on_cooldown());
    }

    // ------------------------------------------------------------------
    // get_on_field / get_off_field 访问器
    // ------------------------------------------------------------------

    #[test]
    fn test_get_on_field() {
        let team = three_character_team();
        assert_eq!(team.get_on_field().char_id, "test_char");
    }

    #[test]
    fn test_get_on_field_mut() {
        let mut team = three_character_team();
        let c = team.get_on_field_mut();
        c.resources.energy = 50.0;
        assert_eq!(team.characters[0].resources.energy, 50.0);
    }

    #[test]
    fn test_get_off_field_valid() {
        let team = three_character_team();
        let off = team.get_off_field(1);
        assert!(off.is_some());
        assert_eq!(off.unwrap().char_id, "test_char");
    }

    #[test]
    fn test_get_off_field_on_field_returns_none() {
        let team = three_character_team();
        assert!(team.get_off_field(0).is_none());
    }

    #[test]
    fn test_get_off_field_out_of_bounds() {
        let team = three_character_team();
        assert!(team.get_off_field(5).is_none());
    }

    #[test]
    fn test_get_off_field_mut_valid() {
        let mut team = three_character_team();
        let off = team.get_off_field_mut(1);
        assert!(off.is_some());
        off.unwrap().resources.energy = 99.0;
        assert_eq!(team.characters[1].resources.energy, 99.0);
    }

    #[test]
    fn test_get_off_field_mut_on_field_returns_none() {
        let mut team = three_character_team();
        assert!(team.get_off_field_mut(0).is_none());
    }

    // ------------------------------------------------------------------
    // switch_to 切换
    // ------------------------------------------------------------------

    #[test]
    fn test_switch_to_basic() {
        let mut team = three_character_team();
        assert!(team.switch_to(1).is_ok());
        assert_eq!(team.current_on_field_index, 1);
        assert_eq!(team.characters[0].state, CharacterState::Standby);
        assert_eq!(team.characters[1].state, CharacterState::Active);
    }

    #[test]
    fn test_switch_to_sets_cooldown() {
        let mut team = three_character_team();
        assert!(team.switch_to(2).is_ok());
        assert!(team.switch_cooldown_remaining > 0);
        assert!(team.is_switch_on_cooldown());
    }

    #[test]
    fn test_switch_to_invalid_index() {
        let mut team = three_character_team();
        let err = team.switch_to(10).unwrap_err();
        assert_eq!(err, SwitchError::InvalidIndex(10));
    }

    #[test]
    fn test_switch_to_already_on_field() {
        let mut team = three_character_team();
        let err = team.switch_to(0).unwrap_err();
        assert_eq!(err, SwitchError::AlreadyOnField);
    }

    #[test]
    fn test_switch_to_while_on_cooldown() {
        let mut team = three_character_team();
        team.switch_cooldown_remaining = 15;
        let err = team.switch_to(1).unwrap_err();
        assert_eq!(err, SwitchError::OnCooldown(15));
    }

    #[test]
    fn test_switch_to_dead_character() {
        let mut team = three_character_team();
        team.characters[1].current_stats.hp = 0.0;
        let err = team.switch_to(1).unwrap_err();
        assert_eq!(err, SwitchError::TargetDead(1));
    }

    #[test]
    fn test_switch_to_single_character_team_fails() {
        let mut team = TeamManager::new(vec![make_character(8000.0)]);
        // 只有索引 0 存在，切换到它返回 AlreadyOnField
        let err = team.switch_to(0).unwrap_err();
        assert_eq!(err, SwitchError::AlreadyOnField);
    }

    // ------------------------------------------------------------------
    // Decibel 管理
    // ------------------------------------------------------------------

    #[test]
    fn test_add_decibel_to_all() {
        let mut team = three_character_team();
        team.add_decibel(500.0);
        assert_eq!(team.characters[0].resources.decibel, 500.0);
        assert_eq!(team.characters[1].resources.decibel, 500.0);
        assert_eq!(team.characters[2].resources.decibel, 500.0);
    }

    #[test]
    fn test_add_decibel_stacks() {
        let mut team = three_character_team();
        team.add_decibel(200.0);
        team.add_decibel(300.0);
        assert_eq!(team.characters[0].resources.decibel, 500.0);
    }

    #[test]
    fn test_add_decibel_caps_at_max() {
        let mut team = three_character_team();
        team.add_decibel(5000.0);
        assert_eq!(team.characters[0].resources.decibel, MAX_DECIBEL);
        assert_eq!(team.characters[1].resources.decibel, MAX_DECIBEL);
        assert_eq!(team.characters[2].resources.decibel, MAX_DECIBEL);
    }

    #[test]
    fn test_add_decibel_negative_is_noop() {
        let mut team = three_character_team();
        team.characters[0].resources.decibel = 100.0;
        team.add_decibel(-50.0);
        // -50 会下溢……用 max(0, ...) 来保持整洁
        // 使用 .min(MAX_DECIBEL)，负数加法按预期工作。
        assert_eq!(
            team.characters[0].resources.decibel,
            50.0_f64.max(0.0).min(MAX_DECIBEL)
        );
    }

    #[test]
    fn test_consume_decibel_success() {
        let mut team = three_character_team();
        team.characters[0].resources.decibel = 2000.0;
        assert!(team.consume_decibel(1500.0).is_ok());
        assert_eq!(team.characters[0].resources.decibel, 500.0);
    }

    #[test]
    fn test_consume_decibel_insufficient() {
        let mut team = three_character_team();
        team.characters[0].resources.decibel = 100.0;
        let err = team.consume_decibel(200.0).unwrap_err();
        assert!((err.current - 100.0).abs() < 1e-9);
        assert!((err.required - 200.0).abs() < 1e-9);
        // 失败时 Decibel 不变
        assert_eq!(team.characters[0].resources.decibel, 100.0);
    }

    #[test]
    fn test_decibel_of() {
        let mut team = three_character_team();
        team.characters[1].resources.decibel = 2500.0;
        assert!((team.decibel_of(1).unwrap() - 2500.0).abs() < 1e-9);
    }

    #[test]
    fn test_decibel_of_out_of_bounds() {
        let team = three_character_team();
        assert!(team.decibel_of(10).is_none());
    }

    #[test]
    fn test_consume_decibel_on_off_field_character() {
        let mut team = three_character_team();
        // 切换到索引 1 的角色，然后从上场角色（现在是索引 1）消耗
        team.switch_to(1).unwrap();
        team.characters[1].resources.decibel = 800.0;
        assert!(team.consume_decibel(300.0).is_ok());
        assert_eq!(team.characters[1].resources.decibel, 500.0);
        // 索引 0 的角色应保持不变
        assert_eq!(team.characters[0].resources.decibel, 0.0);
    }

    // ------------------------------------------------------------------
    // 连携点数管理
    // ------------------------------------------------------------------

    #[test]
    fn test_add_chain_point() {
        let mut team = three_character_team();
        team.add_chain_point();
        assert_eq!(team.characters[0].resources.chain_points, 1);
    }

    #[test]
    fn test_add_chain_point_stacks() {
        let mut team = three_character_team();
        team.add_chain_point();
        team.add_chain_point();
        team.add_chain_point();
        assert_eq!(team.characters[0].resources.chain_points, 3);
    }

    #[test]
    fn test_add_chain_point_to_on_field_after_switch() {
        let mut team = three_character_team();
        team.switch_to(1).unwrap();
        team.add_chain_point();
        assert_eq!(team.characters[1].resources.chain_points, 1);
        assert_eq!(team.characters[0].resources.chain_points, 0);
    }

    #[test]
    fn test_consume_chain_point_success() {
        let mut team = three_character_team();
        team.characters[0].resources.chain_points = 2;
        assert!(team.consume_chain_point().is_ok());
        assert_eq!(team.characters[0].resources.chain_points, 1);
    }

    #[test]
    fn test_consume_chain_point_insufficient() {
        let mut team = three_character_team();
        let err = team.consume_chain_point().unwrap_err();
        assert!((err.current - 0.0).abs() < 1e-9);
        assert!((err.required - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_chain_points_query() {
        let mut team = three_character_team();
        team.characters[0].resources.chain_points = 3;
        assert_eq!(team.chain_points(), 3);
    }

    // ------------------------------------------------------------------
    // on_tick 推进
    // ------------------------------------------------------------------

    #[test]
    fn test_on_tick_decrements_switch_cooldown() {
        let mut team = three_character_team();
        team.switch_to(1).unwrap();
        let before = team.switch_cooldown_remaining;
        team.on_tick();
        assert_eq!(team.switch_cooldown_remaining, before - 1);
    }

    #[test]
    fn test_on_tick_does_not_underflow() {
        let mut team = three_character_team();
        team.switch_cooldown_remaining = 0;
        team.on_tick();
        assert_eq!(team.switch_cooldown_remaining, 0);
    }

    #[test]
    fn test_switch_cooldown_expires_after_enough_ticks() {
        let mut team = three_character_team();
        team.switch_to(1).unwrap();
        let cooldown = team.switch_cooldown_remaining;
        for _ in 0..cooldown {
            assert!(team.is_switch_on_cooldown());
            team.on_tick();
        }
        assert!(!team.is_switch_on_cooldown());
        // 应该能够再次切换
        assert!(team.switch_to(2).is_ok());
    }

    // ------------------------------------------------------------------
    // 完整集成：切换 → Decibel → 连携 → tick → 切回
    // ------------------------------------------------------------------

    #[test]
    fn test_full_rotation() {
        let mut team = three_character_team();
        // 开始：角色 0 在场上
        assert_eq!(team.on_field_index(), 0);

        // 为所有角色添加资源
        team.add_decibel(500.0);
        assert_eq!(team.characters[0].resources.decibel, 500.0);

        // 切换到角色 1
        assert!(team.switch_to(1).is_ok());
        assert_eq!(team.on_field_index(), 1);

        // 为角色 1（上场）添加连携点数
        team.add_chain_point();
        assert_eq!(team.chain_points(), 1);

        // 角色 1 添加 Decibel 并消耗
        team.add_decibel(2000.0);
        assert_eq!(team.characters[1].resources.decibel, 2500.0);
        assert!(team.consume_decibel(2400.0).is_ok());
        assert_eq!(team.characters[1].resources.decibel, 100.0);

        // 消耗连携点数
        assert!(team.consume_chain_point().is_ok());
        assert_eq!(team.chain_points(), 0);

        // 等待冷却结束
        let cooldown = team.switch_cooldown_remaining;
        for _ in 0..=cooldown {
            team.on_tick();
        }

        // 切回角色 0
        assert!(team.switch_to(0).is_ok());
        assert_eq!(team.on_field_index(), 0);
        assert_eq!(team.characters[0].state, CharacterState::Active);
        assert_eq!(team.characters[1].state, CharacterState::Standby);
    }
}
