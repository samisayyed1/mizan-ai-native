//! Legacy no-op handlers kept for old imports.

use crate::error::AppError;

/// `GET /api/v1/subscription/plans` — auth-optional stub.
///
/// Reachable with or without a bearer token; the desktop calls this path
/// from both `get_subscription_plans` and `fetch_subscription_plans_public`.
pub async fn subscription_plans() -> Result<(), AppError> {
    Err(AppError::not_implemented(
        "Mizan Connect plans & billing is coming in a future release.",
    ))
}
