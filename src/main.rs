use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use log::{info, error};
use serde::{Serialize, Deserialize};
use tera::{Tera, Context};
use std::sync::Mutex;

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
    posts: Mutex<Vec<Post>>,
}

async fn index(data: web::Data<AppState>) -> impl Responder {
    let mut ctx = Context::new();
    let posts = data.posts.lock().unwrap();
    ctx.insert("posts", &*posts);
    
    match data.template.render("index.html", &ctx) {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => {
            error!("Template rendering error: {}", e);
            HttpResponse::InternalServerError().body("Template rendering error")
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
    
    // Create some example posts
    let posts = vec![
        Post {
            id: 1,
            title: String::from("Rust Introduction"),
            summary: String::from("Basic knowledge and getting started guide for Rust language"),
            content: String::from("This is an introductory article about the Rust programming language..."),
            date: String::from("2025-06-01"),
        },
        Post {
            id: 2,
            title: String::from("Building Web Applications with Actix-web"),
            summary: String::from("How to create high-performance web applications using the Actix-web framework"),
            content: String::from("Actix-web is a powerful Rust Web framework..."),
            date: String::from("2025-06-15"),
        },
    ];
    
    // Create application state
    let app_state = web::Data::new(AppState {
        template: tera,
        posts: Mutex::new(posts),
    });
    
    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(index))
            .route("/about", web::get().to(about))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
