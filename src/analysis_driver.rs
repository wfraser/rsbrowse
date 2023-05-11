use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

pub struct Analysis {
    map: HashMap<Id, Entry>,
}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>) -> Result<(), String> {
        let me = std::env::current_exe()
            .map_err(|e| format!("failed to get current exe path: {e}"))?;
        let cargo = Command::new("cargo")
            .arg("check")
            .env("RUSTC_WRAPPER", me)
            .current_dir(workspace_path)
            .status()
            .map_err(|e| {
                format!("failed to run 'cargo build': {e}")
            })?;

        if cargo.success() {
            Ok(())
        } else if let Some(code) = cargo.code() {
            Err(format!("'cargo build' failed with exit code {code}"))
        } else {
            Err("'cargo build' killed by signal".to_owned())
        }
    }

    pub fn load(workspace_path: impl AsRef<Path>) -> Self {
        let path = workspace_path.as_ref()
            .join("target")
            .join("rsbrowse.json");
        eprintln!("Reading {path:?}");
        let mut f = File::open(&path)
            .expect("failed to open rsbrowse.json");

        let mut map = HashMap::new();
        let mut buf = vec![];
        f.read_to_end(&mut buf).unwrap();
        for value in serde_json::Deserializer::from_slice(&buf).into_iter() {
            let value: Entry = value.expect("deserialization error");
            map.insert(value.id.clone(), value);
        }

        Self { map }
    }

    pub fn crates(&self) -> impl Iterator<Item = (&Id, &str)> {
        self.map
            .values()
            .filter_map(|e| {
                if let Kind::Crate(name) = &e.kind {
                    Some((&e.id, name.as_str()))
                } else {
                    None
                }
            })
    }

    pub fn entries_under(&self, parent: &Id) -> Vec<&Entry> {
        if let Id::Crate(num) = parent {
            return self.map
                .iter()
                .filter_map(|(id, entry)| {
                    match id {
                        Id::Def(krate, _) if krate == num => Some(entry),
                        _ => None,
                    }
                })
                .collect();
        }

        let parent_entry = match self.map.get(parent) {
            Some(e) => e,
            None => return vec![],
        };

        let mut entries = parent_entry.children
            .iter()
            .filter_map(|id| self.map.get(id))
            .collect::<Vec<_>>();
        
        // Find impls too.
        for e in self.map.values() {
            if let Kind::Impl { on, .. } = &e.kind {
                if on == parent {
                    entries.push(e);
                }
            }
        }

        entries
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: Id,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Id>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(untagged)]
pub enum Id {
    Crate(u32),
    Def(u32, u32),
    Primitive(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Kind {
    Crate(String),
    Static(String),
    Const(String),
    Fn(String),
    Macro(String),
    Mod(String),
    Extern(String),
    Field { name: String, ty: Id },
    Struct(String),
    Impl { of: Option<Id>, on: Id },
    Type(String),
}
