use std::collections::BTreeMap;

use crate::{CompileError, ModuleId, Result, SymbolId};

/// Module catalog used for import resolution and stable module id assignment.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModuleCatalog {
    modules: BTreeMap<String, ModuleId>,
    next_module_id: ModuleId,
}

impl ModuleCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, path: &str) -> ModuleId {
        if let Some(module_id) = self.modules.get(path) {
            return *module_id;
        }
        let module_id = self.next_module_id.max(1);
        self.next_module_id = module_id.saturating_add(1);
        self.modules.insert(path.to_owned(), module_id);
        module_id
    }

    pub fn resolve(&self, path: &str) -> Option<ModuleId> {
        self.modules.get(path).copied()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.modules.contains_key(path)
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.modules.keys().map(String::as_str)
    }
}

/// Symbol kinds tracked by the resolver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Import,
    Function,
    Global,
    Parameter,
}

/// Symbol table entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolEntry {
    pub symbol_id: SymbolId,
    pub kind: SymbolKind,
    pub exported: bool,
}

/// Symbol table used in the resolver and IR.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolTable {
    entries: BTreeMap<String, SymbolEntry>,
    next_symbol_id: SymbolId,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        name: impl Into<String>,
        kind: SymbolKind,
        exported: bool,
    ) -> Result<SymbolId> {
        let name = name.into();
        if self.entries.contains_key(&name) {
            return Err(CompileError::DuplicateSymbol { name });
        }
        let symbol_id = self.next_symbol_id.max(1);
        self.next_symbol_id = symbol_id.saturating_add(1);
        self.entries.insert(
            name,
            SymbolEntry {
                symbol_id,
                kind,
                exported,
            },
        );
        Ok(symbol_id)
    }

    pub fn get(&self, name: &str) -> Option<&SymbolEntry> {
        self.entries.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &SymbolEntry)> {
        self.entries
            .iter()
            .map(|(name, entry)| (name.as_str(), entry))
    }
}
