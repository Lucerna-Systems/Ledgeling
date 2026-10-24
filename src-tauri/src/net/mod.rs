//! Optional online features, behind one gate. Ledgeling makes no network call
//! unless the user has switched on the scope that call belongs to, and only
//! [`client`] may make one. See `docs/privacy.md`.
pub mod client;
pub mod consent;
pub mod scope;

pub use client::{Net, NetError, Request, Response, ScopeStatus};
pub use consent::{Consent, Mode};
pub use scope::Scope;

#[cfg(test)]
mod tests;
