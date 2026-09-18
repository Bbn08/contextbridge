use contextbridge::LocalSqliteStorage;

#[tokio::test]
async fn local_sqlite_satisfies_storage_contract() {
    let storage = LocalSqliteStorage::connect("sqlite::memory:")
        .await
        .unwrap();
    contextbridge::test_support::storage_contract(&storage)
        .await
        .unwrap();
}
