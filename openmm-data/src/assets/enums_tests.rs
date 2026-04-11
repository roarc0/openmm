use super::*;

#[test]
fn evt_opcode_round_trip() {
    assert_eq!(EvtOpcode::from_u8(0), Some(EvtOpcode::Invalid));
    assert_eq!(EvtOpcode::from_u8(1), Some(EvtOpcode::Exit));
    assert_eq!(EvtOpcode::from_u8(6), Some(EvtOpcode::MoveToMap));
    assert_eq!(EvtOpcode::from_u8(15), Some(EvtOpcode::ChangeDoorState));
    assert_eq!(EvtOpcode::from_u8(68), Some(EvtOpcode::IsNPCInParty));
    assert_eq!(EvtOpcode::from_u8(20), Some(EvtOpcode::Unknown20)); // named unknown slot
    assert_eq!(EvtOpcode::from_u8(255), None);
}

#[test]
fn evt_opcode_name_matches_variant() {
    assert_eq!(EvtOpcode::MoveToMap.name(), "MoveToMap");
    assert_eq!(EvtOpcode::Exit.name(), "Exit");
    assert_eq!(EvtOpcode::ChangeDoorState.name(), "ChangeDoorState");
}

#[test]
fn ai_state_round_trip() {
    assert_eq!(AIState::from_u16(0), Some(AIState::Standing));
    assert_eq!(AIState::from_u16(6), Some(AIState::Pursuing));
    assert_eq!(AIState::from_u16(19), Some(AIState::Disabled));
    assert_eq!(AIState::from_u16(20), None);
    assert_eq!(AIState::from_u16(100), None);
}

#[test]
fn monster_movement_type_round_trip() {
    assert_eq!(MonsterMovementType::from_u8(0), Some(MonsterMovementType::Short));
    assert_eq!(MonsterMovementType::from_u8(5), Some(MonsterMovementType::Stationary));
    assert_eq!(MonsterMovementType::from_u8(6), None);
}

#[test]
fn monster_ai_type_round_trip() {
    assert_eq!(MonsterAIType::from_u8(0), Some(MonsterAIType::Suicide));
    assert_eq!(MonsterAIType::from_u8(3), Some(MonsterAIType::Aggressive));
    assert_eq!(MonsterAIType::from_u8(4), None);
}

#[test]
fn monster_hostility_round_trip() {
    assert_eq!(MonsterHostility::from_u8(0), Some(MonsterHostility::Friendly));
    assert_eq!(MonsterHostility::from_u8(4), Some(MonsterHostility::Long));
    assert_eq!(MonsterHostility::from_u8(5), None);
}

#[test]
fn monster_special_attack_round_trip() {
    assert_eq!(MonsterSpecialAttack::from_u8(0), Some(MonsterSpecialAttack::None));
    assert_eq!(MonsterSpecialAttack::from_u8(23), Some(MonsterSpecialAttack::Fear));
    assert_eq!(MonsterSpecialAttack::from_u8(24), None);
}

#[test]
fn monster_special_ability_round_trip() {
    assert_eq!(MonsterSpecialAbility::from_u8(0), Some(MonsterSpecialAbility::None));
    assert_eq!(MonsterSpecialAbility::from_u8(3), Some(MonsterSpecialAbility::Explode));
    assert_eq!(MonsterSpecialAbility::from_u8(4), None);
}

#[test]
fn polygon_type_ceiling_detection() {
    assert!(PolygonType::Ceiling.is_ceiling());
    assert!(PolygonType::InBetweenCeilingAndWall.is_ceiling());
    assert!(!PolygonType::Floor.is_ceiling());
    assert!(!PolygonType::VerticalWall.is_ceiling());
    assert!(!PolygonType::Invalid.is_ceiling());
}

#[test]
fn door_action_round_trip() {
    assert_eq!(DoorAction::from_u8(0), Some(DoorAction::GoToOpen));
    assert_eq!(DoorAction::from_u8(3), Some(DoorAction::Toggle));
    assert_eq!(DoorAction::from_u8(4), None);
    assert_eq!(DoorAction::GoToClosed.as_u8(), 1);
    assert_eq!(DoorAction::ToggleIfStopped.as_u8(), 2);
}

#[test]
fn door_action_display() {
    assert_eq!(format!("{}", DoorAction::GoToOpen), "GoToOpen");
    assert_eq!(format!("{}", DoorAction::Toggle), "Toggle");
}

#[test]
fn evt_variable_is_map_var() {
    assert!(EvtVariable(0x69).is_map_var()); // MapVar0
    assert!(EvtVariable(0xCC).is_map_var()); // MapVar99
    assert!(!EvtVariable(0x68).is_map_var()); // just before range (CondMain)
    assert!(!EvtVariable(0xCD).is_map_var()); // just after range (AutonotesBits)
}

#[test]
fn evt_variable_map_var_index() {
    assert_eq!(EvtVariable(0x69).map_var_index(), Some(0)); // MapVar0
    assert_eq!(EvtVariable(0xCC).map_var_index(), Some(99)); // MapVar99
    assert_eq!(EvtVariable::HP.map_var_index(), None);
}

#[test]
fn evt_variable_is_skill() {
    assert!(EvtVariable::SKILL_STAFF.is_skill());
    assert!(EvtVariable::SKILL_MISC.is_skill());
    assert!(!EvtVariable::HP.is_skill());
    assert!(!EvtVariable(0x69).is_skill()); // map var, not skill
}

#[test]
fn evt_variable_skill_index() {
    assert_eq!(EvtVariable::SKILL_STAFF.skill_index(), Some(0));
    assert_eq!(EvtVariable::SKILL_MISC.skill_index(), Some(30));
    assert_eq!(EvtVariable::HP.skill_index(), None);
}

#[test]
fn evt_variable_is_character_scoped() {
    assert!(EvtVariable::HP.is_character_scoped()); // 0x03
    assert!(EvtVariable::SKILL_MISC.is_character_scoped()); // 0x56
    assert!(EvtVariable::COND_MAIN.is_character_scoped()); // 0x68
    assert!(!EvtVariable(0x00).is_character_scoped()); // before range
    assert!(!EvtVariable(0x69).is_character_scoped()); // start of map vars
    assert!(EvtVariable::GOLD.is_character_scoped()); // 0x15 is in the range
}

#[test]
fn evt_variable_display_map_var() {
    assert_eq!(format!("{}", EvtVariable(0x69)), "MapVar0");
    assert_eq!(format!("{}", EvtVariable(0x6A)), "MapVar1");
    assert_eq!(format!("{}", EvtVariable(0xCC)), "MapVar99");
}

#[test]
fn evt_variable_display_named() {
    assert_eq!(format!("{}", EvtVariable::HP), "HP");
    assert_eq!(format!("{}", EvtVariable::GOLD), "Gold");
}

#[test]
fn actor_attributes_bitflags() {
    let attrs = ActorAttributes::HOSTILE | ActorAttributes::ACTIVE;
    assert!(attrs.contains(ActorAttributes::HOSTILE));
    assert!(attrs.contains(ActorAttributes::ACTIVE));
    assert!(!attrs.contains(ActorAttributes::FLEEING));
}

#[test]
fn tile_flags_default_is_empty() {
    assert_eq!(TileFlags::default(), TileFlags::empty());
    let flags = TileFlags::WATER | TileFlags::WAVY;
    assert!(flags.contains(TileFlags::WATER));
    assert!(!flags.contains(TileFlags::BURN));
}

#[test]
fn sound_type_round_trip() {
    assert_eq!(SoundType::from_u32(0), Some(SoundType::LevelSpecific));
    assert_eq!(SoundType::from_u32(4), Some(SoundType::Lock));
    assert_eq!(SoundType::from_u32(5), None);
}

#[test]
fn evt_target_character_round_trip() {
    assert_eq!(EvtTargetCharacter::from_u8(0), Some(EvtTargetCharacter::Player1));
    assert_eq!(EvtTargetCharacter::from_u8(6), Some(EvtTargetCharacter::Random));
    assert_eq!(EvtTargetCharacter::from_u8(7), None);
}
