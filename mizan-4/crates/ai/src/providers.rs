//! AI provider catalog and client management.
//!
//! This module provides:
//! - Provider catalog loaded from JSON configuration
//! - Client factory for rig-core providers
//! - API key management via the environment's secret store

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::env::AiEnvironment;
use crate::error::AiError;
use crate::provider_model::{
    AiProviderSettings, CapabilityInfo, ConnectionField, ModelCapabilities, ProviderDefaultConfig,
    ProviderTuning, AI_PROVIDER_SETTINGS_KEY,
};
use crate::types::normalize_tools_allowlist;

// ============================================================================
// Provider Catalog (Static JSON)
// ============================================================================

/// Static provider catalog loaded from embedded JSON.
static PROVIDER_CATALOG: Lazy<ProviderCatalog> = Lazy::new(|| {
    let json = include_str!("ai_providers.json");
    serde_json::from_str(json).expect("Failed to parse ai_providers.json")
});

#[derive(Debug, Deserialize)]
struct ProviderCatalog {
    providers: HashMap<String, ProviderCatalogEntry>,
    capabilities: HashMap<String, CapabilityInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderCatalogEntry {
    name: String,
    #[serde(rename = "type")]
    provider_type: String,
    icon: String,
    description: String,
    #[serde(default)]
    env_key: Option<String>,
    #[serde(default)]
    default_config: ProviderDefaultConfig,
    #[serde(default)]
    connection_fields: Vec<ConnectionField>,
    models: HashMap<String, ModelCatalogEntry>,
    default_model: String,
    /// Fast model for title generation (falls back to default_model if not set).
    #[serde(default)]
    title_model_id: Option<String>,
    #[serde(default)]
    documentation_url: Option<String>,
    /// Sampling/output defaults for this provider (temperature, max_tokens, etc.).
    #[serde(default)]
    tuning: Option<ProviderTuning>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelCatalogEntry {
    #[serde(default)]
    capabilities: ModelCapabilities,
}

// ============================================================================
// Local Types (simplified views for this service)
// ============================================================================

/// Simple provider info for catalog listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleProviderInfo {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub icon: String,
    pub description: String,
    pub default_model: String,
    pub documentation_url: Option<String>,
    #[serde(default)]
    pub default_config: ProviderDefaultConfig,
    #[serde(default)]
    pub connection_fields: Vec<ConnectionField>,
    #[serde(default)]
    pub models: Vec<SimpleModelInfo>,
    #[serde(default)]
    pub env_key: Option<String>,
}

/// Simple model info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleModelInfo {
    pub id: String,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
}

/// Provider setting for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleProviderSetting {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider_type: String,
    pub icon: String,
    pub default_model: String,
    pub enabled: bool,
    #[serde(default)]
    pub supports_custom_url: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub documentation_url: Option<String>,
    #[serde(default)]
    pub env_key: Option<String>,
    #[serde(default)]
    pub models: Vec<SimpleModelInfo>,
}

/// Combined settings response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleSettings {
    pub provider_id: String,
    pub model: String,
    pub has_api_key: bool,
    pub providers: Vec<SimpleProviderSetting>,
    #[serde(default)]
    pub capabilities: HashMap<String, CapabilityInfo>,
}

// ============================================================================
// Provider Service
// ============================================================================

/// Service key for storing AI provider settings.
pub const AI_SETTINGS_KEY: &str = "ai_settings";

/// Provider service for managing AI settings.
pub struct ProviderService<E: AiEnvironment> {
    env: Arc<E>,
}

impl<E: AiEnvironment> ProviderService<E> {
    /// Create a new provider service.
    pub fn new(env: Arc<E>) -> Self {
        Self { env }
    }

    /// Get all provider info from the catalog.
    pub fn get_provider_catalog(&self) -> Vec<SimpleProviderInfo> {
        PROVIDER_CATALOG
            .providers
            .iter()
            .map(|(id, entry)| SimpleProviderInfo {
                id: id.clone(),
                name: entry.name.clone(),
                provider_type: entry.provider_type.clone(),
                icon: entry.icon.clone(),
                description: entry.description.clone(),
                default_model: entry.default_model.clone(),
                documentation_url: entry.documentation_url.clone(),
                default_config: entry.default_config.clone(),
                connection_fields: entry.connection_fields.clone(),
                models: entry
                    .models
                    .iter()
                    .map(|(id, m)| SimpleModelInfo {
                        id: id.clone(),
                        capabilities: m.capabilities.clone(),
                    })
                    .collect(),
                env_key: entry.env_key.clone(),
            })
            .collect()
    }

    /// Get capability info.
    pub fn get_capabilities(&self) -> HashMap<String, CapabilityInfo> {
        PROVIDER_CATALOG.capabilities.clone()
    }

    /// Get the current AI settings (merged from catalog + stored settings).
    pub fn get_settings(&self) -> Result<SimpleSettings, AiError> {
        // Load stored settings
        let stored: StoredAiSettings = self
            .env
            .settings_service()
            .get_setting_value(AI_SETTINGS_KEY)
            .map_err(|e| AiError::Internal(e.to_string()))?
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        // Get current provider and model
        let provider_id = stored
            .provider_id
            .clone()
            .unwrap_or_else(|| "ollama".to_string());
        let model = stored.model.clone().unwrap_or_else(|| {
            PROVIDER_CATALOG
                .providers
                .get(&provider_id)
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "deepseek-r1:8b".to_string())
        });

        // Check if we have an API key
        let has_api_key = self.has_api_key(&provider_id);

        // Build provider settings
        let providers: Vec<SimpleProviderSetting> = PROVIDER_CATALOG
            .providers
            .iter()
            .map(|(id, entry)| {
                let stored_provider = stored.providers.get(id);
                let enabled = stored_provider
                    .and_then(|p| p.enabled)
                    .unwrap_or(entry.default_config.enabled);
                let url = stored_provider
                    .and_then(|p| p.url.clone())
                    .or_else(|| entry.default_config.url.clone());

                SimpleProviderSetting {
                    id: id.clone(),
                    name: entry.name.clone(),
                    description: entry.description.clone(),
                    provider_type: entry.provider_type.clone(),
                    icon: entry.icon.clone(),
                    default_model: entry.default_model.clone(),
                    enabled,
                    supports_custom_url: entry.provider_type == "local",
                    url,
                    documentation_url: entry.documentation_url.clone(),
                    env_key: entry.env_key.clone(),
                    models: entry
                        .models
                        .iter()
                        .map(|(id, m)| SimpleModelInfo {
                            id: id.clone(),
                            capabilities: m.capabilities.clone(),
                        })
                        .collect(),
                }
            })
            .collect();

        Ok(SimpleSettings {
            provider_id,
            model,
            has_api_key,
            providers,
            capabilities: PROVIDER_CATALOG.capabilities.clone(),
        })
    }

    /// Build the secret key for a provider (format: ai_<provider_id>).
    /// Matches the frontend convention in use-ai-providers.ts.
    fn secret_key_for_provider(provider_id: &str) -> String {
        format!("ai_{}", provider_id)
    }

    /// Get API key for a provider. Resolution order:
    /// 1. OS secret store (what the user pasted in Settings → AI
    ///    Providers). Survives across launches; encrypted by the OS
    ///    keychain.
    /// 2. The catalog-declared env var (e.g. `ANTHROPIC_API_KEY`),
    ///    picked up from `.env.local` via the Tauri runner's
    ///    `dotenvy` load on boot.
    ///
    /// The env fallback is what makes the Uncle Feroz demo "just
    /// work" when the key is dropped in `.env.local`: no Settings
    /// pane click-through required. The env value is never written
    /// back to the secret store — clearing the env re-locks the
    /// provider correctly. Whitespace-only values in either source
    /// are treated as unset.
    ///
    /// Mirrors the precedence logic in
    /// `provider_service::resolve_api_key`; kept here as a parallel
    /// implementation rather than a shared helper because the two
    /// services live in different modules with no shared parent
    /// suitable for a util.
    pub fn get_api_key(&self, provider_id: &str) -> Result<Option<String>, AiError> {
        let secret_key = Self::secret_key_for_provider(provider_id);
        let from_store = self
            .env
            .secret_store()
            .get_secret(&secret_key)
            .map_err(|e| AiError::Internal(e.to_string()))?
            .filter(|s| !s.trim().is_empty());
        if from_store.is_some() {
            return Ok(from_store);
        }
        let env_value = PROVIDER_CATALOG
            .providers
            .get(provider_id)
            .and_then(|p| p.env_key.as_deref())
            .filter(|k| !k.is_empty())
            .and_then(|key| std::env::var(key).ok())
            .filter(|s| !s.trim().is_empty());
        Ok(env_value)
    }

    /// Check if a provider has an API key stored.
    pub fn has_api_key(&self, provider_id: &str) -> bool {
        self.get_api_key(provider_id)
            .ok()
            .flatten()
            .map(|k| !k.is_empty())
            .unwrap_or(false)
    }

    /// Set API key for a provider.
    pub async fn set_api_key(&self, provider_id: &str, api_key: &str) -> Result<(), AiError> {
        let secret_key = Self::secret_key_for_provider(provider_id);
        self.env
            .secret_store()
            .set_secret(&secret_key, api_key)
            .map_err(|e| AiError::Internal(e.to_string()))
    }

    /// Delete API key for a provider.
    pub async fn delete_api_key(&self, provider_id: &str) -> Result<(), AiError> {
        let secret_key = Self::secret_key_for_provider(provider_id);
        self.env
            .secret_store()
            .delete_secret(&secret_key)
            .map_err(|e| AiError::Internal(e.to_string()))
    }

    /// Get model capabilities for a specific provider/model combination.
    /// Checks user capability overrides first, then falls back to catalog, then defaults.
    pub fn get_model_capabilities(&self, provider_id: &str, model_id: &str) -> ModelCapabilities {
        // First, get base capabilities from catalog
        let catalog_capabilities = PROVIDER_CATALOG
            .providers
            .get(provider_id)
            .and_then(|p| p.models.get(model_id))
            .map(|m| m.capabilities.clone());

        // Check for user capability overrides in the new settings system
        let user_overrides = self
            .env
            .settings_service()
            .get_setting_value(AI_PROVIDER_SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<AiProviderSettings>(&s).ok())
            .and_then(|settings| settings.providers.get(provider_id).cloned())
            .and_then(|provider_settings| {
                provider_settings
                    .model_capability_overrides
                    .get(model_id)
                    .cloned()
            });

        // Build final capabilities: start with catalog or defaults, then apply overrides
        let base = catalog_capabilities.unwrap_or(ModelCapabilities {
            tools: false,
            thinking: false,
            vision: false,
            streaming: true,
        });

        // Apply user overrides if present
        if let Some(overrides) = user_overrides {
            ModelCapabilities {
                tools: overrides.tools.unwrap_or(base.tools),
                thinking: overrides.thinking.unwrap_or(base.thinking),
                vision: overrides.vision.unwrap_or(base.vision),
                streaming: overrides.streaming.unwrap_or(base.streaming),
            }
        } else {
            base
        }
    }

    /// Get the title model ID for a provider.
    /// Returns title_model_id if configured, otherwise falls back to default_model.
    pub fn get_title_model(&self, provider_id: &str) -> Option<String> {
        PROVIDER_CATALOG.providers.get(provider_id).map(|p| {
            p.title_model_id
                .clone()
                .unwrap_or_else(|| p.default_model.clone())
        })
    }

    /// Get the tools allowlist for a provider.
    /// Returns None if all tools are allowed, Some(list) if only specific tools are allowed.
    pub fn get_tools_allowlist(&self, provider_id: &str) -> Option<Vec<String>> {
        let stored: AiProviderSettings = self
            .env
            .settings_service()
            .get_setting_value(AI_PROVIDER_SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        stored
            .providers
            .get(provider_id)
            .and_then(|p| normalize_tools_allowlist(p.tools_allowlist.clone()))
    }

    /// Resolve effective provider tuning: catalog defaults merged with any
    /// user overrides persisted under `ai_provider_settings`. Returns an empty
    /// (all-`None`) `ProviderTuning` when neither the catalog nor the user has
    /// set anything — callers treat that as "leave provider defaults alone."
    pub fn get_resolved_tuning(&self, provider_id: &str) -> ProviderTuning {
        let catalog_tuning = PROVIDER_CATALOG
            .providers
            .get(provider_id)
            .and_then(|p| p.tuning.clone())
            .unwrap_or_default();

        let stored: AiProviderSettings = self
            .env
            .settings_service()
            .get_setting_value(AI_PROVIDER_SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let user_overrides = stored
            .providers
            .get(provider_id)
            .and_then(|p| p.tuning_overrides.clone());

        match user_overrides {
            Some(ovr) => catalog_tuning.apply_overrides(&ovr),
            None => catalog_tuning,
        }
    }

    /// Get provider URL (for local providers like Ollama).
    pub fn get_provider_url(&self, provider_id: &str) -> Option<String> {
        let stored: AiProviderSettings = self
            .env
            .settings_service()
            .get_setting_value(AI_PROVIDER_SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let url = stored
            .providers
            .get(provider_id)
            .and_then(|p| p.custom_url.clone())
            .or_else(|| {
                PROVIDER_CATALOG
                    .providers
                    .get(provider_id)
                    .and_then(|p| p.default_config.url.clone())
            });

        // Validate URL to prevent panics in rig-core's HTTP client
        url.filter(|u| reqwest::Url::parse(u).is_ok())
    }

    /// Update AI settings.
    pub async fn update_settings(
        &self,
        provider_id: Option<String>,
        model: Option<String>,
        provider_config: Option<StoredProviderConfig>,
    ) -> Result<SimpleSettings, AiError> {
        // Load current stored settings
        let mut stored: StoredAiSettings = self
            .env
            .settings_service()
            .get_setting_value(AI_SETTINGS_KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        // Update fields
        if let Some(p) = provider_id {
            stored.provider_id = Some(p);
        }
        if let Some(m) = model {
            stored.model = Some(m);
        }
        if let Some(config) = provider_config {
            stored.providers.insert(
                config.id.clone(),
                StoredProviderSettings {
                    enabled: Some(config.enabled),
                    url: config.url,
                },
            );
        }

        // Save
        let json = serde_json::to_string(&stored).map_err(|e| AiError::Internal(e.to_string()))?;
        self.env
            .settings_service()
            .set_setting_value(AI_SETTINGS_KEY, &json)
            .await
            .map_err(|e| AiError::Internal(e.to_string()))?;

        self.get_settings()
    }
}

/// Stored AI settings (in app_settings).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAiSettings {
    pub provider_id: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, StoredProviderSettings>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredProviderSettings {
    pub enabled: Option<bool>,
    pub url: Option<String>,
}

/// Config update for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredProviderConfig {
    pub id: String,
    pub enabled: bool,
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_catalog_loads() {
        let catalog = &*PROVIDER_CATALOG;
        assert!(!catalog.providers.is_empty());
        assert!(catalog.providers.contains_key("openai"));
        assert!(catalog.providers.contains_key("ollama"));
    }

    #[test]
    fn test_capabilities_loads() {
        let catalog = &*PROVIDER_CATALOG;
        assert!(catalog.capabilities.contains_key("tools"));
        assert!(catalog.capabilities.contains_key("thinking"));
    }

    // Env-fallback tests for ProviderService::get_api_key. Use a
    // process-global Mutex to serialise tests that mutate
    // ANTHROPIC_API_KEY — env state is shared across the test
    // runner's threads.
    use crate::env::test_env::MockEnvironment;
    use std::sync::{Arc, Mutex};
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn fresh_provider_service() -> ProviderService<MockEnvironment> {
        ProviderService::new(Arc::new(MockEnvironment::new()))
    }

    #[test]
    fn get_api_key_falls_back_to_env_when_secret_store_empty() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-env-fallback-works");
        let svc = fresh_provider_service();
        let got = svc.get_api_key("anthropic").unwrap();
        assert_eq!(got.as_deref(), Some("sk-ant-env-fallback-works"));
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn get_api_key_prefers_secret_store_over_env() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ANTHROPIC_API_KEY", "from-env-should-lose");
        let svc = ProviderService::new(Arc::new(
            MockEnvironment::new().with_secret("ai_anthropic", "from-store-wins"),
        ));
        let got = svc.get_api_key("anthropic").unwrap();
        assert_eq!(got.as_deref(), Some("from-store-wins"));
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn get_api_key_returns_none_when_both_sources_empty() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("ANTHROPIC_API_KEY");
        let svc = fresh_provider_service();
        assert!(svc.get_api_key("anthropic").unwrap().is_none());
    }

    #[test]
    fn get_api_key_ignores_whitespace_only_env_value() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ANTHROPIC_API_KEY", "    ");
        let svc = fresh_provider_service();
        assert!(svc.get_api_key("anthropic").unwrap().is_none());
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn has_api_key_reports_true_when_only_env_is_set() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-test");
        let svc = fresh_provider_service();
        assert!(
            svc.has_api_key("anthropic"),
            "has_api_key must see env fallback"
        );
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn ollama_provider_with_no_env_key_returns_none_not_random_env() {
        // Ollama's catalog entry has envKey="OLLAMA_API_KEY" but
        // unsetting it must not leak into reading OTHER env vars.
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("OLLAMA_API_KEY");
        // Set an unrelated key to prove no cross-contamination.
        std::env::set_var("ANTHROPIC_API_KEY", "wrong-provider");
        let svc = fresh_provider_service();
        assert!(svc.get_api_key("ollama").unwrap().is_none());
        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_catalog_default_models_are_cataloged() {
        let catalog = &*PROVIDER_CATALOG;

        for (provider_id, provider) in &catalog.providers {
            assert!(
                provider.models.contains_key(&provider.default_model),
                "provider '{}' default_model '{}' is missing from models",
                provider_id,
                provider.default_model
            );

            if let Some(title_model_id) = &provider.title_model_id {
                assert!(
                    provider.models.contains_key(title_model_id),
                    "provider '{}' title_model_id '{}' is missing from models",
                    provider_id,
                    title_model_id
                );
            }
        }
    }
}
