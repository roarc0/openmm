# Party & Game Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a mock MM6 party with per-member skill data, wire `ForPartyMember` and `CheckSkill` EVT opcodes, add readable logging for QBit/Autonote operations, and persist quest progress in `GameSave`.

**Architecture:** A new `game/party/` module owns the `Party` resource (4 fixed members, active EVT target). `GameVariables` gains explicit logging methods for QBit/Autonote ops. `GameSave` is extended with a `SavedProgress` block. `event_dispatch.rs` is wired to read `Party` for `ForPartyMember` and `CheckSkill`.

**Tech Stack:** Rust, Bevy 0.18 ECS (Resource), serde_json (already in Cargo.toml), `lod::enums::EvtTargetCharacter` + `EvtVariable`

---

## File Map

| File | Action | Purpose |
|---|---|---|
| `lod/src/enums.rs` | Modify | Add `is_skill()` + `skill_index()` to `EvtVariable` |
| `openmm/src/game/party/member.rs` | Create | `CharacterClass` enum, `PartyMember` struct with skills array |
| `openmm/src/game/party/mod.rs` | Create | `Party` resource, mock default party, `PartyPlugin` |
| `openmm/src/game/mod.rs` | Modify | Add `pub(crate) mod party;` + register `PartyPlugin` |
| `openmm/src/game/world_state.rs` | Modify | Add `set_qbit`, `clear_qbit`, `has_qbit`, `add_autonote`, `remove_autonote`, `has_autonote` to `GameVariables` |
| `openmm/src/save.rs` | Modify | Add `SavedProgress` struct, extend `GameSave`, update `write_to_save`/`read_from_save` |
| `openmm/src/game/event_dispatch.rs` | Modify | Wire `ForPartyMember` (store to `Party`), use logging methods, implement `CheckSkill` |

---

## Task 1: Add `is_skill()` and `skill_index()` to `EvtVariable`

**Files:**
- Modify: `lod/src/enums.rs`

Skills occupy `0x38..=0x56` (31 values). We need helpers to query this range and get a 0-based index, mirroring `is_map_var()` / `map_var_index()` already in the file.

- [ ] **Step 1: Add methods to `EvtVariable`**

In `lod/src/enums.rs`, find the block after `map_var_index()` (around line 1040) and add:

```rust
    /// Returns true if this variable ID refers to a skill (SkillStaff..SkillMisc).
    pub fn is_skill(self) -> bool {
        (0x38..=0x56).contains(&self.0)
    }

    /// For skill variables, returns the skill index (0 = Staff, 30 = Misc).
    pub fn skill_index(self) -> Option<u8> {
        if self.is_skill() {
            Some(self.0 - 0x38)
        } else {
            None
        }
    }
```

- [ ] **Step 2: Build to verify**

```bash
cd /home/roarc/repos/openmm && cargo build -p lod 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add lod/src/enums.rs
git commit --no-gpg-sign -m "feat(lod): add is_skill() and skill_index() to EvtVariable"
```

---

## Task 2: Create `party/member.rs`

**Files:**
- Create: `openmm/src/game/party/member.rs`

`PartyMember` holds the character name, class, level, and a skills array indexed by `skill_index()`. Skill values are raw (level 1 = 1, level 10 = 10). Mastery is out of scope.

- [ ] **Step 1: Write the file**

```rust
use lod::enums::EvtVariable;

pub const SKILL_COUNT: usize = 31; // EvtVariable 0x38..=0x56

/// MM6 character classes (6 classes, matching the original game).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterClass {
    Knight,
    Paladin,
    Archer,
    Cleric,
    Sorcerer,
    Druid,
}

impl std::fmt::Display for CharacterClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Knight    => "Knight",
            Self::Paladin   => "Paladin",
            Self::Archer    => "Archer",
            Self::Cleric    => "Cleric",
            Self::Sorcerer  => "Sorcerer",
            Self::Druid     => "Druid",
        };
        write!(f, "{}", s)
    }
}

/// A single party member. Skills are stored as raw levels (0 = untrained).
#[derive(Debug, Clone)]
pub struct PartyMember {
    pub name: &'static str,
    pub class: CharacterClass,
    pub level: u8,
    /// Raw skill levels, indexed by `EvtVariable::skill_index()`.
    pub skills: [u8; SKILL_COUNT],
}

impl PartyMember {
    pub fn new(name: &'static str, class: CharacterClass, level: u8) -> Self {
        Self {
            name,
            class,
            level,
            skills: [0; SKILL_COUNT],
        }
    }

    /// Set a skill by its EvtVariable. No-op if variable is not a skill.
    pub fn set_skill(&mut self, var: EvtVariable, level: u8) {
        if let Some(idx) = var.skill_index() {
            self.skills[idx as usize] = level;
        }
    }

    /// Get skill level for a given EvtVariable (0 if not a skill variable).
    pub fn get_skill(&self, var: EvtVariable) -> u8 {
        var.skill_index()
            .map(|idx| self.skills[idx as usize])
            .unwrap_or(0)
    }
}
```

- [ ] **Step 2: Build**

We cannot build yet (mod not declared). Proceed to Task 3, which declares it.

---

## Task 3: Create `party/mod.rs`

**Files:**
- Create: `openmm/src/game/party/mod.rs`

`Party` is a Bevy `Resource`. `active_target` tracks the last `ForPartyMember` target; it defaults to `EvtTargetCharacter::Player1`.

- [ ] **Step 1: Write the file**

```rust
pub mod member;

use bevy::prelude::*;
use lod::enums::{EvtTargetCharacter, EvtVariable};

use member::{CharacterClass, PartyMember, SKILL_COUNT};

/// The active party: exactly 4 members (indices 0–3 match EvtTargetCharacter::Player1–4).
#[derive(Resource)]
pub struct Party {
    pub members: [PartyMember; 4],
    /// The character target set by the most recent ForPartyMember EVT opcode.
    pub active_target: EvtTargetCharacter,
}

impl Party {
    /// Resolve active_target to a slice of member indices.
    fn target_indices(&self, target: EvtTargetCharacter) -> Vec<usize> {
        match target {
            EvtTargetCharacter::Player1 => vec![0],
            EvtTargetCharacter::Player2 => vec![1],
            EvtTargetCharacter::Player3 => vec![2],
            EvtTargetCharacter::Player4 => vec![3],
            EvtTargetCharacter::Active  => vec![0], // first member as stand-in
            EvtTargetCharacter::Party   => vec![0, 1, 2, 3],
            EvtTargetCharacter::Random  => vec![0], // deterministic fallback
        }
    }

    /// Returns the highest skill level for `var` across all members matching `target`.
    /// Returns 0 if the variable is not a skill or no members match.
    pub fn max_skill(&self, target: EvtTargetCharacter, var: EvtVariable) -> u8 {
        self.target_indices(target)
            .iter()
            .map(|&i| self.members[i].get_skill(var))
            .max()
            .unwrap_or(0)
    }
}

impl Default for Party {
    fn default() -> Self {
        // Mock MM6 starting party
        let mut zoltan  = PartyMember::new("Zoltan",  CharacterClass::Knight,   1);
        let mut roderick = PartyMember::new("Roderick", CharacterClass::Paladin,  1);
        let mut alexei  = PartyMember::new("Alexei",  CharacterClass::Archer,   1);
        let mut serena  = PartyMember::new("Serena",  CharacterClass::Cleric,   1);

        // Give each member their starting skills (level 1)
        zoltan.set_skill(EvtVariable::SKILL_SWORD,        1);
        zoltan.set_skill(EvtVariable::SKILL_SHIELD,       1);
        zoltan.set_skill(EvtVariable::SKILL_LEATHER,      1);

        roderick.set_skill(EvtVariable::SKILL_SWORD,      1);
        roderick.set_skill(EvtVariable::SKILL_CHAIN,      1);
        roderick.set_skill(EvtVariable::SKILL_SPIRIT_MAGIC, 1);

        alexei.set_skill(EvtVariable::SKILL_BOW,          1);
        alexei.set_skill(EvtVariable::SKILL_LEATHER,      1);
        alexei.set_skill(EvtVariable::SKILL_AIR_MAGIC,    1);

        serena.set_skill(EvtVariable::SKILL_MACE,         1);
        serena.set_skill(EvtVariable::SKILL_CHAIN,        1);
        serena.set_skill(EvtVariable::SKILL_SPIRIT_MAGIC, 1);

        Self {
            members: [zoltan, roderick, alexei, serena],
            active_target: EvtTargetCharacter::Player1,
        }
    }
}

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Party>();
    }
}
```

- [ ] **Step 2: Register the module and plugin in `game/mod.rs`**

In `openmm/src/game/mod.rs`, add after the existing `pub(crate) mod world_state;` line:

```rust
pub(crate) mod party;
```

And in `InGamePlugin::build`, add `party::PartyPlugin` to the `add_plugins` call. Insert it after `world_state::WorldStatePlugin,`:

```rust
            world_state::WorldStatePlugin,
            party::PartyPlugin,
```

- [ ] **Step 3: Build**

```bash
cd /home/roarc/repos/openmm && cargo build -p openmm 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add openmm/src/game/party/member.rs openmm/src/game/party/mod.rs openmm/src/game/mod.rs
git commit --no-gpg-sign -m "feat: add Party resource with mock MM6 party (Knight/Paladin/Archer/Cleric)"
```

---

## Task 4: Add logging methods to `GameVariables`

**Files:**
- Modify: `openmm/src/game/world_state.rs`

Replace the bare `HashSet::insert/remove` calls in `event_dispatch.rs` with explicit, logged methods. The methods live on `GameVariables` so the logic and logging are co-located.

- [ ] **Step 1: Add methods to `GameVariables` in `world_state.rs`**

After the `impl Default for GameVariables` block (after line 96), add:

```rust
impl GameVariables {
    pub fn set_qbit(&mut self, bit: i32) {
        if self.quest_bits.insert(bit) {
            info!("[QBit {:4}] set", bit);
        }
    }

    pub fn clear_qbit(&mut self, bit: i32) {
        if self.quest_bits.remove(&bit) {
            info!("[QBit {:4}] cleared", bit);
        }
    }

    pub fn has_qbit(&self, bit: i32) -> bool {
        self.quest_bits.contains(&bit)
    }

    pub fn add_autonote(&mut self, note: i32) {
        if self.autonotes.insert(note) {
            info!("[Note {:4}] added", note);
        }
    }

    pub fn remove_autonote(&mut self, note: i32) {
        if self.autonotes.remove(&note) {
            info!("[Note {:4}] removed", note);
        }
    }

    pub fn has_autonote(&self, note: i32) -> bool {
        self.autonotes.contains(&note)
    }
}
```

- [ ] **Step 2: Build**

```bash
cd /home/roarc/repos/openmm && cargo build -p openmm 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add openmm/src/game/world_state.rs
git commit --no-gpg-sign -m "feat: add logging methods to GameVariables (set/clear/has for QBit and Autonote)"
```

---

## Task 5: Extend `GameSave` with `SavedProgress`

**Files:**
- Modify: `openmm/src/save.rs`

Add a `SavedProgress` struct and wire it into `GameSave`. Also update `WorldState::write_to_save` and `read_from_save` to persist quest bits, autonotes, gold, and food.

- [ ] **Step 1: Add `SavedProgress` to `save.rs`**

Add the struct and extend `GameSave`:

```rust
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SavedProgress {
    pub quest_bits: Vec<i32>,
    pub autonotes: Vec<i32>,
    pub gold: i32,
    pub food: i32,
}
```

In the `GameSave` struct, add the field after `player`:

```rust
pub struct GameSave {
    pub version: u32,
    pub map: MapState,
    pub player: PlayerState,
    pub progress: SavedProgress,
}
```

In `impl Default for GameSave`, add after `player: PlayerState { ... }`:

```rust
            progress: SavedProgress {
                quest_bits: Vec::new(),
                autonotes: Vec::new(),
                gold: 200,
                food: 7,
            },
```

- [ ] **Step 2: Update `WorldState::write_to_save` in `world_state.rs`**

In the `write_to_save` method (currently lines 112–121), add after `save.player.yaw = self.player.yaw;`:

```rust
        save.progress.quest_bits = self.game_vars.quest_bits.iter().copied().collect();
        save.progress.autonotes  = self.game_vars.autonotes.iter().copied().collect();
        save.progress.gold       = self.game_vars.gold;
        save.progress.food       = self.game_vars.food;
```

- [ ] **Step 3: Update `WorldState::read_from_save` in `world_state.rs`**

In `read_from_save` (currently lines 124–130), add after `self.map.map_y = save.map.map_y;`:

```rust
        self.game_vars.quest_bits = save.progress.quest_bits.iter().copied().collect();
        self.game_vars.autonotes  = save.progress.autonotes.iter().copied().collect();
        self.game_vars.gold       = save.progress.gold;
        self.game_vars.food       = save.progress.food;
```

- [ ] **Step 4: Build**

```bash
cd /home/roarc/repos/openmm && cargo build -p openmm 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/save.rs openmm/src/game/world_state.rs
git commit --no-gpg-sign -m "feat: persist quest progress (QBits, autonotes, gold, food) in GameSave"
```

---

## Task 6: Wire `ForPartyMember`, use logging methods, implement `CheckSkill`

**Files:**
- Modify: `openmm/src/game/event_dispatch.rs`

Three changes in one pass:
1. Add `Party` as a system parameter so `ForPartyMember` can set `active_target` and `CheckSkill` can read skills.
2. Replace bare `quest_bits.insert/remove` and `autonotes.insert/remove` calls with the new logging methods.
3. Implement `CheckSkill` using `party.max_skill()`.

- [ ] **Step 1: Add `Party` to `process_events` system parameters**

In `event_dispatch.rs`, add to the imports at the top:

```rust
use crate::game::party::Party;
```

In the `process_events` function signature, add after `mut world_state: ResMut<crate::game::world_state::WorldState>,`:

```rust
    mut party: ResMut<Party>,
```

- [ ] **Step 2: Update `add_variable` — replace bare HashSet calls with logging methods**

Find the `add_variable` function (~line 189). Replace the `QBITS` and `AUTONOTES_BITS` arms:

```rust
        EvtVariable::QBITS => {
            vars.set_qbit(value);
        }
        EvtVariable::AUTONOTES_BITS => {
            vars.add_autonote(value);
        }
```

- [ ] **Step 3: Update `subtract_variable`**

Find the `subtract_variable` function (~line 221). Replace the `QBITS` and `AUTONOTES_BITS` arms:

```rust
        EvtVariable::QBITS => {
            vars.clear_qbit(value);
        }
        EvtVariable::AUTONOTES_BITS => {
            vars.remove_autonote(value);
        }
```

- [ ] **Step 4: Update `set_variable`**

Find the `set_variable` function (~line 152). Replace the `QBITS` and `AUTONOTES_BITS` arms:

```rust
        EvtVariable::QBITS => {
            if value != 0 {
                vars.set_qbit(value);
            }
        }
        EvtVariable::AUTONOTES_BITS => {
            if value != 0 {
                vars.add_autonote(value);
            }
        }
```

- [ ] **Step 5: Update `evaluate_compare` — use logging methods**

Find `evaluate_compare` (~line 253). Replace the two `quest_bits.contains` / `autonotes.contains` calls:

```rust
    if var == EvtVariable::QBITS {
        let result = vars.has_qbit(value);
        debug!("  Compare: QBit {} present? -> {}", value, result);
        return result;
    }
    if var == EvtVariable::AUTONOTES_BITS {
        let result = vars.has_autonote(value);
        debug!("  Compare: Autonote {} present? -> {}", value, result);
        return result;
    }
```

- [ ] **Step 6: Wire `ForPartyMember`**

Find the `GameEvent::ForPartyMember { player }` arm (~line 440). Replace the entire arm:

```rust
            GameEvent::ForPartyMember { player } => {
                if let Some(target) = lod::enums::EvtTargetCharacter::from_u8(*player) {
                    info!("  ForPartyMember: target = {:?}", target);
                    party.active_target = target;
                } else {
                    warn!("  ForPartyMember: unknown player byte {}", player);
                }
            }
```

- [ ] **Step 7: Implement `CheckSkill`**

Find the `GameEvent::CheckSkill { skill_id, skill_level, jump_step }` arm (~line 613). Replace the entire arm:

```rust
            GameEvent::CheckSkill { skill_id, skill_level, jump_step } => {
                let var = EvtVariable(*skill_id);
                let best = party.max_skill(party.active_target, var);
                let pass = best >= *skill_level as u8;
                info!("  CheckSkill: {} level {} required, best={} target={:?} -> {}",
                    var, skill_level, best, party.active_target,
                    if pass { "pass" } else { "fail -> jump" });
                if !pass {
                    if let Some(target_idx) = steps.iter().position(|s| s.step >= *jump_step) {
                        pc = target_idx;
                    } else {
                        return;
                    }
                }
            }
```

- [ ] **Step 8: Build**

```bash
cd /home/roarc/repos/openmm && cargo build -p openmm 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 9: Commit**

```bash
git add openmm/src/game/event_dispatch.rs
git commit --no-gpg-sign -m "feat: wire ForPartyMember target, implement CheckSkill, use GameVariables logging methods"
```

---

## Task 7: Full build and smoke test

- [ ] **Step 1: Full build**

```bash
cd /home/roarc/repos/openmm && cargo build 2>&1 | grep -E "^error|warning\[unused"
```

Expected: no errors, no unused-import warnings.

- [ ] **Step 2: Run tests**

```bash
cd /home/roarc/repos/openmm && cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 3: Verify Party logs at startup**

```bash
RUST_LOG=openmm=info cargo run 2>&1 | grep -E "Party|QBit|Note|ForParty|CheckSkill" | head -20
```

Expected: no output at startup (Party is silent until EVT events fire). If you load a map with QBit-setting events, you should see `[QBit  NNN] set` lines.

- [ ] **Step 4: Final commit if any fixes needed**

If any lint warnings or minor fixes came out of the above, commit them:

```bash
git add -u
git commit --no-gpg-sign -m "fix: post-review cleanup after party/progress feature"
```
