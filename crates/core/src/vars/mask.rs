//! Redacting resolved secret values from *display* surfaces.
//!
//! Secrets are tracked by name in the [`VarStack`], resolved to real values for
//! the request that goes on the wire, but must never surface in the UI: the
//! script console, echoed responses, script errors, hover previews. This module
//! is the one-way masking layer for those surfaces — the bytes sent to the
//! network are always the real thing.

use super::interpolate::{Interpolated, interpolate};
use super::scope::VarStack;

/// What a redacted secret renders as.
pub const SECRET_MASK: &str = "••••••";

/// Replace every occurrence of a known secret value in `text` with
/// [`SECRET_MASK`]. `secret_values` should be longest-first so a secret that is
/// a substring of another is masked as part of the longer match first (see
/// [`VarStack::secret_values`]).
pub fn mask_secrets(text: &str, secret_values: &[String]) -> String {
    let mut out = text.to_string();
    for secret in secret_values {
        if !secret.is_empty() {
            out = out.replace(secret.as_str(), SECRET_MASK);
        }
    }
    out
}

/// Like [`interpolate`], but any resolved secret value in the result is masked.
/// For preview/echo surfaces (e.g. a `{{token}}` hover tooltip) that must show
/// the *shape* of an interpolated string without leaking the secret itself.
pub fn interpolate_masked(text: &str, stack: &VarStack) -> Interpolated {
    let Interpolated { text, warnings } = interpolate(text, stack);
    Interpolated {
        text: mask_secrets(&text, &stack.secret_values()),
        warnings,
    }
}
