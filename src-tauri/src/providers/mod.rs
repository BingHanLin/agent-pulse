pub mod claude;
pub mod opencode;

use serde::Serialize;

pub trait HookProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn badge_label(&self) -> &str;
    fn badge_color(&self) -> &str;
    fn install(&self, port: u16) -> Result<(), String>;
    fn remove(&self) -> Result<(), String>;
    fn is_installed(&self) -> bool;
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub badge_label: String,
    pub badge_color: String,
    pub installed: bool,
}

pub struct ProviderRegistry {
    providers: Vec<Box<dyn HookProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn HookProvider>) {
        self.providers.push(provider);
    }

    pub fn get(&self, id: &str) -> Option<&dyn HookProvider> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
    }

    pub fn list(&self) -> Vec<ProviderInfo> {
        self.providers
            .iter()
            .map(|p| ProviderInfo {
                id: p.id().to_string(),
                display_name: p.display_name().to_string(),
                badge_label: p.badge_label().to_string(),
                badge_color: p.badge_color().to_string(),
                installed: p.is_installed(),
            })
            .collect()
    }
}

pub fn create_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(claude::ClaudeCodeProvider));
    registry.register(Box::new(opencode::OpenCodeProvider));
    registry
}
