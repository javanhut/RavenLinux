use crate::package::Dependency;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

pub struct DependencyResolver {
    installed: HashMap<String, String>,
    available: HashMap<String, Vec<String>>,
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            installed: HashMap::new(),
            available: HashMap::new(),
        }
    }

    pub fn set_installed(&mut self, packages: HashMap<String, String>) {
        self.installed = packages;
    }

    pub fn set_available(&mut self, packages: HashMap<String, Vec<String>>) {
        self.available = packages;
    }

    pub fn resolve(&self, packages: &[String]) -> Result<Vec<ResolvedPackage>> {
        let mut resolved = Vec::new();
        let mut visited = HashSet::new();

        for pkg in packages {
            self.resolve_recursive(pkg, &mut resolved, &mut visited)?;
        }

        Ok(resolved)
    }

    fn resolve_recursive(
        &self,
        package: &str,
        resolved: &mut Vec<ResolvedPackage>,
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(package) {
            return Ok(());
        }

        visited.insert(package.to_string());

        // TODO: Look up package dependencies and recursively resolve
        // For now, just add the package itself

        resolved.push(ResolvedPackage {
            name: package.to_string(),
            version: "1.0.0".to_string(),
            action: InstallAction::Install,
        });

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPackage {
    pub name: String,
    pub version: String,
    pub action: InstallAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstallAction {
    Install,
    Upgrade { from: String },
    Reinstall,
    Skip,
}
