use std::fs;
pub fn write_claude_identity(account: &mut crate::store::ProviderAccount) {
    let home = account.home.as_ref().unwrap();
    fs::create_dir_all(home).unwrap();
    let raw = serde_json::json!({"claudeAiOauth":{"accessToken":format!("fixture-{}", account.account_id),"expiresAt":4102444800000i64}}).to_string();
    fs::write(home.join(".credentials.json"), &raw).unwrap();
    account.observed_email = Some(account.login_email.as_ref().unwrap().to_string());
    account.observed_subject = Some(account.account_id.to_string());
    account.observed_credential_digest =
        Some(crate::provider_account::identity::credential_digest(&raw));
}
