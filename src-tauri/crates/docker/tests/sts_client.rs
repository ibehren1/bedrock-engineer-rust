//! Port of `src/test/sandbox/sts.client.test.ts`: an STS `GetCallerIdentity` smoke check
//! with the default credential chain. It was `test.skip` in TS; here it is `#[ignore]`
//! and needs real AWS credentials.
//!
//! Run with: `cargo test -p docker --test sts_client -- --ignored`

async fn get_account_id() -> Option<String> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_sts::Client::new(&config);
    let response = client
        .get_caller_identity()
        .send()
        .await
        .expect("GetCallerIdentity");
    println!("{response:?}");
    response.account().map(str::to_string)
}

#[tokio::test]
#[ignore = "requires AWS credentials"]
async fn get_account_id_test() {
    let account_id = get_account_id().await;
    println!("{account_id:?}");
    assert!(account_id.is_some());
}
