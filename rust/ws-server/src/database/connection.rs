use diesel::prelude::*;
use anyhow::Result;

pub fn establish_connection() -> Result<PgConnection> {
    let db_host = std::env::var("POSTGRES_HOST").unwrap_or("localhost".to_string());
    let db_port = std::env::var("POSTGRES_PORT").unwrap_or("5432".to_string());
    let db_user = std::env::var("POSTGRES_USER").expect("No env POSTGRES_USER was provided");
    let db_pass = std::env::var("POSTGRES_PASSWORD").expect("No env POSTGRES_PASSWORD was provided");
    let db_name = std::env::var("POSTGRES_DB").unwrap_or("tuidb".to_string());

    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        db_user, db_pass, db_host, db_port, db_name
    );

    match PgConnection::establish(&database_url) {
        Ok(conn) => Ok(conn),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}
