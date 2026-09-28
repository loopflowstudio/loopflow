pub mod build_info;
pub mod chat;
pub mod child;
pub mod controller;
pub mod durable;
pub mod engine;
pub mod exec;
pub mod harness;
pub mod id;
pub mod journal;
pub mod lf;
pub mod machine_install;
// Build-time parsing lives here so its golden tests compile against the exact parser.
#[allow(dead_code)]
pub(crate) mod migration_drafts;
pub mod ops;
pub mod planning;
pub mod pm;
pub mod pr_landing;
pub mod profile;
pub(crate) mod promotion_lock;
pub mod provider_account;
pub mod provider_auth;
pub mod repo;
pub mod repository;
pub(crate) mod run_record;
pub mod security;
pub mod session;
pub mod store;
pub mod subscription;
pub mod trace;
pub mod work;

#[cfg(test)]
#[path = "../tests/support/ambient.rs"]
pub(crate) mod test_ambient;
