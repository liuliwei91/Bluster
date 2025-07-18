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
    pub security_question: Option<String>,
    #[serde(skip_serializing)]
    pub security_answer_hash: Option<String>,
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

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct About {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub updated_at: String,
}

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    // Create absolute path to database file
    let db_path = "sqlite:./data/blog.db?mode=rwc";

    
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
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            security_question TEXT,
            security_answer_hash TEXT
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

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS about (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
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

    // Check if about content exists, if not create default one
    let about_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM about LIMIT 1)"
    )
    .fetch_one(&pool)
    .await?;

    if !about_exists {
        sqlx::query(
            "INSERT INTO about (title, content) VALUES (?, ?)"
        )
        .bind("About My Blog")
        .bind("This is a blog system built with Rust language and the Actix-web framework. This blog is a project for me to learn Rust, and I hope to gain a deeper understanding of Rust's web development capabilities through this project.")
        .execute(&pool)
        .await?;
        log::info!("Default about content created");
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
        security_question: None,
        security_answer_hash: None,
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

// 更新用户密码
pub async fn update_user_password(pool: &SqlitePool, user_id: i64, new_password: &str) -> Result<(), sqlx::Error> {
    let password_hash = hash(new_password, DEFAULT_COST).map_err(|e| {
        sqlx::Error::Decode(Box::new(e))
    })?;
    
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(password_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    
    Ok(())
}

// 设置安全问题
pub async fn set_security_question(pool: &SqlitePool, user_id: i64, question: &str, answer: &str) -> Result<(), sqlx::Error> {
    let answer_hash = hash(answer, DEFAULT_COST).map_err(|e| {
        sqlx::Error::Decode(Box::new(e))
    })?;
    
    sqlx::query("UPDATE users SET security_question = ?, security_answer_hash = ? WHERE id = ?")
        .bind(question)
        .bind(answer_hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    
    Ok(())
}

// 验证安全问题答案
pub async fn verify_security_answer(pool: &SqlitePool, username: &str, answer: &str) -> Result<User, sqlx::Error> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(username)
        .fetch_one(pool)
        .await?;
    
    if let Some(answer_hash) = &user.security_answer_hash {
        if verify(answer, answer_hash).map_err(|e| {
            sqlx::Error::Decode(Box::new(e))
        })? {
            Ok(user)
        } else {
            Err(sqlx::Error::RowNotFound)
        }
    } else {
        Err(sqlx::Error::RowNotFound)
    }
}

// 通过用户名重置密码
pub async fn reset_password_by_username(pool: &SqlitePool, username: &str, new_password: &str) -> Result<(), sqlx::Error> {
    let password_hash = hash(new_password, DEFAULT_COST).map_err(|e| {
        sqlx::Error::Decode(Box::new(e))
    })?;
    
    sqlx::query("UPDATE users SET password_hash = ? WHERE username = ?")
        .bind(password_hash)
        .bind(username)
        .execute(pool)
        .await?;
    
    Ok(())
}