use pp_database::auth::AuthStore;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

#[sqlx::test]
async fn setup_session_and_revocation_are_persistent(pool: PgPool) {
    let store = AuthStore::new(pool.clone());
    let now = OffsetDateTime::now_utc();

    assert!(store.setup_available().await.unwrap());

    let identity = store
        .create_account(
            "person@example.test",
            "hash",
            &[1; 32],
            now + Duration::days(30),
        )
        .await
        .unwrap()
        .unwrap();

    assert!(!store.setup_available().await.unwrap());
    assert!(
        store
            .create_account(
                "other@example.test",
                "hash",
                &[2; 32],
                now + Duration::days(30)
            )
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.authenticate(&[1; 32], now).await.unwrap(),
        Some(identity)
    );
    assert!(
        store
            .authenticate(&[1; 32], now + Duration::days(30))
            .await
            .unwrap()
            .is_none()
    );

    store
        .renew(&[1; 32], now, now + Duration::days(40))
        .await
        .unwrap()
        .unwrap();

    assert!(
        store
            .authenticate(&[1; 32], now + Duration::days(31))
            .await
            .unwrap()
            .is_some()
    );

    store.revoke(&[1; 32]).await.unwrap();

    assert!(
        store
            .renew(&[1; 32], now, now + Duration::days(50))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        AuthStore::new(pool)
            .authenticate(&[1; 32], now)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test]
async fn concurrent_setup_leaves_one_workspace_and_account(pool: PgPool) {
    let a = AuthStore::new(pool.clone());
    let b = a.clone();
    let expiry = OffsetDateTime::now_utc() + Duration::days(30);
    let (a, b) = tokio::join!(
        a.create_account("a@example.test", "hash", &[1; 32], expiry),
        b.create_account("b@example.test", "hash", &[2; 32], expiry)
    );

    assert_eq!(
        usize::from(a.unwrap().is_some()) + usize::from(b.unwrap().is_some()),
        1
    );

    for table in ["accounts", "workspaces", "sessions"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}

#[sqlx::test]
async fn renewal_and_logout_cannot_resurrect_session(pool: PgPool) {
    let store = AuthStore::new(pool);
    let now = OffsetDateTime::now_utc();
    store
        .create_account("a@example.test", "hash", &[1; 32], now + Duration::days(1))
        .await
        .unwrap();

    let (renew, revoke) = tokio::join!(
        store.renew(&[1; 32], now, now + Duration::days(2)),
        store.revoke(&[1; 32])
    );

    renew.unwrap();
    revoke.unwrap();

    assert!(store.authenticate(&[1; 32], now).await.unwrap().is_none());
}

#[sqlx::test]
async fn replacing_one_login_does_not_revoke_another(pool: PgPool) {
    let store = AuthStore::new(pool);
    let now = OffsetDateTime::now_utc();
    let id = store
        .create_account("a@example.test", "hash", &[1; 32], now + Duration::days(1))
        .await
        .unwrap()
        .unwrap();

    store
        .create_session(id.account_id.0, &[2; 32], now + Duration::days(1), None)
        .await
        .unwrap();
    store
        .create_session(
            id.account_id.0,
            &[3; 32],
            now + Duration::days(1),
            Some(&[1; 32]),
        )
        .await
        .unwrap();

    assert!(store.authenticate(&[1; 32], now).await.unwrap().is_none());
    assert!(store.authenticate(&[2; 32], now).await.unwrap().is_some());
    assert!(store.authenticate(&[3; 32], now).await.unwrap().is_some());
}
