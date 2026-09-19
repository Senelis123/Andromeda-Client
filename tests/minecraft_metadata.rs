use std::collections::HashMap;

use andromeda_client::minecraft::{
    OsRule, Rule, RuleAction, RuleContext, RuleDecision, VersionKind, VersionManifest,
    evaluate_rules,
};

#[test]
fn parses_pinned_global_manifest() {
    let manifest: VersionManifest =
        serde_json::from_str(include_str!("fixtures/version_manifest.json")).unwrap();
    assert_eq!(manifest.latest.release, "1.21.8");
    assert_eq!(manifest.versions.len(), 2);
    assert_eq!(manifest.versions[0].kind, VersionKind::Release);
}

#[test]
fn no_rules_means_allowed() {
    let features = HashMap::new();
    let context = RuleContext {
        os_name: "windows",
        architecture: "x86_64",
        os_version: "10.0",
        features: &features,
    };
    assert_eq!(evaluate_rules(&[], &context), RuleDecision::Allow);
}

#[test]
fn last_matching_rule_wins() {
    let features = HashMap::from([("is_demo_user".to_owned(), false)]);
    let rules = vec![
        Rule {
            action: RuleAction::Allow,
            os: None,
            features: HashMap::new(),
        },
        Rule {
            action: RuleAction::Disallow,
            os: Some(OsRule {
                name: Some("windows".to_owned()),
                arch: None,
                version: None,
            }),
            features: HashMap::new(),
        },
    ];
    let context = RuleContext {
        os_name: "windows",
        architecture: "x86_64",
        os_version: "10.0",
        features: &features,
    };
    assert_eq!(evaluate_rules(&rules, &context), RuleDecision::Disallow);
}

#[test]
fn required_features_must_match() {
    let features = HashMap::from([("has_custom_resolution".to_owned(), false)]);
    let rules = vec![Rule {
        action: RuleAction::Allow,
        os: None,
        features: HashMap::from([("has_custom_resolution".to_owned(), true)]),
    }];
    let context = RuleContext {
        os_name: "linux",
        architecture: "x86_64",
        os_version: "6.0",
        features: &features,
    };
    assert_eq!(evaluate_rules(&rules, &context), RuleDecision::Disallow);
}
