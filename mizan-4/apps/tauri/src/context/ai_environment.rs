//! Tauri-side implementation of AiEnvironment.
//!
//! Provides the mizan-ai crate with access to Tauri services
//! for tool execution and settings management.

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use mizan_ai::{AiEnvironment, ChatRepositoryTrait};
use mizan_core::{
    accounts::AccountServiceTrait, activities::ActivityServiceTrait,
    allocation::AllocationServiceTrait, goals::GoalServiceTrait, health::HealthServiceTrait,
    holdings::HoldingsServiceTrait, income::IncomeServiceTrait,
    performance::PerformanceServiceTrait, quotes::QuoteServiceTrait, secrets::SecretStore,
    settings::SettingsServiceTrait, valuation::ValuationServiceTrait,
};

use crate::services::{cloud_api_base_url, ConnectService};

/// Tauri-side implementation of AiEnvironment.
///
/// Wraps existing services from ServiceContext to provide access
/// to the AI crate for tool execution.
pub struct TauriAiEnvironment {
    base_currency: Arc<RwLock<String>>,
    account_service: Arc<dyn AccountServiceTrait + Send + Sync>,
    activity_service: Arc<dyn ActivityServiceTrait + Send + Sync>,
    holdings_service: Arc<dyn HoldingsServiceTrait + Send + Sync>,
    valuation_service: Arc<dyn ValuationServiceTrait + Send + Sync>,
    goal_service: Arc<dyn GoalServiceTrait + Send + Sync>,
    settings_service: Arc<dyn SettingsServiceTrait + Send + Sync>,
    secret_store: Arc<dyn SecretStore>,
    chat_repository: Arc<dyn ChatRepositoryTrait + Send + Sync>,
    quote_service: Arc<dyn QuoteServiceTrait + Send + Sync>,
    allocation_service: Arc<dyn AllocationServiceTrait + Send + Sync>,
    performance_service: Arc<dyn PerformanceServiceTrait + Send + Sync>,
    income_service: Arc<dyn IncomeServiceTrait + Send + Sync>,
    health_service: Arc<dyn HealthServiceTrait + Send + Sync>,
    /// Lets the AI dispatcher resolve a Mizan Connect JWT for the managed
    /// `mizan` provider. `None` for tests / non-cloud builds.
    connect_service: Option<Arc<ConnectService>>,
}

impl TauriAiEnvironment {
    /// Create a new Tauri AI environment.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_currency: Arc<RwLock<String>>,
        account_service: Arc<dyn AccountServiceTrait + Send + Sync>,
        activity_service: Arc<dyn ActivityServiceTrait + Send + Sync>,
        holdings_service: Arc<dyn HoldingsServiceTrait + Send + Sync>,
        valuation_service: Arc<dyn ValuationServiceTrait + Send + Sync>,
        goal_service: Arc<dyn GoalServiceTrait + Send + Sync>,
        settings_service: Arc<dyn SettingsServiceTrait + Send + Sync>,
        secret_store: Arc<dyn SecretStore>,
        chat_repository: Arc<dyn ChatRepositoryTrait + Send + Sync>,
        quote_service: Arc<dyn QuoteServiceTrait + Send + Sync>,
        allocation_service: Arc<dyn AllocationServiceTrait + Send + Sync>,
        performance_service: Arc<dyn PerformanceServiceTrait + Send + Sync>,
        income_service: Arc<dyn IncomeServiceTrait + Send + Sync>,
        health_service: Arc<dyn HealthServiceTrait + Send + Sync>,
        connect_service: Option<Arc<ConnectService>>,
    ) -> Self {
        Self {
            base_currency,
            account_service,
            activity_service,
            holdings_service,
            valuation_service,
            goal_service,
            settings_service,
            secret_store,
            chat_repository,
            quote_service,
            allocation_service,
            performance_service,
            income_service,
            health_service,
            connect_service,
        }
    }
}

#[async_trait]
impl AiEnvironment for TauriAiEnvironment {
    fn base_currency(&self) -> String {
        self.base_currency.read().unwrap().clone()
    }

    fn account_service(&self) -> Arc<dyn AccountServiceTrait> {
        self.account_service.clone()
    }

    fn activity_service(&self) -> Arc<dyn ActivityServiceTrait> {
        self.activity_service.clone()
    }

    fn holdings_service(&self) -> Arc<dyn HoldingsServiceTrait> {
        self.holdings_service.clone()
    }

    fn valuation_service(&self) -> Arc<dyn ValuationServiceTrait> {
        self.valuation_service.clone()
    }

    fn goal_service(&self) -> Arc<dyn GoalServiceTrait> {
        self.goal_service.clone()
    }

    fn settings_service(&self) -> Arc<dyn SettingsServiceTrait> {
        self.settings_service.clone()
    }

    fn secret_store(&self) -> Arc<dyn SecretStore> {
        self.secret_store.clone()
    }

    fn chat_repository(&self) -> Arc<dyn ChatRepositoryTrait> {
        self.chat_repository.clone()
    }

    fn quote_service(&self) -> Arc<dyn QuoteServiceTrait> {
        self.quote_service.clone()
    }

    fn allocation_service(&self) -> Arc<dyn AllocationServiceTrait> {
        self.allocation_service.clone()
    }

    fn performance_service(&self) -> Arc<dyn PerformanceServiceTrait> {
        self.performance_service.clone()
    }

    fn income_service(&self) -> Arc<dyn IncomeServiceTrait> {
        self.income_service.clone()
    }

    fn health_service(&self) -> Arc<dyn HealthServiceTrait> {
        self.health_service.clone()
    }

    async fn connect_access_token(&self) -> Option<String> {
        // The connect service's get_valid_access_token returns
        // `Result<String, String>` — Err on signed-out users or no-cloud
        // builds. Either case translates to `None` here so the chat
        // dispatcher refuses the mizan provider cleanly; the IPC-layer
        // `managed_ai` gate has already explained "upgrade required" to the
        // user upstream.
        let svc = self.connect_service.as_ref()?;
        svc.get_valid_access_token().await.ok()
    }

    async fn connect_api_url(&self) -> Option<String> {
        // `cloud_api_base_url()` already honors `CONNECT_API_URL` at compile
        // time + falls back to `DEFAULT_CLOUD_API_URL`. Returns None when
        // no cloud-sync feature is compiled in.
        cloud_api_base_url()
    }
}
