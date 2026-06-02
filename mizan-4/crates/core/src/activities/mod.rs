//! Activities module - domain models, services, and traits.

mod activities_constants;
mod activities_errors;
mod activities_model;
mod activities_service;
mod activities_traits;
mod compiler;
// Track H PR-H3.e — csv_parser moved out of mizan-core into the
// `mizan-csv-import` crate. Re-exported below from mizan_csv_import so
// existing `mizan_core::activities::ParseConfig` consumers keep working.
mod fd_scheduler;
mod idempotency;
mod import_run_model;
mod rsp_scheduler;

#[cfg(test)]
mod activities_service_tests;

#[cfg(test)]
mod activities_model_tests;

pub use activities_constants::*;
pub use activities_errors::ActivityError;
pub use activities_model::import_type;
pub use activities_model::{
    into_field_mapping_values, normalize_context_kind_value, parse_decimal_string_tolerant,
    Activity, ActivityBulkIdentifierMapping, ActivityBulkMutationError,
    ActivityBulkMutationRequest, ActivityBulkMutationResult, ActivityDetails, ActivityImport,
    ActivitySearchResponse, ActivitySearchResponseMeta, ActivityStatus, ActivityType,
    ActivityUpdate, ActivityUpsert, AssetResolutionInput, BrokerActivityProfileConfig,
    BrokerProfileScope, BrokerSyncProfileData, BulkUpsertResult, FieldMappingValue,
    ImportActivitiesResult, ImportActivitiesSummary, ImportAssetCandidate, ImportAssetPreviewItem,
    ImportAssetPreviewStatus, ImportMapping, ImportMappingData, ImportTemplate, ImportTemplateData,
    ImportTemplateScope, IncomeData, NewActivity, PrepareActivitiesResult,
    SaveBrokerSyncProfileRulesRequest, Sort, TemplateKind,
};
pub use activities_service::ActivityService;
pub use activities_traits::{ActivityRepositoryTrait, ActivityServiceTrait};
pub use compiler::{ActivityCompiler, DefaultActivityCompiler};
// Track H PR-H3.e — re-export the public CSV parser surface from
// mizan_csv_import. Same path (`mizan_core::activities::{parse_csv,
// ParseConfig, ...}`) so downstream consumers compile unchanged.
pub use fd_scheduler::{generate_fd_schedule, FdParams, FdPaymentFrequency, FdSchedulerError};
pub use idempotency::{
    compute_activity_idempotency_key, compute_idempotency_key, generate_manual_idempotency_key,
};
pub use import_run_model::{
    ImportRun, ImportRunMode, ImportRunRepositoryTrait, ImportRunStatus, ImportRunSummary,
    ImportRunType, ReviewMode,
};
pub use mizan_csv_import::{parse_csv, CsvImportError, ParseConfig, ParseError, ParsedCsvResult};
pub use rsp_scheduler::{generate_rsp_schedule, RspFrequency, RspParams, RspSchedulerError};
