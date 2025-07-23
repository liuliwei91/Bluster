use actix_web::{web, App, HttpServer, Responder, HttpResponse};
use actix_web::middleware::Logger;
use actix_session::{Session, SessionMiddleware};
use actix_multipart::Multipart;
use futures_util::TryStreamExt;
use log::{info, error};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use crate::models::{init_db, verify_user};
use crate::services::{MarkdownService, FileService};
use tera::{Tera, Context};

mod models;
mod services;

// Helper function to strip HTML tags for creating plain text summaries
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    
    // Clean up extra whitespace
    result.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize, Serialize)]
struct ArticleForm {
    title: String,
    content: String,
}

#[derive(Deserialize, Serialize)]
struct AboutForm {
    title: String,
    content: String,
}

#[derive(Deserialize)]
struct ChangePasswordForm {
    current_password: String,
    new_password: String,
    confirm_password: String,
}

#[derive(Deserialize)]
struct SecurityQuestionForm {
    question: String,
    answer: String,
}

#[derive(Deserialize)]
struct ResetPasswordForm {
    username: String,
    security_answer: String,
    new_password: String,
    confirm_password: String,
}

#[derive(Deserialize)]
struct PreviewRequest {
    content: String,
}

#[derive(Serialize)]
struct PreviewResponse {
    html: String,
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
    markdown_service: MarkdownService,
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
                // Render markdown content to HTML with fallback
                let rendered_content = data.markdown_service.render_to_html_with_fallback(&content);
                
                // Create summary from plain text (strip HTML tags for summary)
                let plain_text = strip_html_tags(&rendered_content);
                let summary = plain_text.chars().take(100).collect();
                
                Post {
                    id: id as u32,
                    title,
                    summary,
                    content: rendered_content,
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
            // Render markdown content to HTML with fallback
            let rendered_content = data.markdown_service.render_to_html_with_fallback(&content);
            
            // Create summary from plain text
            let plain_text = strip_html_tags(&rendered_content);
            let summary = plain_text.chars().take(100).collect();
            
            let post = Post {
                id: id as u32,
                title,
                summary,
                content: rendered_content,
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

async fn about(
    data: web::Data<AppState>,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    let mut ctx = Context::new();
    
    match sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, title, content, updated_at FROM about ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(_pool.get_ref())
    .await {
        Ok((_, title, content, _)) => {
            ctx.insert("title", &title);
            ctx.insert("content", &content);
            match data.template.render("about.html", &ctx) {
                Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
                Err(e) => {
                    error!("Template rendering error: {}", e);
                    HttpResponse::InternalServerError().body("Template rendering error")
                }
            }
        },
        Err(e) => {
            error!("Failed to fetch about content: {}", e);
            // Fallback to default content
            ctx.insert("title", "About My Blog");
            ctx.insert("content", "This is a blog system built with Rust language and the Actix-web framework.");
            match data.template.render("about.html", &ctx) {
                Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
                Err(e) => {
                    error!("Template rendering error: {}", e);
                    HttpResponse::InternalServerError().body("Template rendering error")
                }
            }
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
        match sqlx::query_as::<_, (i64, String, String, String)>(
            "SELECT id, title, content, created_at FROM articles ORDER BY created_at DESC"
        )
        .fetch_all(_pool.get_ref())
        .await {
            Ok(articles) => {
                let mut ctx = Context::new();
                // 打印articles调试信息
                info!("Articles data: {:?}", articles);
                // 转换articles为模板需要的格式
                #[derive(serde::Serialize)]
                struct TemplateArticle {
                    id: i64,
                    title: String,
                    content: String,
                    created_at: String,
                }

                let template_articles: Vec<TemplateArticle> = articles.into_iter().map(|(id, title, content, created_at)| {
                    TemplateArticle {
                        id,
                        title,
                        content,
                        created_at,
                    }
                }).collect();
                ctx.insert("articles", &template_articles);
                match data.template.render("admin/dashboard.html", &ctx) {
                    Ok(html) => Ok(HttpResponse::Ok().content_type("text/html").body(html)),
                    Err(e) => {
                        error!("Detailed template rendering error: {:#?}", e);
                        Ok(HttpResponse::InternalServerError().body(format!("Detailed template error: {:#?}", e)))
                    }
                }
            },
            Err(e) => {
                error!("Failed to fetch articles: {}", e);
                Ok(HttpResponse::InternalServerError().finish())
            }
        }
    } else {
        // 未登录，重定向到登录页面
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn admin_articles(
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    
    match sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, title, content FROM articles ORDER BY created_at DESC"
    )
    .fetch_all(_pool.get_ref())
    .await {
        Ok(articles) => HttpResponse::Ok().json(articles),
        Err(e) => {
            error!("Failed to fetch articles: {}", e);
            HttpResponse::InternalServerError().json("Failed to fetch articles")
        }
    }
}

async fn admin_edit_article(
    data: web::Data<AppState>,
    path: web::Path<i64>,
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    let article_id = path.into_inner();
    match sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, title, content FROM articles WHERE id = ?"
    )
    .bind(article_id)
    .fetch_one(_pool.get_ref())
    .await {
        Ok((id, title, content)) => {
            #[derive(serde::Serialize)]
            struct TemplateArticle {
                id: i64,
                title: String,
                content: String
            }
            let mut ctx = Context::new();
            ctx.insert("article", &TemplateArticle {
                id,
                title,
                content
            });
            match data.template.render("admin/edit_article.html", &ctx) {
                Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
                Err(e) => {
                    error!("Template rendering error: {:#?}", e);
                    HttpResponse::InternalServerError().body(format!("Template rendering error: {:#?}", e))
                }
            }
        },
        Err(e) => {
            error!("Failed to fetch article: {}", e);
            HttpResponse::NotFound().finish()
        }
    }
}

async fn admin_update_article(
    path: web::Path<i64>,
    json: web::Json<ArticleForm>,
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    
    let article_id = path.into_inner();
    match sqlx::query(
        "UPDATE articles SET title = ?, content = ?, updated_at = datetime('now') WHERE id = ?"
    )
    .bind(&json.title)
    .bind(&json.content)
    .bind(article_id)
    .execute(_pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Ok().json("Article updated successfully"),
        Err(e) => {
            error!("Failed to update article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn admin_create_article(
    form: web::Form<ArticleForm>,
    _pool: web::Data<SqlitePool>,
    session: Session,
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    match sqlx::query(
        "INSERT INTO articles (title, content, created_at, updated_at) VALUES (?, ?, datetime('now'), datetime('now'))"
    )
    .bind(&form.title)
    .bind(&form.content)
    .execute(_pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Found().append_header(("Location", "/admin")).finish(),
        Err(e) => {
            error!("Failed to create article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn admin_about_edit(
    data: web::Data<AppState>,
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    
    let mut ctx = Context::new();
    
    match sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, title, content, updated_at FROM about ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(_pool.get_ref())
    .await {
        Ok((id, title, content, updated_at)) => {
            ctx.insert("id", &id);
            ctx.insert("title", &title);
            ctx.insert("content", &content);
            ctx.insert("updated_at", &updated_at);
            match data.template.render("admin/edit_about.html", &ctx) {
                Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
                Err(e) => {
                    error!("Template rendering error: {:#?}", e);
                    HttpResponse::InternalServerError().body(format!("Template rendering error: {:#?}", e))
                }
            }
        },
        Err(e) => {
            error!("Failed to fetch about content: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn admin_update_about(
    json: web::Json<AboutForm>,
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    
    match sqlx::query(
        "UPDATE about SET title = ?, content = ?, updated_at = datetime('now') WHERE id = (SELECT id FROM about ORDER BY id DESC LIMIT 1)"
    )
    .bind(&json.title)
    .bind(&json.content)
    .execute(_pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Ok().json("About content updated successfully"),
        Err(e) => {
            error!("Failed to update about content: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn login(
    form: web::Form<LoginForm>,
    _pool: web::Data<SqlitePool>,
    session: Session,
) -> impl Responder {
    match verify_user(&_pool, &form.username, &form.password).await {
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

async fn get_articles(_pool: web::Data<SqlitePool>) -> impl Responder {
    match sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, title, content FROM articles ORDER BY created_at DESC"
    )
    .fetch_all(_pool.get_ref())
    .await {
        Ok(articles) => HttpResponse::Ok().json(articles),
        Err(e) => {
            error!("Failed to fetch articles: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

async fn get_article(
    _pool: web::Data<SqlitePool>,
    path: web::Path<i64>
) -> impl Responder {
    let article_id = path.into_inner();
    match sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, title, content FROM articles WHERE id = ?"
    )
    .bind(article_id)
    .fetch_one(_pool.get_ref())
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
    _pool: web::Data<SqlitePool>,
    session: Session,
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    let article_id = path.into_inner();
    match sqlx::query(
        "UPDATE articles SET title = ?, content = ?, updated_at = datetime('now') WHERE id = ?"
    )
    .bind(&form.title)
    .bind(&form.content)
    .bind(article_id)
    .execute(_pool.get_ref())
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
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish();
    }
    let article_id = path.into_inner();
    match sqlx::query(
        "DELETE FROM articles WHERE id = ?"
    )
    .bind(article_id)
    .execute(_pool.get_ref())
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
    _pool: web::Data<SqlitePool>,
    session: Session,
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json("Unauthorized");
    }
    match sqlx::query(
        "INSERT INTO articles (title, content, created_at, updated_at) VALUES (?, ?, datetime('now'), datetime('now'))"
    )
    .bind(&form.title)
    .bind(&form.content)
    .execute(_pool.get_ref())
    .await {
        Ok(_) => HttpResponse::Ok().json("Article created successfully"),
        Err(e) => {
            error!("Failed to create article: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

// 管理员密码设置页面
async fn admin_password_settings(
    data: web::Data<AppState>,
    session: Session,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    if let Ok(Some(_)) = session.get::<String>("username") {
        // 获取当前用户信息
        let username = session.get::<String>("username").unwrap().unwrap();
        match sqlx::query_as::<_, models::User>("SELECT * FROM users WHERE username = ?")
            .bind(&username)
            .fetch_one(_pool.get_ref())
            .await {
            Ok(user) => {
                let mut ctx = Context::new();
                ctx.insert("user", &user);
                match data.template.render("admin/password_settings.html", &ctx) {
                    Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
                    Err(e) => {
                        error!("Template rendering error: {}", e);
                        HttpResponse::InternalServerError().body("Template rendering error")
                    }
                }
            },
            Err(_) => HttpResponse::Found().append_header(("Location", "/login")).finish()
        }
    } else {
        HttpResponse::Found().append_header(("Location", "/login")).finish()
    }
}

// 修改密码
async fn admin_change_password(
    form: web::Form<ChangePasswordForm>,
    session: Session,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    if let Ok(Some(username)) = session.get::<String>("username") {
        if form.new_password != form.confirm_password {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "message": "新密码和确认密码不匹配"
            }));
        }
        
        // 验证当前密码
        match models::verify_user(_pool.get_ref(), &username, &form.current_password).await {
            Ok(user) => {
                // 更新密码
                match models::update_user_password(_pool.get_ref(), user.id, &form.new_password).await {
                    Ok(_) => HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "message": "密码修改成功"
                    })),
                    Err(e) => {
                        error!("Failed to update password: {}", e);
                        HttpResponse::InternalServerError().json(serde_json::json!({
                            "success": false,
                            "message": "密码更新失败"
                        }))
                    }
                }
            },
            Err(_) => HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "message": "当前密码错误"
            }))
        }
    } else {
        HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "message": "未登录"
        }))
    }
}

// 设置安全问题
async fn admin_set_security_question(
    form: web::Form<SecurityQuestionForm>,
    session: Session,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    if let Ok(Some(username)) = session.get::<String>("username") {
        // 获取用户ID
        match sqlx::query_as::<_, models::User>("SELECT * FROM users WHERE username = ?")
            .bind(&username)
            .fetch_one(_pool.get_ref())
            .await {
            Ok(user) => {
                match models::set_security_question(_pool.get_ref(), user.id, &form.question, &form.answer).await {
                    Ok(_) => HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "message": "安全问题设置成功"
                    })),
                    Err(e) => {
                        error!("Failed to set security question: {}", e);
                        HttpResponse::InternalServerError().json(serde_json::json!({
                            "success": false,
                            "message": "设置失败"
                        }))
                    }
                }
            },
            Err(_) => HttpResponse::Unauthorized().json(serde_json::json!({
                "success": false,
                "message": "用户不存在"
            }))
        }
    } else {
        HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "message": "未登录"
        }))
    }
}

// 重置密码页面
async fn reset_password_page(data: web::Data<AppState>) -> impl Responder {
    match data.template.render("reset_password.html", &Context::new()) {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => {
            error!("Template rendering error: {}", e);
            HttpResponse::InternalServerError().body("Template rendering error")
        }
    }
}

// 处理重置密码
async fn reset_password(
    form: web::Form<ResetPasswordForm>,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    if form.new_password != form.confirm_password {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "新密码和确认密码不匹配"
        }));
    }
    
    // 验证安全问题答案
    match models::verify_security_answer(_pool.get_ref(), &form.username, &form.security_answer).await {
        Ok(_) => {
            // 重置密码
            match models::reset_password_by_username(_pool.get_ref(), &form.username, &form.new_password).await {
                Ok(_) => HttpResponse::Ok().json(serde_json::json!({
                    "success": true,
                    "message": "密码重置成功，请使用新密码登录"
                })),
                Err(e) => {
                    error!("Failed to reset password: {}", e);
                    HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "message": "密码重置失败"
                    }))
                }
            }
        },
        Err(_) => HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "用户名或安全问题答案错误"
        }))
    }
}

// 获取用户安全问题
async fn get_security_question(
    query: web::Query<std::collections::HashMap<String, String>>,
    _pool: web::Data<SqlitePool>
) -> impl Responder {
    if let Some(username) = query.get("username") {
        match sqlx::query_as::<_, models::User>("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_one(_pool.get_ref())
            .await {
            Ok(user) => {
                if let Some(question) = user.security_question {
                    HttpResponse::Ok().json(serde_json::json!({
                        "success": true,
                        "question": question
                    }))
                } else {
                    HttpResponse::BadRequest().json(serde_json::json!({
                        "success": false,
                        "message": "该用户未设置安全问题"
                    }))
                }
            },
            Err(_) => HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "message": "用户不存在"
            }))
        }
    } else {
        HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "缺少用户名参数"
        }))
    }
}

// Performance monitoring endpoint
async fn admin_performance_stats(
    data: web::Data<AppState>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Unauthorized"
        }));
    }
    
    let metrics = data.markdown_service.get_metrics();
    let cache_hit_rate = if metrics.cache_hits + metrics.cache_misses > 0 {
        (metrics.cache_hits as f64 / (metrics.cache_hits + metrics.cache_misses) as f64) * 100.0
    } else {
        0.0
    };
    
    HttpResponse::Ok().json(serde_json::json!({
        "total_renders": metrics.total_renders,
        "cache_hits": metrics.cache_hits,
        "cache_misses": metrics.cache_misses,
        "cache_hit_rate": format!("{:.1}%", cache_hit_rate),
        "avg_render_time_ms": format!("{:.2}", metrics.avg_render_time_ms),
        "cache_size": metrics.cache_size,
        "memory_usage_kb": format!("{:.2}", metrics.memory_usage_bytes as f64 / 1024.0)
    }))
}

// Cache management endpoint
async fn admin_cache_clear(
    data: web::Data<AppState>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Unauthorized"
        }));
    }
    
    data.markdown_service.clear_cache();
    
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Cache cleared successfully"
    }))
}

// Cache optimization endpoint
async fn admin_cache_optimize(
    data: web::Data<AppState>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Unauthorized"
        }));
    }
    
    data.markdown_service.optimize_cache();
    
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Cache optimized successfully"
    }))
}

// Markdown预览功能
async fn admin_preview_markdown(
    data: web::Data<AppState>,
    json: web::Json<PreviewRequest>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Unauthorized"
        }));
    }
    
    match data.markdown_service.render_to_html(&json.content) {
        Ok(html) => HttpResponse::Ok().json(PreviewResponse { html }),
        Err(e) => {
            error!("Markdown rendering failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to render markdown",
                "details": e.to_string()
            }))
        }
    }
}

// 文件导入功能
async fn admin_import_article(
    mut payload: Multipart,
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "message": "Unauthorized"
        }));
    }

    // 处理文件上传
    while let Some(mut field) = payload.try_next().await.unwrap_or(None) {
        let content_disposition = field.content_disposition();
        
        if let Some(filename) = content_disposition.get_filename() {
            // 验证文件扩展名
            if let Err(e) = FileService::validate_file_extension(filename) {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "message": format!("Invalid file type: {}", e)
                }));
            }

            // 读取文件内容
            let mut file_content = Vec::new();
            while let Some(chunk) = field.try_next().await.unwrap_or(None) {
                file_content.extend_from_slice(&chunk);
            }

            // 转换为字符串
            let content_str = match String::from_utf8(file_content) {
                Ok(content) => content,
                Err(_) => {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "success": false,
                        "message": "File must be valid UTF-8 text"
                    }));
                }
            };

            // 验证文件大小 (5MB limit)
            if let Err(e) = FileService::validate_file_size(&content_str, 5) {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "success": false,
                    "message": format!("File too large: {}", e)
                }));
            }

            // 解析Markdown文件
            match FileService::parse_markdown_file(&content_str) {
                Ok(markdown_file) => {
                    // 插入到数据库
                    match sqlx::query(
                        "INSERT INTO articles (title, content, created_at, updated_at) VALUES (?, ?, datetime('now'), datetime('now'))"
                    )
                    .bind(&markdown_file.title)
                    .bind(&markdown_file.content)
                    .execute(_pool.get_ref())
                    .await {
                        Ok(result) => {
                            let article_id = result.last_insert_rowid();
                            return HttpResponse::Ok().json(serde_json::json!({
                                "success": true,
                                "message": "Article imported successfully",
                                "article_id": article_id,
                                "title": markdown_file.title
                            }));
                        },
                        Err(e) => {
                            error!("Failed to insert article: {}", e);
                            return HttpResponse::InternalServerError().json(serde_json::json!({
                                "success": false,
                                "message": "Failed to save article to database"
                            }));
                        }
                    }
                },
                Err(e) => {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "success": false,
                        "message": format!("Failed to parse markdown file: {}", e)
                    }));
                }
            }
        }
    }

    HttpResponse::BadRequest().json(serde_json::json!({
        "success": false,
        "message": "No valid file found in upload"
    }))
}

// 文章导出功能
async fn admin_export_article(
    path: web::Path<i64>,
    _pool: web::Data<SqlitePool>,
    session: Session
) -> impl Responder {
    // 检查session中的登录状态
    if session.get::<String>("username").unwrap_or(None).is_none() {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Unauthorized"
        }));
    }

    let article_id = path.into_inner();
    
    // 从数据库获取文章
    match sqlx::query_as::<_, models::Article>(
        "SELECT id, title, content, author_id, created_at, updated_at FROM articles WHERE id = ?"
    )
    .bind(article_id)
    .fetch_one(_pool.get_ref())
    .await {
        Ok(article) => {
            // 生成Markdown导出内容
            let markdown_content = match FileService::generate_markdown_export(&article) {
                Ok(content) => content,
                Err(e) => {
                    error!("Failed to generate markdown export: {}", e);
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "success": false,
                        "message": format!("Export generation failed: {}", e)
                    }));
                }
            };
            
            // 生成安全的文件名
            let safe_filename = FileService::sanitize_filename_with_fallback(&article.title);
            let filename = format!("{}.md", safe_filename);
            
            // 返回文件下载响应
            HttpResponse::Ok()
                .content_type("text/markdown; charset=utf-8")
                .append_header(("Content-Disposition", format!("attachment; filename=\"{}\"", filename)))
                .body(markdown_content)
        },
        Err(e) => {
            error!("Failed to fetch article for export: {}", e);
            HttpResponse::NotFound().json(serde_json::json!({
                "error": "Article not found"
            }))
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
    
    // Create application state with optimized markdown service
    let cache_ttl = std::env::var("MARKDOWN_CACHE_TTL")
        .unwrap_or_else(|_| "3600".to_string())
        .parse::<u64>()
        .unwrap_or(3600);
    
    let max_cache_size = std::env::var("MARKDOWN_MAX_CACHE_SIZE")
        .unwrap_or_else(|_| "1000".to_string())
        .parse::<usize>()
        .unwrap_or(1000);
    
    let max_content_size = std::env::var("MARKDOWN_MAX_CONTENT_SIZE")
        .unwrap_or_else(|_| "1048576".to_string())
        .parse::<usize>()
        .unwrap_or(1024 * 1024);
    
    let markdown_service = MarkdownService::with_cache_config(
        std::time::Duration::from_secs(cache_ttl),
        max_cache_size,
        max_content_size,
    );
    
    info!("Markdown service configured with cache TTL: {}s, max cache size: {}, max content size: {} bytes", 
          cache_ttl, max_cache_size, max_content_size);
    
    let app_state = web::Data::new(AppState {
        template: tera,
        markdown_service,
    });
    
    // Start periodic cache optimization task
    let app_state_for_task = app_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1800)); // Every 30 minutes
        loop {
            interval.tick().await;
            log::info!("Running periodic cache optimization...");
            app_state_for_task.markdown_service.optimize_cache();
        }
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
            .route("/admin", web::get().to(admin_dashboard))
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
            .route("/admin/articles", web::get().to(admin_articles))
            .route("/admin/articles", web::post().to(admin_create_article))
            .route("/admin/articles/{id}/edit", web::get().to(admin_edit_article))
            .route("/admin/articles/{id}", web::put().to(admin_update_article))
            .route("/admin/articles/{id}", web::delete().to(delete_article))
            .route("/admin/articles/preview", web::post().to(admin_preview_markdown))
            .route("/admin/articles/import", web::post().to(admin_import_article))
            .route("/admin/articles/{id}/export", web::get().to(admin_export_article))
            .route("/admin/about/edit", web::get().to(admin_about_edit))
            .route("/admin/about", web::put().to(admin_update_about))
            .route("/admin/password", web::get().to(admin_password_settings))
            .route("/admin/password/change", web::post().to(admin_change_password))
            .route("/admin/security-question", web::post().to(admin_set_security_question))
            .route("/admin/performance", web::get().to(admin_performance_stats))
            .route("/admin/cache/clear", web::post().to(admin_cache_clear))
            .route("/admin/cache/optimize", web::post().to(admin_cache_optimize))
            .route("/reset-password", web::get().to(reset_password_page))
            .route("/reset-password", web::post().to(reset_password))
            .route("/api/security-question", web::get().to(get_security_question))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
