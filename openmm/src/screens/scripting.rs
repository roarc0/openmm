//! Screen scripting: flat action executor with Compare/Else/End control flow.
//! Screen-native actions are handled locally; `evt:` prefixed actions proxy
//! to the EVT EventQueue.

use crate::game::world::state::GameVariables;

/// Parsed screen action.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // Screen-native
    LoadScreen(String),
    ShowScreen(String),
    HideScreen(String),
    ShowSprite(String),
    HideSprite(String),
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

    // evt: prefix -> proxy to EVT system
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
    Action::Unknown(s.to_string())
}

/// Extract string arg from `FuncName("value")`.
pub(crate) fn parse_string_arg<'a>(input: &'a str, func_name: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(func_name)?.trim();
    let rest = rest.strip_prefix('(')?.strip_suffix(')')?;
    let rest = rest.trim();
    rest.strip_prefix('"')?.strip_suffix('"')
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // -- Task 1: parse_action tests --

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
        // Bare Hint is unknown — use evt:Hint instead
        assert!(matches!(parse_action("Hint(\"Cast Spell\")"), Action::Unknown(_)));
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

    // -- Task 2: eval_condition tests --

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
        assert!(!eval_condition("", &vars));
        assert!(!eval_condition("nonsense", &vars));
    }

    // -- Task 3: execute_actions tests --

    #[test]
    fn execute_unconditional() {
        let actions = vec!["evt:Hint(\"hello\")".into(), "evt:Hint(\"world\")".into()];
        let vars = GameVariables::default();
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Action::EvtProxy("Hint(\"hello\")".into()));
        assert_eq!(result[1], Action::EvtProxy("Hint(\"world\")".into()));
    }

    #[test]
    fn execute_compare_true() {
        let mut vars = GameVariables::default();
        vars.set_qbit(12);
        let actions = vec![
            "Compare(\"quest_bit(12)\")".into(),
            "evt:Hint(\"yes\")".into(),
            "End()".into(),
            "evt:Hint(\"always\")".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Action::EvtProxy("Hint(\"yes\")".into()));
        assert_eq!(result[1], Action::EvtProxy("Hint(\"always\")".into()));
    }

    #[test]
    fn execute_compare_false() {
        let vars = GameVariables::default();
        let actions = vec![
            "Compare(\"quest_bit(12)\")".into(),
            "evt:Hint(\"yes\")".into(),
            "End()".into(),
            "evt:Hint(\"always\")".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Action::EvtProxy("Hint(\"always\")".into()));
    }

    #[test]
    fn execute_compare_else() {
        let mut vars = GameVariables::default();
        vars.set_qbit(5);
        let actions = vec![
            "Compare(\"quest_bit(5)\")".into(),
            "evt:Hint(\"yes\")".into(),
            "Else()".into(),
            "evt:Hint(\"no\")".into(),
            "End()".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Action::EvtProxy("Hint(\"yes\")".into()));

        let vars2 = GameVariables::default();
        let result2 = execute_actions(&actions, &vars2);
        assert_eq!(result2.len(), 1);
        assert_eq!(result2[0], Action::EvtProxy("Hint(\"no\")".into()));
    }

    #[test]
    fn execute_evt_proxy_passes_through() {
        let vars = GameVariables::default();
        let actions = vec!["evt:PlaySound(75)".into()];
        let result = execute_actions(&actions, &vars);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], Action::EvtProxy("PlaySound(75)".into()));
    }

    // -- Task 5: EVT proxy tests --

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
        let vars = GameVariables::default();
        let actions = vec![
            "Compare(\"quest_bit(99)\")".into(),
            "evt:PlaySound(75)".into(),
            "End()".into(),
        ];
        let result = execute_actions(&actions, &vars);
        assert!(result.is_empty());
    }
}
