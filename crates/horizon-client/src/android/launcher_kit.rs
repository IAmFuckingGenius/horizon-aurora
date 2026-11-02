//! The launcher's «Горизонт» look lives in `horizon_client::pixel_kit`, shared with
//! the in-game touch layout editor; the launcher's widgets carry its page actions.
//! Geometry follows `.local/mockups/variant-c-spec.md` (a local design note).
pub(super) use horizon_client::pixel_kit::*;

/// One frame of launcher drawing with the launcher's page actions.
pub(super) type Kit<'k, 'a> = horizon_client::pixel_kit::Kit<'k, 'a, super::pages::Action>;
