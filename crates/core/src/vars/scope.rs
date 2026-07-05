//! The layered variable stack.

use std::collections::HashSet;

use indexmap::IndexMap;

use crate::model::{EnvironmentFile, SecretsFile, VarValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Process,
    Collection,
    Environment,
    Folder,
    Request,
    Runtime,
}

/// Everything needed to assemble a stack for one request run.
/// All inputs are injected (no global reads) so tests stay hermetic.
#[derive(Default)]
pub struct StackInputs<'a> {
    pub process_env: IndexMap<String, String>,
    pub dotenv: IndexMap<String, String>,
    pub collection_vars: Option<&'a IndexMap<String, VarValue>>,
    pub environment: Option<&'a EnvironmentFile>,
    pub secrets: Option<&'a SecretsFile>,
    /// Folder chain from the collection root down to the request's folder.
    pub folder_vars: Vec<&'a IndexMap<String, VarValue>>,
    pub request_vars: Option<&'a IndexMap<String, VarValue>>,
    pub runtime_vars: Option<&'a IndexMap<String, VarValue>>,
}

#[derive(Debug, Clone, Default)]
pub struct VarStack {
    /// Layers ordered low → high precedence.
    layers: Vec<(Scope, IndexMap<String, VarValue>)>,
    /// Names whose values are secrets (for masking in UIs/logs).
    pub secret_names: HashSet<String>,
    /// Secrets listed in the environment but found nowhere.
    pub missing_secrets: Vec<String>,
}

impl VarStack {
    pub fn build(inputs: StackInputs<'_>) -> Self {
        let mut stack = VarStack::default();

        // process env + .env (dotenv overrides process within the same layer)
        let mut process: IndexMap<String, VarValue> = IndexMap::new();
        for (k, v) in &inputs.process_env {
            process.insert(k.clone(), VarValue::String(v.clone()));
        }
        for (k, v) in &inputs.dotenv {
            process.insert(k.clone(), VarValue::String(v.clone()));
        }
        stack.layers.push((Scope::Process, process));

        if let Some(vars) = inputs.collection_vars {
            stack.layers.push((Scope::Collection, vars.clone()));
        }

        if let Some(env) = inputs.environment {
            let mut layer = env.vars.clone();
            for name in &env.meta.secrets {
                stack.secret_names.insert(name.clone());
                let resolved = inputs
                    .secrets
                    .and_then(|s| s.environments.get(&env.meta.name))
                    .and_then(|m| m.get(name))
                    .or_else(|| inputs.secrets.and_then(|s| s.collection.get(name)))
                    .cloned()
                    .or_else(|| inputs.dotenv.get(name).cloned())
                    .or_else(|| inputs.process_env.get(name).cloned());
                match resolved {
                    Some(v) => {
                        layer.insert(name.clone(), VarValue::String(v));
                    }
                    None => {
                        stack.missing_secrets.push(name.clone());
                        layer.insert(name.clone(), VarValue::String(String::new()));
                    }
                }
            }
            stack.layers.push((Scope::Environment, layer));
        }

        for vars in &inputs.folder_vars {
            stack.layers.push((Scope::Folder, (*vars).clone()));
        }
        if let Some(vars) = inputs.request_vars {
            stack.layers.push((Scope::Request, vars.clone()));
        }
        if let Some(vars) = inputs.runtime_vars {
            stack.layers.push((Scope::Runtime, vars.clone()));
        }

        stack
    }

    /// Look up a plain name (no dot path), highest precedence first.
    pub fn resolve(&self, name: &str) -> Option<(&VarValue, Scope)> {
        self.layers
            .iter()
            .rev()
            .find_map(|(scope, map)| map.get(name).map(|v| (v, *scope)))
    }

    /// Winning value of every visible variable, flattened for script snapshots.
    pub fn flatten(&self) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        for (_, map) in &self.layers {
            for (name, value) in map {
                out.insert(name.clone(), value.clone()); // higher layers overwrite
            }
        }
        serde_json::Value::Object(out)
    }

    /// All visible names with their winning scope (for UI autocomplete).
    pub fn visible(&self) -> IndexMap<String, Scope> {
        let mut out: IndexMap<String, Scope> = IndexMap::new();
        for (scope, map) in &self.layers {
            for name in map.keys() {
                out.insert(name.clone(), *scope); // later (higher) layers overwrite
            }
        }
        out
    }
}
