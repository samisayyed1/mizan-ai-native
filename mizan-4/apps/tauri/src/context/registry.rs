use mizan_ai::{safety::AiSafetyRuntime, AiProviderServiceTrait, ChatService};
use mizan_connect::BrokerSyncServiceTrait;
use mizan_core::{
    self, accounts, activities,
    assets::{self, AlternativeAssetServiceTrait},
    daily_brief::DailyBriefService,
    events::DomainEventSink,
    fx, goals, health, limits,
    net_worth_snapshot::NetWorthSnapshotService,
    news,
    notifications::NotificationService,
    portfolio, quotes, settings,
    sync_ledger::SyncRunLedger,
    taxonomies,
};
use mizan_device_sync::{engine::DeviceSyncRuntimeState, DeviceEnrollService};
use mizan_financial_truth::TruthLedger;
use mizan_storage_sqlite::{
    daily_brief::SqliteDailyBriefService,
    hawl::HawlAnchorRepository,
    holdings_metadata::HoldingsMetadataRepository,
    net_worth_snapshot::SqliteNetWorthSnapshotService,
    notifications::SqliteNotificationService,
    portfolio::snapshot::SnapshotRepository,
    sync::AppSyncRepository,
    sync_run_ledger::SqliteSyncRunLedger,
    truth_ledger::{SqliteTruthLedger, SqliteTruthLedgerRetryQueue},
    DbPool, WriteHandle,
};
use std::sync::{Arc, RwLock};

use super::TauriAiEnvironment;
use crate::services::ConnectService;

pub struct ServiceContext {
    pub base_currency: Arc<RwLock<String>>,
    pub timezone: Arc<RwLock<String>>,
    pub instance_id: Arc<String>,

    /// Domain event sink for emitting events after mutations.
    /// Runtime bridges (Tauri/Web) implement this to trigger portfolio recalculation,
    /// asset enrichment, and broker sync based on domain events.
    /// Note: The sink is used by services injected at construction time; this field
    /// is kept for documentation and possible future access patterns.
    #[allow(dead_code)]
    pub domain_event_sink: Arc<dyn DomainEventSink>,

    // Services
    pub settings_service: Arc<dyn settings::SettingsServiceTrait>,
    pub activity_service: Arc<dyn activities::ActivityServiceTrait>,
    pub account_service: Arc<dyn accounts::AccountServiceTrait>,
    pub goal_service: Arc<dyn goals::GoalServiceTrait>,
    pub asset_service: Arc<dyn assets::AssetServiceTrait>,
    pub quote_service: Arc<dyn quotes::QuoteServiceTrait>,
    pub news_service: Arc<news::NewsService>,
    pub limits_service: Arc<dyn limits::ContributionLimitServiceTrait>,
    pub fx_service: Arc<dyn fx::FxServiceTrait>,
    pub performance_service: Arc<dyn portfolio::performance::PerformanceServiceTrait>,
    pub income_service: Arc<dyn portfolio::income::IncomeServiceTrait>,
    pub snapshot_service: Arc<dyn portfolio::snapshot::SnapshotServiceTrait>,
    pub snapshot_repository: Arc<SnapshotRepository>,
    /// Track C PR-C5.d.1 — read-only access to `hawl_anchors` so the
    /// insights scheduler can hydrate `ZakatHawlApproaching` candidates.
    pub hawl_anchor_repository: Arc<HawlAnchorRepository>,
    /// Track C PR-C5.d.5 — read-only access to `holdings_metadata` so
    /// the insights scheduler can detect AAOIFI screening flips for
    /// the `ShariaStatusChanged` rule.
    pub holdings_metadata_repository: Arc<HoldingsMetadataRepository>,
    pub app_sync_repository: Arc<AppSyncRepository>,
    pub holdings_service: Arc<dyn portfolio::holdings::HoldingsServiceTrait>,
    pub allocation_service: Arc<dyn portfolio::allocation::AllocationServiceTrait>,
    pub valuation_service: Arc<dyn portfolio::valuation::ValuationServiceTrait>,
    pub net_worth_service: Arc<dyn portfolio::net_worth::NetWorthServiceTrait>,
    pub zakat_service: Arc<dyn mizan_zakat::ZakatServiceTrait>,
    pub sync_service: Arc<dyn BrokerSyncServiceTrait>,
    pub alternative_asset_service: Arc<dyn AlternativeAssetServiceTrait>,
    pub taxonomy_service: Arc<dyn taxonomies::TaxonomyServiceTrait>,
    pub connect_service: Arc<ConnectService>,
    pub ai_provider_service: Arc<dyn AiProviderServiceTrait>,
    pub ai_chat_service: Arc<ChatService<TauriAiEnvironment>>,
    /// Same AiEnvironment the chat service holds. Surfaced separately
    /// so non-chat callers (currently `scheduler::generate_ai_digest`)
    /// can spin up their own LLM clients without going through the
    /// chat dispatcher.
    pub ai_environment: Arc<TauriAiEnvironment>,
    pub device_enroll_service: Arc<DeviceEnrollService>,
    pub device_sync_runtime: Arc<DeviceSyncRuntimeState>,
    pub health_service: Arc<health::HealthService>,
    pub custom_provider_service: Arc<mizan_core::custom_provider::CustomProviderService>,

    // ─── v3.1 foundation services (in-memory for now; SQLite-backed in follow-on PRs) ───
    //
    // §A6 — emits per tool-call audit rows. Owned by ServiceContext so
    // every chat stream attaches the same runtime + every Tauri command
    // can call `state.ai_safety()` to register tool-call attempts. The
    // chat dispatcher already wires this up via `stream_hook`; surface
    // is dead-code-allowed because no Tauri command directly consumes
    // it yet (the chat path uses the runtime through a different
    // injection). Kept public for the upcoming `state.ai_safety()`
    // call sites surfaced in support-bundle endpoints.
    #[allow(dead_code)]
    pub ai_safety: Arc<AiSafetyRuntime>,
    /// §A4 — append-only ledger of every sync attempt (Plaid / SnapTrade /
    /// Yahoo / TradingView / CSV import / AI tools / FX refresh / Manual).
    pub sync_ledger: Arc<dyn SyncRunLedger>,
    /// §A12 — daily net-worth snapshot writer. Dashboard history line +
    /// §A22 delta-computation read from this.
    pub net_worth_snapshot_service: Arc<dyn NetWorthSnapshotService>,
    /// §A22 — daily brief persistence (no email transport yet).
    pub daily_brief_service: Arc<dyn DailyBriefService>,
    /// Notify track — personalized AI wealth-insight notifications.
    /// Backed by the `notifications` SQLite table. Read by the Tauri
    /// commands in `commands::notifications`; written by the insights
    /// scheduler in `scheduler::insights`.
    pub notification_service: Arc<dyn NotificationService>,
    /// §A1/§A2 — immutable hash-chained ledger that activities + accounts +
    /// alt-asset writes append to. Holdings derivation will move to ledger
    /// replay in a follow-on PR.
    pub truth_ledger: Arc<dyn TruthLedger>,
    /// §A1/§A2 — durable retry queue for ledger appends that failed
    /// transiently after the originating row already committed. Drained
    /// on app boot + can be re-drained on demand from the support bundle.
    pub truth_ledger_retry_queue: Arc<SqliteTruthLedgerRetryQueue>,
}

impl ServiceContext {
    pub fn get_base_currency(&self) -> String {
        self.base_currency.read().unwrap().clone()
    }

    pub fn get_timezone(&self) -> String {
        self.timezone.read().unwrap().clone()
    }

    pub fn update_base_currency(&self, new_currency: String) {
        *self.base_currency.write().unwrap() = new_currency;
    }

    pub fn update_timezone(&self, new_timezone: String) {
        *self.timezone.write().unwrap() = new_timezone;
    }

    pub fn settings_service(&self) -> Arc<dyn settings::SettingsServiceTrait> {
        Arc::clone(&self.settings_service)
    }

    pub fn account_service(&self) -> Arc<dyn accounts::AccountServiceTrait> {
        Arc::clone(&self.account_service)
    }

    pub fn activity_service(&self) -> Arc<dyn activities::ActivityServiceTrait> {
        Arc::clone(&self.activity_service)
    }

    pub fn asset_service(&self) -> Arc<dyn assets::AssetServiceTrait> {
        Arc::clone(&self.asset_service)
    }

    pub fn goal_service(&self) -> Arc<dyn goals::GoalServiceTrait> {
        Arc::clone(&self.goal_service)
    }

    pub fn quote_service(&self) -> Arc<dyn quotes::QuoteServiceTrait> {
        Arc::clone(&self.quote_service)
    }

    pub fn news_service(&self) -> Arc<news::NewsService> {
        Arc::clone(&self.news_service)
    }

    pub fn limits_service(&self) -> Arc<dyn limits::ContributionLimitServiceTrait> {
        Arc::clone(&self.limits_service)
    }

    pub fn fx_service(&self) -> Arc<dyn fx::FxServiceTrait> {
        Arc::clone(&self.fx_service)
    }

    pub fn performance_service(&self) -> Arc<dyn portfolio::performance::PerformanceServiceTrait> {
        Arc::clone(&self.performance_service)
    }

    pub fn income_service(&self) -> Arc<dyn portfolio::income::IncomeServiceTrait> {
        Arc::clone(&self.income_service)
    }

    pub fn snapshot_service(&self) -> Arc<dyn portfolio::snapshot::SnapshotServiceTrait> {
        Arc::clone(&self.snapshot_service)
    }

    pub fn snapshot_repository(&self) -> Arc<SnapshotRepository> {
        Arc::clone(&self.snapshot_repository)
    }

    pub fn holdings_service(&self) -> Arc<dyn portfolio::holdings::HoldingsServiceTrait> {
        Arc::clone(&self.holdings_service)
    }

    pub fn app_sync_repository(&self) -> Arc<AppSyncRepository> {
        Arc::clone(&self.app_sync_repository)
    }

    pub fn allocation_service(&self) -> Arc<dyn portfolio::allocation::AllocationServiceTrait> {
        Arc::clone(&self.allocation_service)
    }

    pub fn valuation_service(&self) -> Arc<dyn portfolio::valuation::ValuationServiceTrait> {
        Arc::clone(&self.valuation_service)
    }

    pub fn sync_service(&self) -> Arc<dyn BrokerSyncServiceTrait> {
        Arc::clone(&self.sync_service)
    }

    pub fn zakat_service(&self) -> Arc<dyn mizan_zakat::ZakatServiceTrait> {
        Arc::clone(&self.zakat_service)
    }

    pub fn net_worth_service(&self) -> Arc<dyn portfolio::net_worth::NetWorthServiceTrait> {
        Arc::clone(&self.net_worth_service)
    }

    pub fn alternative_asset_service(&self) -> Arc<dyn AlternativeAssetServiceTrait> {
        Arc::clone(&self.alternative_asset_service)
    }

    pub fn taxonomy_service(&self) -> Arc<dyn taxonomies::TaxonomyServiceTrait> {
        Arc::clone(&self.taxonomy_service)
    }

    pub fn connect_service(&self) -> Arc<ConnectService> {
        Arc::clone(&self.connect_service)
    }

    pub fn ai_provider_service(&self) -> Arc<dyn AiProviderServiceTrait> {
        Arc::clone(&self.ai_provider_service)
    }

    pub fn ai_chat_service(&self) -> Arc<ChatService<TauriAiEnvironment>> {
        Arc::clone(&self.ai_chat_service)
    }

    pub fn device_enroll_service(&self) -> Arc<DeviceEnrollService> {
        Arc::clone(&self.device_enroll_service)
    }

    pub fn device_sync_runtime(&self) -> Arc<DeviceSyncRuntimeState> {
        Arc::clone(&self.device_sync_runtime)
    }

    pub fn health_service(&self) -> Arc<health::HealthService> {
        Arc::clone(&self.health_service)
    }

    // ─── v3.1 foundation accessors ────────────────────────────────────────
    // ai_safety accessor is `pub(crate)` + allow(dead_code) — used by
    // upcoming support-bundle endpoints surfacing the audit trail.
    #[allow(dead_code)]
    pub fn ai_safety(&self) -> Arc<AiSafetyRuntime> {
        Arc::clone(&self.ai_safety)
    }
    pub fn sync_ledger(&self) -> Arc<dyn SyncRunLedger> {
        Arc::clone(&self.sync_ledger)
    }
    pub fn net_worth_snapshot_service(&self) -> Arc<dyn NetWorthSnapshotService> {
        Arc::clone(&self.net_worth_snapshot_service)
    }
    pub fn daily_brief_service(&self) -> Arc<dyn DailyBriefService> {
        Arc::clone(&self.daily_brief_service)
    }
    pub fn notification_service(&self) -> Arc<dyn NotificationService> {
        Arc::clone(&self.notification_service)
    }
    pub fn truth_ledger(&self) -> Arc<dyn TruthLedger> {
        Arc::clone(&self.truth_ledger)
    }
    pub fn truth_ledger_retry_queue(&self) -> Arc<SqliteTruthLedgerRetryQueue> {
        Arc::clone(&self.truth_ledger_retry_queue)
    }
}

/// Construct the §v3.1 foundation services backed by SQLite (production).
/// Each row survives app restarts + powers the support bundle / audit
/// surfaces. The retry queue is returned alongside so the boot path can
/// drain stale appends before they go stale.
pub fn build_v31_foundation_defaults(pool: Arc<DbPool>, writer: WriteHandle) -> V31Foundations {
    let sync_ledger: Arc<dyn SyncRunLedger> =
        Arc::new(SqliteSyncRunLedger::new(Arc::clone(&pool), writer.clone()));
    let nw_snapshot: Arc<dyn NetWorthSnapshotService> = Arc::new(
        SqliteNetWorthSnapshotService::new(Arc::clone(&pool), writer.clone()),
    );
    let daily_brief: Arc<dyn DailyBriefService> = Arc::new(SqliteDailyBriefService::new(
        Arc::clone(&pool),
        writer.clone(),
    ));
    let notifications: Arc<dyn NotificationService> = Arc::new(SqliteNotificationService::new(
        Arc::clone(&pool),
        writer.clone(),
    ));
    let truth_ledger: Arc<dyn TruthLedger> =
        Arc::new(SqliteTruthLedger::new(Arc::clone(&pool), writer.clone()));
    let retry_queue = Arc::new(SqliteTruthLedgerRetryQueue::new(pool, writer));
    V31Foundations {
        ai_safety: Arc::new(AiSafetyRuntime::new()),
        sync_ledger,
        nw_snapshot,
        daily_brief,
        notifications,
        truth_ledger,
        retry_queue,
    }
}

/// All five §v3.1 foundation services + the truth-ledger retry queue.
/// Returned as a struct (rather than a six-tuple) so callers can
/// destructure by name + so adding a seventh foundation later doesn't
/// silently shift every existing tuple index at the call site.
pub struct V31Foundations {
    pub ai_safety: Arc<AiSafetyRuntime>,
    pub sync_ledger: Arc<dyn SyncRunLedger>,
    pub nw_snapshot: Arc<dyn NetWorthSnapshotService>,
    pub daily_brief: Arc<dyn DailyBriefService>,
    pub notifications: Arc<dyn NotificationService>,
    pub truth_ledger: Arc<dyn TruthLedger>,
    pub retry_queue: Arc<SqliteTruthLedgerRetryQueue>,
}
