use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use actix_web::middleware::Logger;
use actix_session::{Session, SessionMiddleware};
use log::{info, error};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::models::{init_db, verify_user};
use tera::{Tera, Context};

mod models;

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct ArticleForm {
    title: String,
    content: String,
}

// Blog post structure
#[derive(Serialize, Deserialize, Clone)]
struct Post {
    id: u32,
    title: String,
    summary: String,
    content: String,
    date: String,
}

// Application state, storing blog posts
struct AppState {
    template: Tera,
}

async fn index(
    data: web::Data<AppState>,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    let mut ctx = Context::new();
    
    match sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, title, content, created_at FROM articles ORDER BY created_at DESC"
    )
    .fetch_all(_pool.get_ref())
    .await {
        Ok(articles) => {
            let posts: Vec<Post> = articles.into_iter().map(|(id, title, content, date)| {
                Post {
                    id: id as u32,
                    title,
                    summary: content.chars().take(100).collect(),
                    content,
                    date
                }
            }).collect();
            ctx.insert("posts", &posts);
            match data.template.render("index.html", &ctx) {
                Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
                Err(e) => {
                    error!("Template rendering error: {}", e);
                    HttpResponse::InternalServerError().body("Template rendering error")
                }
            }
        },
        Err(e) => {
            error!("Failed to fetch articles: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn post_detail(
    data: web::Data<AppState>,
    path: web::Path<i64>,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    let post_id = path.into_inner();
    let mut ctx = Context::new();
    
    match sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, title, content, created_at FROM articles WHERE id = ?"
    )
    .bind(post_id)
    .fetch_one(_pool.get_ref())
    .await {
        Ok((id, title, content, created_at)) => {
            let post = Post {
                id: id as u32,
                title,
                summary: content.chars().take(100).collect(),
                content,
                date: created_at
            };
            ctx.insert("post", &post);
            match data.template.render("post.html", &ctx) {
                Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
                Err(e) => {
                    error!("Template rendering error: {}", e);
                    HttpResponse::InternalServerError().body("Template rendering error")
                }
            }
        },
        Err(e) => {
            error!("Failed to fetch article: {}", e);
            HttpResponse::NotFound().finish()
        }
    }
}

async fn about(data: web::Data<AppState>) -> impl Responder {
    match data.template.render("about.html", &Context::new()) {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => {
            error!("Template rendering error: {}", e);
            HttpResponse::InternalServerError().body("Template rendering error")
        }
    }
}

async fn login_page(data: web::Data<AppState>) -> impl Responder {
    match data.template.render("login.html", &Context::new()) {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => {
            error!("Template rendering error: {}", e);
            HttpResponse::InternalServerError().body("Template rendering error")
        }
    }
}

async fn admin_dashboard(
    data: web::Data<AppState>,
    session: Session,
    _pool: web::Data<SqlitePool>
) -> actix_web::Result<HttpResponse> {
    // 检查session中的登录状态
    if let Some(_) = session.get::<String>("username")? {
        // 已登录，显示dashboard
        let ctx = Context::new();
        match data.template.render("admin/dashboard.html", &ctx) {
            Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
            Err(e) => {
                error!("Template rendering error: {}", e);
                Ok(HttpResponse::InternalServerError().body("Template rendering error"))
            }
        }
    } else {
        // 未登录，重定向到登录页面
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn admin_articles(pool: web::Data<SqlitePool>) -> impl Responder {
    match sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, title, content FROM articles ORDER BY created_at DESC"
    )
    .fetch_all(pool.get_ref())
    .await {
        Ok(articles) => HttpResponse::Ok().json(articles),
        Err(e) => {
            error!("Failed to fetch articles: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn admin_create_article(
    form: web::Form<ArticleForm>,
    pool: web::Data<SqlitePool>,
) -> impl Responder {
    match sqlx::query(
        "INSERT INTO articles (title, content, created_at, updated_at) VALUES (?, ?, datetime('now'), datetime('now'))"
    )
    .bind(&form.title)
    .bind(&form.content)
    .execute(pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Found().append_header(("Location", "/admin")).finish(),
        Err(e) => {
            error!("Failed to create article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn login(
    form: web::Form<LoginForm>,
    pool: web::Data<SqlitePool>,
    session: Session,
) -> impl Responder {
    match verify_user(&pool, &form.username, &form.password).await {
        Ok(_) => {
            // 登录成功，设置session
            if let Err(e) = session.insert("username", &form.username) {
                error!("Failed to set session: {}", e);
                return HttpResponse::InternalServerError().finish();
            }
            HttpResponse::Found().append_header(("Location", "/admin")).finish()
        },
        Err(_) => HttpResponse::Unauthorized().body("Invalid credentials")
    }
}

async fn logout(session: Session) -> impl Responder {
    // 清除session
    session.clear();
    HttpResponse::Found()
        .append_header(("Location", "/login"))
        .finish()
}

async fn get_articles(pool: web::Data<SqlitePool>) -> impl Responder {
    match sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, title, content FROM articles ORDER BY created_at DESC"
    )
    .fetch_all(pool.get_ref())
    .await {
        Ok(articles) => HttpResponse::Ok().json(articles),
        Err(e) => {
            error!("Failed to fetch articles: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn get_article(
    pool: web::Data<SqlitePool>,
    path: web::Path<i64>
) -> impl Responder {
    let article_id = path.into_inner();
    match sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, title, content FROM articles WHERE id = ?"
    )
    .bind(article_id)
    .fetch_one(pool.get_ref())
    .await {
        Ok(article) => HttpResponse::Ok().json(article),
        Err(e) => {
            error!("Failed to fetch article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn update_article(
    path: web::Path<i64>,
    form: web::Form<ArticleForm>,
    pool: web::Data<SqlitePool>,
) -> impl Responder {
    let article_id = path.into_inner();
    match sqlx::query(
        "UPDATE articles SET title = ?, content = ?, updated_at = datetime('now') WHERE id = ?"
    )
    .bind(&form.title)
    .bind(&form.content)
    .bind(article_id)
    .execute(pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Ok().json("Article updated successfully"),
        Err(e) => {
            error!("Failed to update article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn delete_article(
    path: web::Path<i64>,
    pool: web::Data<SqlitePool>,
) -> impl Responder {
    let article_id = path.into_inner();
    match sqlx::query(
        "DELETE FROM articles WHERE id = ?"
    )
    .bind(article_id)
    .execute(pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Ok().json("Article deleted successfully"),
        Err(e) => {
            error!("Failed to delete article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn create_article(
    form: web::Form<ArticleForm>,
    pool: web::Data<SqlitePool>,
) -> impl Responder {
    match sqlx::query(
        "INSERT INTO articles (title, content, created_at, updated_at) VALUES (?, ?, datetime('now'), datetime('now'))"
    )
    .bind(&form.title)
    .bind(&form.content)
    .execute(pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Ok().json("Article created successfully"),
        Err(e) => {
            error!("Failed to create article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    info!("Starting blog server...");
    
    // Initialize template system
    let mut tera = match Tera::new("templates/**/*") {
        Ok(t) => t,
        Err(e) => {
            error!("Template parsing error: {}", e);
            ::std::process::exit(1);
        }
    };
    tera.autoescape_on(vec!["html", ".html", ".htm"]);
    
    // Create application state
    let app_state = web::Data::new(AppState {
        template: tera,
    });
    
    // Initialize database
    let pool = match init_db().await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            std::process::exit(1);
        }
    };

    // Start HTTP server
    let secret_key = actix_web::cookie::Key::generate(); // 生成固定密钥
    HttpServer::new(move || {
        
        App::new()
            .app_data(app_state.clone())
            .app_data(web::Data::new(pool.clone()))
            .wrap(Logger::default())
            .wrap(
                SessionMiddleware::builder(
                    actix_session::storage::CookieSessionStore::default(),
                    secret_key.clone()
                )
                .cookie_name(String::from("bluster_session"))
                .cookie_secure(false)
                .cookie_http_only(true)
                .build()
            )
            .route("/", web::get().to(index))
            .route("/post/{id}", web::get().to(post_detail))
            .route("/about", web::get().to(about))
            .route("/login", web::get().to(login_page))
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/articles", web::get().to(get_articles))
            .route("/articles/{id}", web::get().to(get_article))
            .route("/articles", web::post().to(create_article))
            .route("/articles/{id}", web::put().to(update_article))
            .route("/articles/{id}", web::delete().to(delete_article))
            .route("/admin", web::get().to(admin_dashboard))
            .route("/admin/articles", web::get().to(admin_articles))
            .route("/admin/articles", web::post().to(admin_create_article))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
