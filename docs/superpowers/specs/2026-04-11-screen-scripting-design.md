# Screen Scripting System

Adds a flat action executor to the screen RON system. Screen actions are data-driven (defined in RON), support control flow (Compare/Else/End), and can proxy into the original EVT event system via an `evt:` prefix.

## Motivation

Screen button handlers are currently hardcoded dispatch in `runtime.rs`. This makes it impossible to add conditional logic or reuse EVT opcodes from RON without code changes. The original game hardcoded its UI logic in C++; we want ours data-driven.

The EVT scripting system (`game/world/scripting.rs`) must stay untouched — it faithfully reproduces MM6's binary map/quest scripting. Screen scripting is a parallel system that shares the `EventQueue` as a sink but never interferes with EVT execution.

## Action String Format

Every string in `on_click`, `on_hover`, `on_end`, and `keys` is one of three types:

| Prefix | Type | Example |
|--------|------|---------|
| *(none)* | Screen action | `"LoadScreen(\"menu\")"` |
| `evt:` | EVT proxy | `"evt:PlaySound(75)"` |
| *(control flow)* | Compare/Else/End | `"Compare(\"quest_bit(12)\")"` |

The `evt:` prefix makes it explicit when reaching into the original game's scripting. Screen actions are the native language of RON files — no prefix needed.

## Screen Actions

Handled directly by the screen executor. Initial set:

- `LoadScreen("id")` — replace current screen
- `ShowScreen("id")` — add screen layer on top
- `HideScreen("id")` — remove screen layer
- `ShowSprite("id")` — unhide an element
- `HideSprite("id")` — hide an element
- `SetState("element_id", "state_name")` — change element texture state
- `PulseSprite()` — alpha fade animation (hover effect)
- `NewGame()` — transition to Loading state
- `Quit()` — exit application
- `Hint("text")` — show hint text (may also proxy to EVT)

This list grows as we add UI features. Each action is a variant in a `ScreenAction` enum.

## EVT Proxy

Strings starting with `evt:` are proxied to the original event system:

1. Strip `evt:` prefix
2. Parse remainder into a `GameEvent` variant
3. Push to `EventQueue`

The screen executor doesn't interpret EVT events — it's a passthrough. The existing `event_dispatch` system processes them on the next frame as usual.

Example: `"evt:PlaySound(75)"` becomes `GameEvent::PlaySound { id: 75 }` pushed to the queue.

Only a subset of `GameEvent` variants make sense from UI context (PlaySound, Hint, StatusText, PlayVideo, etc.). Unknown or invalid EVT action names log a warning and are skipped.

## Control Flow

Flat, single-level, inspired by EVT's Compare/Jmp pattern.

### Keywords

- `Compare("condition")` — evaluate condition, enter block
- `Else()` — flip condition result
- `End()` — exit block, resume unconditional execution

### Execution Model

```
struct ScriptState {
    condition_met: bool,
    in_block: bool,
}
```

Walk the action list sequentially:

1. `Compare(expr)` — evaluate `expr`, set `condition_met`, set `in_block = true`
2. If `in_block && !condition_met` — skip the action
3. `Else()` — flip `condition_met`
4. `End()` — reset `in_block = false`, `condition_met = true`
5. Any other action — dispatch (screen action or EVT proxy)

Actions after the last `End()` (or with no Compare at all) run unconditionally.

### Example

```ron
on_click: [
    "Compare(\"quest_bit(12)\")",
    "ShowSprite(\"reward_icon\")",
    "evt:PlaySound(75)",
    "Else()",
    "Hint(\"Complete the quest first\")",
    "End()",
    "evt:StatusText(\"You clicked the button\")",
]
```

If `quest_bit(12)` is set: show sprite, play sound, show status text.
If not: show hint, show status text.

No nesting. One level only. If you need an "else if", use a second Compare/End block after the first.

## Condition Evaluator

Pure function: `fn eval_condition(expr: &str, vars: &GameVariables) -> bool`

### Supported Expressions

- `quest_bit(N)` — true if quest bit N is set
- `not quest_bit(N)` — negation
- `map_var(N) == X` — map variable equality
- `map_var(N) != X` — inequality
- `gold > X` / `gold < X` / `gold >= X` / `gold <= X` — gold comparisons
- `food > X` / `food < X` / `food >= X` / `food <= X` — food comparisons

Expressions are simple — no compound `and`/`or`. For complex conditions, use sequential Compare/End blocks.

## File Structure

New file: `openmm/src/screens/scripting.rs`

Contents:
- `ScreenAction` enum — parsed screen actions
- `ScriptState` — executor state (condition flag)
- `execute_actions()` — walks action list, dispatches each
- `parse_action()` — string to ScreenAction or EVT proxy
- `eval_condition()` — condition string to bool
- `proxy_evt()` — parse EVT action string into GameEvent, push to EventQueue

## Integration

`runtime.rs` changes:
- `process_pending_actions` delegates to `scripting::execute_actions()` instead of inline matching
- The `PendingActions` resource stays — it queues action string lists from clicks/keys/video end
- `execute_actions` takes references to `EventQueue`, `GameVariables`, and screen state

No changes to:
- `game/world/scripting.rs` (EVT system)
- `game/event_dispatch.rs` (EVT processing)
- `screens/format.rs` (RON schema — action lists are already `Vec<String>`)

## Data Flow

```
RON on_click: ["Compare(...)", "ShowSprite(...)", "evt:PlaySound(75)", "End()"]
        |
        v
  PendingActions (queued by click/key/video handler)
        |
        v
  screens/scripting.rs  execute_actions()
        |
        +---> Screen action? ---> handle locally (show/hide sprite, load screen, etc.)
        |
        +---> evt: prefix? ----> strip prefix, parse GameEvent, push to EventQueue
        |
        +---> Compare/Else/End? -> update ScriptState flags
        |
        v
  EventQueue (shared with EVT system)
        |
        v
  game/event_dispatch.rs  process_events()  (unchanged)
```

## Testing

- `eval_condition` unit tests: each expression type against mock GameVariables
- `execute_actions` unit tests: verify skip/run logic with Compare/Else/End sequences
- `parse_action` unit tests: screen actions, evt proxy, control flow keywords, invalid input
- Integration: RON file with conditional actions, verify correct dispatch
