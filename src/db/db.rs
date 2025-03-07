use crate::env::ENV_CONFIG;
use sqlx::{
    postgres::PgPoolOptions, Connection, Executor, PgConnection, PgPool,
};
pub async fn get_db_pool(name: &str, max_connections: u32) -> PgPool {
    let name_query = format!("SET application_name = '{}'; ", name);
    PgPoolOptions::new()
        .after_connect(move |conn, _meta| {
            let name_query = name_query.clone();
            Box::pin(async move {
                conn.execute(name_query.as_ref()).await?;
                Ok(())
            })
        })
        .max_connections(max_connections)
        .max_lifetime(std::time::Duration::from_secs(20))
        .connect(&ENV_CONFIG.db_url)
        .await
        .expect("expect DB to be available to connect")
}

pub async fn get_db_connection(name: &str) -> PgConnection {
    let mut conn = PgConnection::connect(ENV_CONFIG.db_url.as_str())
        .await
        .expect("expect DB to be available to connect");

    let query = format!("SET application_name = '{}';", name);
    sqlx::query(&query).execute(&mut conn).await.unwrap();
    conn
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use async_trait::async_trait;
    use nanoid::nanoid;
    use sqlx::postgres::PgPoolOptions;
    use test_context::AsyncTestContext;
    use tracing::info;

    const ALPHABET: [char; 16] = [
        '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', 'a', 'b', 'c', 'd',
        'e', 'f',
    ];

    pub async fn get_test_db_connection() -> sqlx::PgConnection {
        if !ENV_CONFIG.test_db_url.contains("testdb") {
            panic!("tried to run tests against db that is not 'testdb")
        }
        get_db_connection("testing").await
    }

    pub struct TestDb {
        pub pool: PgPool,
        name: String,
    }

    #[async_trait]
    impl AsyncTestContext for TestDb {
        async fn setup() -> Self {
            TestDb::new().await
        }
    }

    impl TestDb {
        pub async fn new() -> Self {
            let name = format!("testdb_{}", nanoid!(15, &ALPHABET));

            let mut connection = get_test_db_connection().await;
            println!(
                "create test database for testing with the db name as {name}"
            );
            sqlx::query(&format!("CREATE DATABASE {name}"))
                .execute(&mut connection)
                .await
                .unwrap();

            let pool = PgPoolOptions::new()
                .max_connections(5)
                .max_lifetime(std::time::Duration::from_secs(20))
                .connect(&ENV_CONFIG.db_url.replace("testdb", &name))
                .await
                .unwrap();

            // sqlx::migrate!("../migrations").run(&pool).await.unwrap();

            Self { pool, name }
        }
    }
}
