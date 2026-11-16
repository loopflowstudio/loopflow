//! Held provider and login fixtures shared by Graph and Task lifecycle checks.
use crate::store::{CredentialState, ProviderAccount, ProviderAccountId, RoutingState};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
pub const HOLDING_CLAUDE: &str = r#"#!/usr/bin/python3
import json, os, sys, time
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = os.environ['LF_HOME']
streamed = '--input-format' in sys.argv
message = json.loads(sys.stdin.readline()) if streamed else None
with open(os.path.join(home, 'ready'), 'w') as ready:
    ready.write('ready')
if streamed:
    with open(os.path.join(home, 'stream-ready'), 'w') as ready:
        ready.write('ready')
while not os.path.exists(os.path.join(home, 'release')):
    time.sleep(0.02)
if not streamed:
    sys.exit(0)
message['session_id'] = 'stand-in'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'stand-in'}), flush=True)
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': 'Held.'}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'stand-in-result', 'session_id': 'stand-in', 'subtype': 'success', 'is_error': False, 'result': 'Held.'}), flush=True)
for line in sys.stdin:
    pass
"#;

pub fn connect_fixture_claude(home: &Path, store: &crate::store::Store) {
    connect_claude_account(home, store, "fixture");
}

/// Connect a managed Claude account `name`, signed in as `name@example.com`.
pub fn connect_claude_account(home: &Path, store: &crate::store::Store, name: &str) {
    let account_home = home.join("accounts/claude").join(name);
    fs::create_dir_all(&account_home).unwrap();
    let credential = format!(
        r#"{{"claudeAiOauth":{{"accessToken":"fixture-{name}","expiresAt":4102444800000}}}}"#
    );
    fs::write(account_home.join(".credentials.json"), &credential).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(store.upsert_provider_account(&ProviderAccount {
            provider: "claude".into(),
            account_id: ProviderAccountId::parse(name).unwrap(),
            home: Some(account_home),
            login_email: Some(
                crate::profile::EmailAddress::parse(&format!("{name}@example.com")).unwrap(),
            ),
            observed_email: Some(format!("{name}@example.com")),
            observed_subject: Some(name.into()),
            observed_credential_digest: Some(format!(
                "{:x}",
                Sha256::digest(credential.as_bytes())
            )),
            observed_plan: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        }))
        .unwrap();
}
