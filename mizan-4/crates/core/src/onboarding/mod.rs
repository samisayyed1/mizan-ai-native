//! Onboarding-time side effects.
//!
//! Functions in this module run once when the user finishes the 3-step
//! onboarding flow (settings flip `onboarding_completed = true`). The
//! goal — per Feroz on 25 May 2026 — is "edit-first UX over blank-form
//! UX": users see populated examples they tweak into their reality
//! rather than staring at an empty dashboard.

pub mod seed_examples;

pub use seed_examples::{seed_example_liabilities, EXAMPLE_NAME_PREFIX};
