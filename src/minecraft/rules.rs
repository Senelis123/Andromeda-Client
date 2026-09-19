use std::collections::HashMap;

use super::{Rule, RuleAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleDecision { Allow, Disallow }

#[derive(Debug)]
pub struct RuleContext<'a> {
    pub os_name: &'a str,
    pub architecture: &'a str,
    pub os_version: &'a str,
    pub features: &'a HashMap<String, bool>,
}

/// Evaluates Mojang rules in order. The last matching rule wins.
#[must_use]
pub fn evaluate_rules(rules: &[Rule], context: &RuleContext<'_>) -> RuleDecision {
    if rules.is_empty() { return RuleDecision::Allow; }
    let mut decision = RuleDecision::Disallow;
    for rule in rules {
        if matches(rule, context) {
            decision = match rule.action { RuleAction::Allow => RuleDecision::Allow, RuleAction::Disallow => RuleDecision::Disallow };
        }
    }
    decision
}

fn matches(rule: &Rule, context: &RuleContext<'_>) -> bool {
    let os_matches = rule.os.as_ref().is_none_or(|os| {
        os.name.as_ref().is_none_or(|name| name == context.os_name)
            && os.arch.as_ref().is_none_or(|arch| architecture_matches(arch, context.architecture))
            && os.version.as_ref().is_none_or(|pattern| context.os_version.contains(pattern))
    });
    os_matches && rule.features.iter().all(|(name, expected)| context.features.get(name).copied().unwrap_or(false) == *expected)
}

fn architecture_matches(rule: &str, actual: &str) -> bool {
    rule == actual || (rule == "x86" && matches!(actual, "x86" | "i386" | "i686"))
}
