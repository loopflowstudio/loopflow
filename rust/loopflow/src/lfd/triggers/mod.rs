//! What's left of the trigger organs: token refresh. The activation queue,
//! loop ticker, watch/cron/ci-failure pollers, repair chain, and recovery
//! loop died in the collapse's organ cut — webhooks speak `lf chat` /
//! `lf op queue reconcile`, and cron lives in the wave's resident flowloop.

mod token_refresh;

pub use token_refresh::spawn_token_refresh;
