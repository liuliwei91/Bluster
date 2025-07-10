use sqlx::SqlitePool;
use log::error;
use serde::{Serialize, Deserialize};
// 使用String存储时间简化处理
use bcrypt::{hash, verify, DEFAULT_COST};

use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Article {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub author_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    // Create absolute path to database file
    let db_path = "sqlite:blog.db?mode=rwc";
    
    // Try to connect to database (will create if not exists)
    let pool = match SqlitePool::connect(db_path).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(e);
        }
    };
    
    // Create tables
    if let Err(e) = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#
    ).execute(&pool).await {
        error!("Failed to create users table: {}", e);
        return Err(e);
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            author_id INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(author_id) REFERENCES users(id)
        )
        "#
    ).execute(&pool).await?;

    // Check if admin user exists, if not create one
    let admin_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE username = 'admin')"
    )
    .fetch_one(&pool)
    .await?;

    if !admin_exists {
        let _ = create_user(&pool, "admin", "admin").await?;
        log::info!("Default admin user created with password 'admin'");
    }

    Ok(pool)
}

pub async fn create_user(pool: &SqlitePool, username: &str, password: &str) -> Result<User, sqlx::Error> {
    let password_hash = hash(password, DEFAULT_COST).map_err(|e| {
        sqlx::Error::Decode(Box::new(e))
    })?;
    let user_id = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash) VALUES (?, ?) RETURNING id"
    )
    .bind(username)
    .bind(&password_hash)
    .fetch_one(pool)
    .await?;

    Ok(User {
        id: user_id,
        username: username.to_string(),
        password_hash,
        created_at: chrono::Local::now().to_string(),
    })
}

pub async fn verify_user(pool: &SqlitePool, username: &str, password: &str) -> Result<User, sqlx::Error> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_one(pool)
        .await?;

    if verify(password, &user.password_hash).map_err(|e| {
        sqlx::Error::Decode(Box::new(e))
    })? {
        Ok(user)
    } else {
        Err(sqlx::Error::RowNotFound)
    }
}