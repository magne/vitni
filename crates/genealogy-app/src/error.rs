//! [`AppError`] — the single error type the use-cases return to a frontend.
//!
//! It collapses configuration, workspace I/O, the engine-neutral store ([`genealogy_db::DbError`]),
//! and domain rejections ([`PersonError`]) into one enum so a frontend can render a message and pick
//! an exit status without knowing which layer failed. Domain rejections are kept distinct
//! ([`AppError::Domain`]) because they are the operator's fault (a 4xx), not the system's.

use genealogy_db::DbError;

use crate::aggregates::for_each_aggregate;

/// Generates [`AppError`] from the canonical registry: the fixed infrastructure variants, then one
/// `<Name>NotFound(String)` per aggregate, then one domain-rejection wrapper per aggregate (with a
/// `#[from]` so a use-case can `?`-propagate a core error). Person's wrapper keeps its historical
/// name `Domain`; the rest are `<Name>Domain`.
macro_rules! app_error {
    ($(($snake:ident, $noun:literal, $Id:ty, $id_fn:ident, $Err:ty, $domain:ident, $nf:ident, $msg:literal)),+ $(,)?) => {
        /// A failure surfaced by a `genealogy-app` use-case.
        #[derive(Debug, thiserror::Error)]
        pub enum AppError {
            /// Configuration could not be loaded, bootstrapped, written, or parsed.
            #[error("configuration error: {0}")]
            Config(String),
            /// A workspace directory or its manifest could not be created/read.
            #[error("workspace error: {0}")]
            Workspace(String),
            /// The engine-neutral store failed (infrastructure).
            #[error(transparent)]
            Db(#[from] DbError),
            /// A caller-supplied `human_id` is already in use.
            #[error("human_id {0:?} is already taken")]
            HumanIdTaken(String),
            $(
                #[doc = concat!("No ", $noun, " exists with the given identifier.")]
                #[error($msg)]
                $nf(String),
            )+
            $(
                #[doc = concat!("The command was rejected by a ", $noun, " domain rule (the operator's input is invalid).")]
                #[error("rejected: {0}")]
                $domain(#[from] $Err),
            )+
        }
    };
}

for_each_aggregate!(app_error);
