# Screen Scripting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a flat action executor (`screens/scripting.rs`) that handles screen-native actions, proxies `evt:`-prefixed actions to the EVT `EventQueue`, and supports `Compare`/`Else`/`End` control flow for conditional logic in RON files.

**Architecture:** New `screens/scripting.rs` module with pure action parsing, condition evaluation, and a `ScriptState` executor. `runtime.rs` delegates its `dispatch_action` logic to the new module. EVT proxy pushes `GameEvent` variants onto the existing `EventQueue`.

**Tech Stack:** Rust, Bevy 0.18, existing `openmm_data::evt::GameEvent` enum, existing `GameVariables` resource.

---

### Task 1: Create `screens/scripting.rs` with action parsing

**Files:**
- Create: `openmm/src/screens/scripting.rs`
- Modify: `openmm/src/screens/mod.rs`

- [ ] **Step 1: Write failing tests for `parse_action`**

In `openmm/src/screens/scripting.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_screen_actions() {
        assert_eq!(parse_action("Quit()"), Action::Quit);
        assert_eq!(parse_action("NewGame()"), Action::NewGame);
        assert_eq!(
            parse_action("LoadScreen(\"menu\")"),
            Action::LoadScreen("menu".into())
        );
        assert_eq!(
            parse_action("ShowScreen(\"hud\")"),
            Action::ShowScreen("hud".into())
        );
        assert_eq!(
            parse_action("HideScreen(\"hud\")"),
            Action::HideScreen("hud".into())
        );
        assert_eq!(
            parse_action("ShowSprite(\"icon\")"),
            Action::ShowSprite("icon".into())
        );
        assert_eq!(
            parse_action("HideSprite(\"icon\")"),
            Action::HideSprite("icon".into())
        );
        assert_eq!(
            parse_action("Hint(\"Cast Spell\")"),
            Action::Hint("Cast Spell".into())
        );
        assert_eq!(parse_action("PulseSprite()"), Action::PulseSprite);
    }

    #[test]
    fn parse_evt_proxy() {
        assert_eq!(
            parse_action("evt:PlaySound(75)"),
            Action::EvtProxy("PlaySound(75)".into())
        );
        assert_eq!(
            parse_action("evt:Hint(\"hello\")"),
            Action::EvtProxy("Hint(\"hello\")".into())
        );
    }

    #[test]
    fn parse_control_flow() {
        assert_eq!(
            parse_action("Compare(\"quest_bit(12)\")"),
            Action::Compare("quest_bit(12)".into())
        );
        assert_eq!(parse_action("Else()"), Action::Else);
        assert_eq!(parse_action("End()"), Action::End);
    }

    #[test]
    fn parse_unknown() {
        assert!(matches!(parse_action("Bogus()"), Action::Unknown(_)));
        assert!(matches!(parse_action(""), Action::Unknown(_)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p openmm --lib screens::scripting -- -v 2>&1 | tail -20`
Expected: FAIL — module doesn't exist yet.

- [ ] **Step 3: Write `Action` enum and `parse_action`**

```rust
//! Screen scripting: flat action executor with Compare/Else/End control flow.
//! Screen-native actions are handled locally; `evt:` prefixed actions proxy
//! to the EVT EventQueue.

/// Parsed screen action.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Screen-native
    LoadScreen(String),
    ShowScreen(String),
    HideScreen(String),
    ShowSprite(String),
    HideSprite(String),
    Hint(String),
    PulseSprite,
    NewGame,
    Quit,
    // EVT proxy — raw action string after stripping "evt:" prefix
    EvtProxy(String),
    // Control flow
    Compare(String),
    Else,
    End,
    // Fallback
    Unknown(String),
}

/// Parse an action string from RON into an Action enum.
pub fn parse_action(input: &str) -> Action {
    let s = input.trim();
    if s.is_empty() {
        return Action::Unknown(s.into());
    }

    // evt: prefix → proxy to EVT system
    if let Some(rest) = s.strip_prefix("evt:") {
        return Action::EvtProxy(rest.to_string());
    }

    // Control flow
    if s == "Else()" {
        return Action::Else;
    }
    if s == "End()" {
        return Action::End;
    }
    if let Some(expr) = parse_string_arg(s, "Compare") {
        return Action::Compare(expr.to_string());
    }

    // Screen-native actions
    if s == "Quit()" {
        return Action::Quit;
    }
    if s == "NewGame()" {
        return Action::NewGame;
    }
    if s == "PulseSprite()" {
        return Action::PulseSprite;
    }
    if let Some(id) = parse_string_arg(s, "LoadScreen") {
        return Action::LoadScreen(id.to_string());
    }
    if let Some(id) = parse_string_arg(s, "ShowScreen") {
        return Action::ShowScreen(id.to_string());
    }
    if let Some(id) = parse_string_arg(s, "HideScreen") {
        return Action::HideScreen(id.to_string());
    }
    if let Some(id) = parse_string_arg(s, "ShowSprite") {
        return Action::ShowSprite(id.to_string());
    }
    if let Some(id) = parse_string_arg(s, "HideSprite") {
        return Action::HideSprite(id.to_string());
    }
    if let Some(text) = parse_string_arg(s, "Hint") {
        return Action::Hint(text.to_string());
    }

    Action::Unknown(s.to_string())
}

/// Extract string arg from `FuncName("value")`.
fn parse_string_arg<'a>(input: &'a str, func_name: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(func_name)?.trim();
    let rest = rest.strip_prefix('(')?.strip_suffix(')')?;
    let rest = rest.trim();
    rest.strip_prefix('"')?.strip_suffix('"')
}
```

- [ ] **Step 4: Register module in `mod.rs`**

In `openmm/src/screens/mod.rs`, add:

```rust
pub(crate) mod scripting;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p openmm --lib screens::scripting -- -v 2>&1 | tail -20`
Expected: all 4 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add openmm/src/screens/scripting.rs openmm/src/screens/mod.rs
git commit --no-gpg-sign -m "screens: add scripting module with action parser"
```

---

### Task 2: Add condition evaluator

**Files:**
- Modify: `openmm/src/screens/scripting.rs`

- [ ] **Step 1: Write failing tests for `eval_condition`**

Add to the `tests` module in `scripting.rs`:

```rust
    #[test]
    fn eval_quest_bit() {
        let mut vars = GameVariables::default();
        assert!(!eval_condition("quest_bit(12)", &vars));
        vars.set_qbit(12);
        assert!(eval_condition("quest_bit(12)", &vars));
    }

    #[test]
    fn eval_not_quest_bit() {
        let mut vars = GameVariables::default();
        assert!(eval_condition("not quest_bit(12)", &vars));
        vars.set_qbit(12);
        assert!(!eval_condition("not quest_bit(12)", &vars));
    }

    #[test]
    fn eval_map_var_eq() {
        let mut vars = GameVariables::default();
        vars.map_vars[3] = 5;
        assert!(eval_condition("map_var(3) == 5", &vars));
        assert!(!eval_condition("map_var(3) == 4", &vars));
        assert!(eval_condition("map_var(3) != 4", &vars));
        assert!(!eval_condition("map_var(3) != 5", &vars));
    }

    #[test]
    fn eval_gold_comparisons() {
        let mut vars = GameVariables::default();
        vars.gold = 100;
        assert!(eval_condition("gold > 50", &vars));
        assert!(!eval_condition("gold > 100", &vars));
        assert!(eval_condition("gold >= 100", &vars));
        assert!(eval_condition("gold < 200", &vars));
        assert!(!eval_condition("gold < 100", &vars));
        assert!(eval_condition("gold <= 100", &vars));
    }

    #[test]
    fn eval_food_comparisons() {
        let mut vars = GameVariables::default();
        vars.food = 7;
        assert!(eval_condition("food > 5", &vars));
        assert!(eval_condition("food < 10", &vars));
    }

    #[test]
    fn eval_invalid_condition() {
        let vars = GameVariables::default();
        // Invalid expressions return false
        assert!(!eval_condition("", &vars));
        assert!(!eval_condition("nonsense", &vars));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p openmm --lib screens::scripting -- -v 2>&1 | tail -20`
Expected: FAIL — `eval_condition` not found.

- [ ] **Step 3: Implement `eval_condition`**

Add to `scripting.rs`, above the tests module:

```rust
use crate::game::world::state::GameVariables;

/// Evaluate a condition expression against game variables.
/// Returns false for invalid expressions (fail-safe: skip the block).
pub fn eval_condition(expr: &str, vars: &GameVariables) -> bool {
    let s = expr.trim();
    if s.is_empty() {
        return false;
    }

    // Negation
    if let Some(inner) = s.strip_prefix("not ") {
        return !eval_condition(inner, vars);
    }

    // quest_bit(N)
    if let Some(inner) = s.strip_prefix("quest_bit(").and_then(|r| r.strip_suffix(')')) {
        if let Ok(n) = inner.trim().parse::<i32>() {
            return vars.has_qbit(n);
        }
        return false;
    }

    // map_var(N) op X
    if let Some(rest) = s.strip_prefix("map_var(") {
        if let Some((idx_str, after_paren)) = rest.split_once(')') {
            if let Ok(idx) = idx_str.trim().parse::<usize>() {
                let val = vars.map_vars.get(idx).copied().unwrap_or(0);
                return eval_comparison(after_paren.trim(), val);
            }
        }
        return false;
    }

    // gold op X
    if let Some(rest) = s.strip_prefix("gold") {
        return eval_comparison(rest.trim(), vars.gold);
    }

    // food op X
    if let Some(rest) = s.strip_prefix("food") {
        return eval_comparison(rest.trim(), vars.food);
    }

    bevy::log::warn!("unknown condition expression: '{}'", s);
    false
}

/// Evaluate "op value" against a current value. e.g. "> 50", "== 3", "!= 0".
fn eval_comparison(op_and_value: &str, current: i32) -> bool {
    let s = op_and_value.trim();
    if let Some(v) = s.strip_prefix(">=") {
        return v.trim().parse::<i32>().is_ok_and(|n| current >= n);
    }
    if let Some(v) = s.strip_prefix("<=") {
        return v.trim().parse::<i32>().is_ok_and(|n| current <= n);
    }
    if let Some(v) = s.strip_prefix("!=") {
        return v.trim().parse::<i32>().is_ok_and(|n| current != n);
    }
    if let Some(v) = s.strip_prefix("==") {
        return v.trim().parse::<i32>().is_ok_and(|n| current == n);
    }
    if let Some(v) = s.strip_prefix('>') {
        return v.trim().parse::<i32>().is_ok_and(|n| current > n);
    }
    if let Some(v) = s.strip_prefix('<') {
        return v.trim().parse::<i32>().is_ok_and(|n| current < n);
    }
    false
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p openmm --lib screens::scripting -- -v 2>&1 | tail -20`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/screens/scripting.rs
git commit --no-gpg-sign -m "screens: add condition evaluator for scripting"
```

---

### Task 3: Add ScriptState executor with Compare/Else/End

**Files:**
- Modify: `openmm/src/screens/scripting.rs`

- [ ] **Step 1: Write failing tests for `execute_actions`**

Add to the `tests` module:

```rust
    #[test]
    fn execute_unconditional() {
        let actions = vec!["Hint(\"hello\")".into(), "Hint(\"world\")".into()];
        let vars = GameVariables::default();
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Action::Hint("hello".into()));
        assert_eq!(result[1], Action::Hint("world".into()));
    }

    #[test]
    fn execute_compare_true() {
        let mut vars = GameVariables::default();
        vars.set_qbit(12);
        let actions = vec![
            "Compare(\"quest_bit(12)\")".into(),
            "Hint(\"yes\")".into(),
            "End()".into(),
            "Hint(\"always\")".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Action::Hint("yes".into()));
        assert_eq!(result[1], Action::Hint("always".into()));
    }

    #[test]
    fn execute_compare_false() {
        let vars = GameVariables::default();
        let actions = vec![
            "Compare(\"quest_bit(12)\")".into(),
            "Hint(\"yes\")".into(),
            "End()".into(),
            "Hint(\"always\")".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Action::Hint("always".into()));
    }

    #[test]
    fn execute_compare_else() {
        let mut vars = GameVariables::default();
        vars.set_qbit(5);
        let actions = vec![
            "Compare(\"quest_bit(5)\")".into(),
            "Hint(\"yes\")".into(),
            "Else()".into(),
            "Hint(\"no\")".into(),
            "End()".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Action::Hint("yes".into()));

        // Now test the else branch
        let vars2 = GameVariables::default();
        let result2 = execute_actions(&actions, &vars2);
        assert_eq!(result2.len(), 1);
        assert_eq!(result2[0], Action::Hint("no".into()));
    }

    #[test]
    fn execute_evt_proxy_passes_through() {
        let vars = GameVariables::default();
        let actions = vec!["evt:PlaySound(75)".into()];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Action::EvtProxy("PlaySound(75)".into()));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p openmm --lib screens::scripting -- -v 2>&1 | tail -20`
Expected: FAIL — `execute_actions` not found.

- [ ] **Step 3: Implement `execute_actions`**

Add to `scripting.rs`:

```rust
/// Walk an action string list with Compare/Else/End control flow.
/// Returns the list of actions that should be dispatched (control flow
/// keywords consumed, skipped actions excluded).
pub fn execute_actions(action_strings: &[String], vars: &GameVariables) -> Vec<Action> {
    let mut result = Vec::new();
    let mut condition_met = true;
    let mut in_block = false;

    for s in action_strings {
        let action = parse_action(s);
        match action {
            Action::Compare(ref expr) => {
                in_block = true;
                condition_met = eval_condition(expr, vars);
            }
            Action::Else => {
                condition_met = !condition_met;
            }
            Action::End => {
                in_block = false;
                condition_met = true;
            }
            _ => {
                if !in_block || condition_met {
                    result.push(action);
                }
            }
        }
    }

    result
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p openmm --lib screens::scripting -- -v 2>&1 | tail -20`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add openmm/src/screens/scripting.rs
git commit --no-gpg-sign -m "screens: add ScriptState executor with Compare/Else/End"
```

---

### Task 4: Wire executor into `runtime.rs`

**Files:**
- Modify: `openmm/src/screens/runtime.rs`

This task replaces the inline `dispatch_action` function with the scripting module. The executor walks the action list, evaluates conditions, and dispatches each surviving action.

- [ ] **Step 1: Replace `process_pending_actions` to use `execute_actions`**

In `runtime.rs`, replace the `process_pending_actions` function (lines ~1039-1069) with:

```rust
/// Process queued actions via the scripting executor.
fn process_pending_actions(
    mut commands: Commands,
    pending: Option<Res<PendingActions>>,
    mut layers: ResMut<ScreenLayers>,
    layer_entities: Query<(Entity, &ScreenLayer)>,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut exit_writer: bevy::ecs::message::MessageWriter<bevy::app::AppExit>,
    world_state: Option<Res<crate::game::world::WorldState>>,
    mut event_queue: Option<ResMut<crate::game::world::scripting::EventQueue>>,
    footer: Option<ResMut<crate::game::hud::FooterText>>,
    time: Res<Time>,
) {
    let Some(pending) = pending else { return };
    let action_strings = pending.actions.clone();
    commands.remove_resource::<PendingActions>();

    // Use GameVariables for condition evaluation if available (Game state),
    // otherwise use default (Menu state — no conditions available).
    let default_vars = crate::game::world::state::GameVariables::default();
    let vars = world_state.as_ref().map(|ws| &ws.game_vars).unwrap_or(&default_vars);

    let actions = super::scripting::execute_actions(&action_strings, vars);

    for action in &actions {
        match action {
            super::scripting::Action::Quit => {
                info!("action: Quit()");
                exit_writer.write(bevy::app::AppExit::Success);
            }
            super::scripting::Action::NewGame => {
                info!("action: NewGame()");
                commands.set_state(GameState::Loading);
            }
            super::scripting::Action::LoadScreen(id) => {
                info!("action: LoadScreen(\"{}\")", id);
                load_screen_replace_all(
                    id, &mut commands, &mut layers, &layer_entities,
                    &mut ui_assets, &game_assets, &mut images,
                    &mut audio_sources, &cfg,
                );
            }
            super::scripting::Action::ShowScreen(id) => {
                info!("action: ShowScreen(\"{}\")", id);
                show_screen(
                    id, &mut commands, &mut layers, &mut ui_assets,
                    &game_assets, &mut images, &mut audio_sources, &cfg,
                );
            }
            super::scripting::Action::HideScreen(id) => {
                info!("action: HideScreen(\"{}\")", id);
                hide_screen(id, &mut commands, &mut layers, &layer_entities);
            }
            super::scripting::Action::ShowSprite(_id) => {
                // TODO: find entity by element id, set Visibility::Inherited
                info!("action: ShowSprite (not yet wired)");
            }
            super::scripting::Action::HideSprite(_id) => {
                // TODO: find entity by element id, set Visibility::Hidden
                info!("action: HideSprite (not yet wired)");
            }
            super::scripting::Action::Hint(text) => {
                info!("action: Hint(\"{}\")", text);
                if let Some(ref mut ft) = footer {
                    ft.set(text);
                }
            }
            super::scripting::Action::PulseSprite => {
                // Handled at spawn time via Pulsable component, not at dispatch.
            }
            super::scripting::Action::EvtProxy(evt_str) => {
                info!("action: evt:{}", evt_str);
                if let Some(ref mut eq) = event_queue {
                    proxy_evt_action(evt_str, eq);
                } else {
                    warn!("evt: proxy '{}' ignored — no EventQueue (not in Game state)", evt_str);
                }
            }
            super::scripting::Action::Unknown(s) => {
                warn!("unknown screen action: '{}'", s);
            }
            // Control flow variants already consumed by execute_actions
            super::scripting::Action::Compare(_)
            | super::scripting::Action::Else
            | super::scripting::Action::End => {}
        }
    }
}
```

- [ ] **Step 2: Add `proxy_evt_action` function**

Add after the new `process_pending_actions`:

```rust
/// Parse an EVT action string and push it onto the EventQueue.
/// Only supports a subset of GameEvent variants that make sense from UI context.
fn proxy_evt_action(evt_str: &str, event_queue: &mut crate::game::world::scripting::EventQueue) {
    use openmm_data::evt::GameEvent;

    let s = evt_str.trim();

    // PlaySound(id)
    if let Some(rest) = s.strip_prefix("PlaySound(").and_then(|r| r.strip_suffix(')')) {
        if let Ok(id) = rest.trim().parse::<u32>() {
            event_queue.push_single(GameEvent::PlaySound { sound_id: id });
            return;
        }
    }

    // Hint("text")
    if let Some(text) = parse_string_arg(s, "Hint") {
        event_queue.push_single(GameEvent::Hint {
            str_id: 0,
            text: text.to_string(),
        });
        return;
    }

    // StatusText("text")
    if let Some(text) = parse_string_arg(s, "StatusText") {
        event_queue.push_single(GameEvent::StatusText {
            str_id: 0,
            text: text.to_string(),
        });
        return;
    }

    warn!("evt: unknown proxy action: '{}'", s);
}
```

- [ ] **Step 3: Remove the old `dispatch_action` function and its `parse_string_arg`**

Delete the old `dispatch_action` function (lines ~1073-1128) and the old `parse_string_arg` helper (lines ~1131-1136) from `runtime.rs`. The `parse_string_arg` in `scripting.rs` handles all parsing now, but `runtime.rs` still needs it for `proxy_evt_action` — so keep a local copy in `runtime.rs` or make the one in `scripting.rs` `pub(crate)`. Simplest: make `scripting::parse_string_arg` `pub(crate)` and import it:

In `scripting.rs`, change:
```rust
fn parse_string_arg<'a>(input: &'a str, func_name: &str) -> Option<&'a str> {
```
to:
```rust
pub(crate) fn parse_string_arg<'a>(input: &'a str, func_name: &str) -> Option<&'a str> {
```

In `runtime.rs`, add at the top of the file's use section:
```rust
use super::scripting::parse_string_arg;
```

And delete the local `parse_string_arg` function from `runtime.rs`.

- [ ] **Step 4: Build and verify**

Run: `make build 2>&1 | tail -10`
Expected: compiles with only pre-existing warnings. No new errors.

- [ ] **Step 5: Run all screen tests**

Run: `cargo test -p openmm --lib screens -- -v 2>&1 | tail -20`
Expected: all tests PASS (format + scripting tests).

- [ ] **Step 6: Commit**

```bash
git add openmm/src/screens/runtime.rs openmm/src/screens/scripting.rs
git commit --no-gpg-sign -m "screens: wire scripting executor into runtime dispatch"
```

---

### Task 5: Add EVT proxy tests

**Files:**
- Modify: `openmm/src/screens/scripting.rs`

- [ ] **Step 1: Write tests for parsing EVT proxy action strings**

Add to the `tests` module in `scripting.rs`:

```rust
    #[test]
    fn parse_evt_play_sound() {
        assert_eq!(
            parse_action("evt:PlaySound(75)"),
            Action::EvtProxy("PlaySound(75)".into())
        );
    }

    #[test]
    fn parse_evt_status_text() {
        assert_eq!(
            parse_action("evt:StatusText(\"You found it!\")"),
            Action::EvtProxy("StatusText(\"You found it!\")".into())
        );
    }

    #[test]
    fn parse_evt_hint() {
        assert_eq!(
            parse_action("evt:Hint(\"Cast Spell\")"),
            Action::EvtProxy("Hint(\"Cast Spell\")".into())
        );
    }

    #[test]
    fn compare_blocks_evt_proxy() {
        // evt: actions inside a false Compare block should be skipped
        let vars = GameVariables::default();
        let actions = vec![
            "Compare(\"quest_bit(99)\")".into(),
            "evt:PlaySound(75)".into(),
            "End()".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert!(result.is_empty());
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p openmm --lib screens::scripting -- -v 2>&1 | tail -20`
Expected: all tests PASS (these exercise existing code paths).

- [ ] **Step 3: Commit**

```bash
git add openmm/src/screens/scripting.rs
git commit --no-gpg-sign -m "screens: add EVT proxy and integration tests"
```

---

### Task 6: Update docs

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add screen scripting docs to CLAUDE.md**

Add a new section after the "Event dispatch" section in CLAUDE.md:

```markdown
### Screen scripting

- `screens/scripting.rs` — flat action executor for screen RON files
- Action strings in `on_click`, `on_hover`, `on_end`, `keys` are one of:
  - Screen action (bare): `LoadScreen("menu")`, `ShowSprite("icon")`, `Hint("text")`, `Quit()`, `NewGame()`, `PulseSprite()`
  - EVT proxy (`evt:` prefix): `evt:PlaySound(75)`, `evt:Hint("text")`, `evt:StatusText("text")`
  - Control flow: `Compare("condition")`, `Else()`, `End()`
- Compare/Else/End: flat, single-level. Compare sets a flag; actions skip if flag is false; Else flips; End resets.
- Condition expressions: `quest_bit(N)`, `not quest_bit(N)`, `map_var(N) == X`, `gold > X`, `food < X`
- EVT proxy pushes `GameEvent` to `EventQueue` — same sink as the original EVT system, no interference
- `execute_actions()` returns dispatachable actions after control flow evaluation; `runtime.rs` handles the actual side effects
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit --no-gpg-sign -m "docs: add screen scripting section to CLAUDE.md"
```
