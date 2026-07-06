//! Precedence matrix + interpolation behavior for the variable system.

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use tomo_core::model::{EnvMeta, EnvironmentFile, SecretsFile, VarValue};
use tomo_core::vars::{
    SECRET_MASK, Scope, StackInputs, VarStack, Warning, interpolate, interpolate_masked,
    mask_secrets,
};

fn vars(pairs: &[(&str, serde_json::Value)]) -> IndexMap<String, VarValue> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn svars(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn precedence_matrix_all_six_layers() {
    let collection = vars(&[("who", serde_json::json!("collection"))]);
    let env_file = EnvironmentFile {
        meta: EnvMeta {
            name: "dev".into(),
            secrets: vec![],
        },
        vars: vars(&[("who", serde_json::json!("environment"))]),
    };
    let folder_outer = vars(&[("who", serde_json::json!("folder-outer"))]);
    let folder_inner = vars(&[("who", serde_json::json!("folder-inner"))]);
    let request = vars(&[("who", serde_json::json!("request"))]);
    let runtime = vars(&[("who", serde_json::json!("runtime"))]);

    // full stack: runtime wins
    let mut inputs = StackInputs {
        process_env: svars(&[("who", "process")]),
        dotenv: svars(&[]),
        collection_vars: Some(&collection),
        environment: Some(&env_file),
        secrets: None,
        folder_vars: vec![&folder_outer, &folder_inner],
        request_vars: Some(&request),
        runtime_vars: Some(&runtime),
    };
    let stack = VarStack::build(inputs);
    assert_eq!(interpolate("{{who}}", &stack).text, "runtime");

    // peel layers one by one
    inputs = StackInputs {
        process_env: svars(&[("who", "process")]),
        dotenv: svars(&[]),
        collection_vars: Some(&collection),
        environment: Some(&env_file),
        secrets: None,
        folder_vars: vec![&folder_outer, &folder_inner],
        request_vars: Some(&request),
        runtime_vars: None,
    };
    assert_eq!(
        interpolate("{{who}}", &VarStack::build(inputs)).text,
        "request"
    );

    inputs = StackInputs {
        process_env: svars(&[("who", "process")]),
        dotenv: svars(&[]),
        collection_vars: Some(&collection),
        environment: Some(&env_file),
        secrets: None,
        folder_vars: vec![&folder_outer, &folder_inner],
        request_vars: None,
        runtime_vars: None,
    };
    assert_eq!(
        interpolate("{{who}}", &VarStack::build(inputs)).text,
        "folder-inner"
    );

    inputs = StackInputs {
        process_env: svars(&[("who", "process")]),
        dotenv: svars(&[]),
        collection_vars: Some(&collection),
        environment: Some(&env_file),
        secrets: None,
        folder_vars: vec![&folder_outer],
        request_vars: None,
        runtime_vars: None,
    };
    assert_eq!(
        interpolate("{{who}}", &VarStack::build(inputs)).text,
        "folder-outer"
    );

    inputs = StackInputs {
        process_env: svars(&[("who", "process")]),
        dotenv: svars(&[]),
        collection_vars: Some(&collection),
        environment: Some(&env_file),
        secrets: None,
        folder_vars: vec![],
        request_vars: None,
        runtime_vars: None,
    };
    assert_eq!(
        interpolate("{{who}}", &VarStack::build(inputs)).text,
        "environment"
    );

    inputs = StackInputs {
        process_env: svars(&[("who", "process")]),
        dotenv: svars(&[]),
        collection_vars: Some(&collection),
        environment: None,
        secrets: None,
        folder_vars: vec![],
        request_vars: None,
        runtime_vars: None,
    };
    assert_eq!(
        interpolate("{{who}}", &VarStack::build(inputs)).text,
        "collection"
    );

    inputs = StackInputs {
        process_env: svars(&[("who", "process")]),
        ..Default::default()
    };
    assert_eq!(
        interpolate("{{who}}", &VarStack::build(inputs)).text,
        "process"
    );
}

#[test]
fn dotenv_overrides_process_env_within_the_lowest_layer() {
    let inputs = StackInputs {
        process_env: svars(&[("KEY", "from-process")]),
        dotenv: svars(&[("KEY", "from-dotenv")]),
        ..Default::default()
    };
    assert_eq!(
        interpolate("{{KEY}}", &VarStack::build(inputs)).text,
        "from-dotenv"
    );
}

#[test]
fn dot_and_index_paths() {
    let request = vars(&[(
        "user",
        serde_json::json!({
            "name": "Ada",
            "address": { "street": "Baker St" },
            "tags": ["a", "b", "c"],
            "orders": [{ "id": 7 }]
        }),
    )]);
    let inputs = StackInputs {
        request_vars: Some(&request),
        ..Default::default()
    };
    let stack = VarStack::build(inputs);

    assert_eq!(interpolate("{{user.name}}", &stack).text, "Ada");
    assert_eq!(
        interpolate("{{user.address.street}}", &stack).text,
        "Baker St"
    );
    assert_eq!(interpolate("{{user.tags[1]}}", &stack).text, "b");
    assert_eq!(interpolate("{{user.orders[0].id}}", &stack).text, "7");
    // whole object stringifies as compact JSON
    assert_eq!(
        interpolate("{{user.address}}", &stack).text,
        "{\"street\":\"Baker St\"}"
    );
    // missing path -> verbatim + warning
    let out = interpolate("{{user.missing.deep}}", &stack);
    assert_eq!(out.text, "{{user.missing.deep}}");
    assert_eq!(out.warnings.len(), 1);
}

#[test]
fn typed_values_stringify_naturally() {
    let request = vars(&[
        ("retries", serde_json::json!(3)),
        ("ratio", serde_json::json!(1.5)),
        ("flag", serde_json::json!(true)),
    ]);
    let inputs = StackInputs {
        request_vars: Some(&request),
        ..Default::default()
    };
    let stack = VarStack::build(inputs);
    assert_eq!(
        interpolate("r={{retries}} x={{ratio}} f={{flag}}", &stack).text,
        "r=3 x=1.5 f=true"
    );
}

#[test]
fn recursive_interpolation_and_cycles() {
    let request = vars(&[
        ("url", serde_json::json!("{{base}}/users")),
        ("base", serde_json::json!("https://api.test")),
        ("a", serde_json::json!("{{b}}")),
        ("b", serde_json::json!("{{a}}")),
    ]);
    let inputs = StackInputs {
        request_vars: Some(&request),
        ..Default::default()
    };
    let stack = VarStack::build(inputs);

    assert_eq!(
        interpolate("{{url}}", &stack).text,
        "https://api.test/users"
    );

    let out = interpolate("{{a}}", &stack);
    assert!(
        out.text == "{{b}}" || out.text == "{{a}}",
        "cycle stays verbatim: {}",
        out.text
    );
    assert!(
        out.warnings
            .iter()
            .any(|w| matches!(w, Warning::Cycle { .. })),
        "{:?}",
        out.warnings
    );
}

#[test]
fn depth_cap_is_enforced() {
    let mut pairs = Vec::new();
    for i in 0..15 {
        pairs.push((
            format!("v{i}"),
            serde_json::json!(format!("{{{{v{}}}}}", i + 1)),
        ));
    }
    pairs.push(("v15".to_string(), serde_json::json!("done")));
    let request: IndexMap<String, VarValue> = pairs.into_iter().collect();
    let inputs = StackInputs {
        request_vars: Some(&request),
        ..Default::default()
    };
    let out = interpolate("{{v0}}", &VarStack::build(inputs));
    assert!(
        out.warnings
            .iter()
            .any(|w| matches!(w, Warning::DepthExceeded { .. })),
        "expected depth warning, got {:?} -> {}",
        out.warnings,
        out.text
    );
}

#[test]
fn unknown_vars_stay_verbatim_with_one_warning() {
    let stack = VarStack::build(StackInputs::default());
    let out = interpolate("{{missing}} and {{missing}} again", &stack);
    assert_eq!(out.text, "{{missing}} and {{missing}} again");
    assert_eq!(
        out.warnings,
        vec![Warning::Unknown {
            name: "missing".into()
        }]
    );
}

#[test]
fn dynamic_vars_are_fresh_per_occurrence() {
    let stack = VarStack::build(StackInputs::default());
    let out = interpolate("{{$uuid}}:{{$uuid}}", &stack).text;
    let (a, b) = out.split_once(':').unwrap();
    assert_eq!(a.len(), 36);
    assert_ne!(a, b);

    let ts = interpolate("{{$timestamp}}", &stack).text;
    assert!(ts.parse::<i64>().is_ok());
}

#[test]
fn whitespace_inside_braces_is_tolerated() {
    let request = vars(&[("name", serde_json::json!("tomo"))]);
    let inputs = StackInputs {
        request_vars: Some(&request),
        ..Default::default()
    };
    assert_eq!(
        interpolate("{{ name }}", &VarStack::build(inputs)).text,
        "tomo"
    );
}

#[test]
fn secrets_resolution_order_and_missing_warning() {
    let env_file = EnvironmentFile {
        meta: EnvMeta {
            name: "dev".into(),
            secrets: vec![
                "from_env_secrets".into(),
                "from_collection_secrets".into(),
                "from_dotenv".into(),
                "from_process".into(),
                "nowhere".into(),
            ],
        },
        vars: IndexMap::new(),
    };
    let secrets = SecretsFile {
        collection: svars(&[
            ("from_collection_secrets", "col-value"),
            // env-scoped must beat collection-scoped:
            ("from_env_secrets", "col-shadowed"),
        ]),
        environments: IndexMap::from([(
            "dev".to_string(),
            svars(&[("from_env_secrets", "env-value")]),
        )]),
    };
    let inputs = StackInputs {
        process_env: svars(&[("from_process", "proc-value")]),
        dotenv: svars(&[("from_dotenv", "dotenv-value")]),
        environment: Some(&env_file),
        secrets: Some(&secrets),
        ..Default::default()
    };
    let stack = VarStack::build(inputs);

    assert_eq!(
        interpolate("{{from_env_secrets}}", &stack).text,
        "env-value"
    );
    assert_eq!(
        interpolate("{{from_collection_secrets}}", &stack).text,
        "col-value"
    );
    assert_eq!(interpolate("{{from_dotenv}}", &stack).text, "dotenv-value");
    assert_eq!(interpolate("{{from_process}}", &stack).text, "proc-value");
    assert_eq!(interpolate("{{nowhere}}", &stack).text, "");

    assert_eq!(stack.missing_secrets, vec!["nowhere".to_string()]);
    assert!(stack.secret_names.contains("from_env_secrets"));
    assert_eq!(stack.secret_names.len(), 5);
}

// ---- secret masking on display surfaces ----------------------------------

#[test]
fn secret_values_are_longest_first_and_skip_trivially_short() {
    let env_file = EnvironmentFile {
        meta: EnvMeta {
            name: "dev".into(),
            secrets: vec!["token".into(), "pin".into(), "api_key".into()],
        },
        vars: IndexMap::new(),
    };
    let secrets = SecretsFile {
        collection: svars(&[
            ("token", "supersecret-token-value"),
            ("pin", "99"), // under the min mask length -> skipped
            ("api_key", "abcd1234"),
        ]),
        environments: IndexMap::new(),
    };
    let stack = VarStack::build(StackInputs {
        environment: Some(&env_file),
        secrets: Some(&secrets),
        ..Default::default()
    });

    assert_eq!(
        stack.secret_values(),
        vec![
            "supersecret-token-value".to_string(),
            "abcd1234".to_string(),
        ],
        "resolved secret values, longest-first, short ones dropped"
    );
}

#[test]
fn mask_secrets_redacts_every_occurrence() {
    let secrets = vec!["abcd1234".to_string()];
    let masked = mask_secrets("Bearer abcd1234 then echo abcd1234", &secrets);
    assert!(!masked.contains("abcd1234"), "no secret left: {masked}");
    assert_eq!(
        masked,
        format!("Bearer {SECRET_MASK} then echo {SECRET_MASK}")
    );
}

#[test]
fn interpolate_masked_hides_secrets_but_keeps_plain_vars() {
    let env_file = EnvironmentFile {
        meta: EnvMeta {
            name: "dev".into(),
            secrets: vec!["api_key".into()],
        },
        vars: IndexMap::from([("host".to_string(), VarValue::String("api.test".into()))]),
    };
    let secrets = SecretsFile {
        collection: svars(&[("api_key", "abcd1234secret")]),
        environments: IndexMap::new(),
    };
    let stack = VarStack::build(StackInputs {
        environment: Some(&env_file),
        secrets: Some(&secrets),
        ..Default::default()
    });

    // real interpolation still resolves the secret — that's what goes on the wire
    assert_eq!(
        interpolate("{{host}}:{{api_key}}", &stack).text,
        "api.test:abcd1234secret"
    );
    // masked interpolation redacts the secret, keeps the ordinary variable
    assert_eq!(
        interpolate_masked("{{host}}:{{api_key}}", &stack).text,
        format!("api.test:{SECRET_MASK}")
    );
}

#[test]
fn visible_reports_winning_scope() {
    let collection = vars(&[("a", serde_json::json!(1)), ("b", serde_json::json!(1))]);
    let runtime = vars(&[("b", serde_json::json!(2))]);
    let inputs = StackInputs {
        collection_vars: Some(&collection),
        runtime_vars: Some(&runtime),
        ..Default::default()
    };
    let visible = VarStack::build(inputs).visible();
    assert_eq!(visible.get("a"), Some(&Scope::Collection));
    assert_eq!(visible.get("b"), Some(&Scope::Runtime));
}
