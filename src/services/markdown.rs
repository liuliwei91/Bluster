use pulldown_cmark::{Parser, Options, html, Event, Tag, CodeBlockKind};
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use syntect::html::ClassedHTMLGenerator;
use syntect::util::LinesWithEndings;
use html_escape;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[derive(Debug, thiserror::Error)]
pub enum MarkdownError {
    #[error("Markdown parsing failed: {0}")]
    ParseError(String),
    #[error("HTML sanitization failed: {0}")]
    SanitizationError(String),
    #[error("Syntax highlighting failed: {0}")]
    HighlightError(String),
}

// Cache entry structure
#[derive(Clone)]
struct CacheEntry {
    html: String,
    created_at: Instant,
    access_count: u64,
}

// Performance metrics
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub total_renders: u64,
    pub avg_render_time_ms: f64,
    pub cache_size: usize,
    pub memory_usage_bytes: usize,
}

pub struct MarkdownService {
    syntax_set: SyntaxSet,
    #[allow(dead_code)] // Reserved for future theme customization
    theme_set: ThemeSet,
    options: Options,
    // HTML rendering cache with TTL and LRU eviction
    html_cache: Arc<RwLock<HashMap<u64, CacheEntry>>>,
    // Performance metrics
    metrics: Arc<RwLock<PerformanceMetrics>>,
    // Cache configuration
    cache_ttl: Duration,
    max_cache_size: usize,
    max_content_size: usize, // Maximum content size to cache (bytes)
}

impl MarkdownService {
    pub fn new() -> Self {
        Self::with_cache_config(
            Duration::from_secs(3600), // 1 hour TTL
            1000,                      // Max 1000 cached entries
            1024 * 1024,              // Max 1MB content size to cache
        )
    }

    pub fn with_cache_config(cache_ttl: Duration, max_cache_size: usize, max_content_size: usize) -> Self {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TASKLISTS);
        
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            options,
            html_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(PerformanceMetrics {
                cache_hits: 0,
                cache_misses: 0,
                total_renders: 0,
                avg_render_time_ms: 0.0,
                cache_size: 0,
                memory_usage_bytes: 0,
            })),
            cache_ttl,
            max_cache_size,
            max_content_size,
        }
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().unwrap().clone()
    }

    /// Clear cache and reset metrics
    pub fn clear_cache(&self) {
        let mut cache = self.html_cache.write().unwrap();
        cache.clear();
        
        let mut metrics = self.metrics.write().unwrap();
        metrics.cache_size = 0;
        metrics.memory_usage_bytes = 0;
        
        log::info!("Markdown cache cleared");
    }

    /// Generate cache key from markdown content
    fn generate_cache_key(&self, markdown: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        markdown.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if content should be cached based on size
    fn should_cache_content(&self, content: &str) -> bool {
        content.len() <= self.max_content_size
    }

    /// Evict expired and least recently used cache entries
    fn evict_cache_entries(&self) {
        let mut cache = self.html_cache.write().unwrap();
        let now = Instant::now();
        
        // Remove expired entries
        let expired_keys: Vec<u64> = cache
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.created_at) > self.cache_ttl)
            .map(|(key, _)| *key)
            .collect();
        
        for key in expired_keys {
            cache.remove(&key);
        }
        
        // If still over limit, remove least recently used entries
        if cache.len() > self.max_cache_size {
            let mut entries: Vec<(u64, u64)> = cache
                .iter()
                .map(|(key, entry)| (*key, entry.access_count))
                .collect();
            
            // Sort by access count (ascending) to remove least used first
            entries.sort_by_key(|(_, access_count)| *access_count);
            
            let remove_count = cache.len() - self.max_cache_size;
            for (key, _) in entries.iter().take(remove_count) {
                cache.remove(key);
            }
        }
        
        // Update metrics
        let mut metrics = self.metrics.write().unwrap();
        metrics.cache_size = cache.len();
        metrics.memory_usage_bytes = cache
            .values()
            .map(|entry| entry.html.len())
            .sum();
    }

    pub fn render_to_html(&self, markdown: &str) -> Result<String, MarkdownError> {
        let start_time = Instant::now();
        
        // Validate input
        if markdown.trim().is_empty() {
            return Ok(String::new());
        }

        // Check cache first if content is cacheable
        let cache_key = if self.should_cache_content(markdown) {
            Some(self.generate_cache_key(markdown))
        } else {
            None
        };

        if let Some(key) = cache_key {
            // Try to get from cache
            if let Ok(mut cache) = self.html_cache.write() {
                if let Some(entry) = cache.get_mut(&key) {
                    // Check if entry is still valid
                    if start_time.duration_since(entry.created_at) <= self.cache_ttl {
                        // Cache hit - update access count and return cached result
                        entry.access_count += 1;
                        
                        // Update metrics
                        if let Ok(mut metrics) = self.metrics.write() {
                            metrics.cache_hits += 1;
                        }
                        
                        log::debug!("Markdown cache hit for content hash: {}", key);
                        return Ok(entry.html.clone());
                    } else {
                        // Entry expired, remove it
                        cache.remove(&key);
                    }
                }
            }
        }

        // Cache miss or non-cacheable content - render markdown
        log::debug!("Rendering markdown content (size: {} bytes)", markdown.len());
        
        // Parse markdown with custom event processing for code highlighting
        let parser = Parser::new_ext(markdown, self.options);
        let events = self.process_events(parser)
            .map_err(|e| MarkdownError::ParseError(format!("Event processing failed: {}", e)))?;
        
        // Convert processed events to HTML
        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());
        
        // Sanitize HTML to prevent XSS
        let sanitized_html = self.sanitize_html(&html_output)
            .map_err(|e| MarkdownError::SanitizationError(format!("HTML sanitization failed: {}", e)))?;
        
        let render_time = start_time.elapsed();
        
        // Cache the result if applicable
        if let Some(key) = cache_key {
            if let Ok(mut cache) = self.html_cache.write() {
                // Evict old entries if needed
                if cache.len() >= self.max_cache_size {
                    self.evict_cache_entries();
                }
                
                // Add new entry to cache
                cache.insert(key, CacheEntry {
                    html: sanitized_html.clone(),
                    created_at: start_time,
                    access_count: 1,
                });
                
                log::debug!("Cached markdown result for content hash: {}", key);
            }
        }
        
        // Update performance metrics
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.cache_misses += 1;
            metrics.total_renders += 1;
            
            // Update average render time using exponential moving average
            let render_time_ms = render_time.as_millis() as f64;
            if metrics.total_renders == 1 {
                metrics.avg_render_time_ms = render_time_ms;
            } else {
                // EMA with alpha = 0.1 for smoothing
                metrics.avg_render_time_ms = 0.9 * metrics.avg_render_time_ms + 0.1 * render_time_ms;
            }
            
            // Update cache size
            if let Ok(cache) = self.html_cache.read() {
                metrics.cache_size = cache.len();
                metrics.memory_usage_bytes = cache
                    .values()
                    .map(|entry| entry.html.len())
                    .sum();
            }
        }
        
        log::debug!("Markdown rendering completed in {:.2}ms", render_time.as_millis());
        
        Ok(sanitized_html)
    }

    /// Render markdown with fallback to original content on error
    pub fn render_to_html_with_fallback(&self, markdown: &str) -> String {
        match self.render_to_html(markdown) {
            Ok(html) => html,
            Err(e) => {
                log::warn!("Markdown rendering failed, falling back to escaped original content: {}", e);
                // Fallback: return HTML-escaped original markdown content
                html_escape::encode_text(markdown).to_string()
            }
        }
    }

    fn process_events<'a>(&self, parser: Parser<'a, 'a>) -> Result<Vec<Event<'a>>, MarkdownError> {
        // Pre-allocate with reasonable capacity to reduce reallocations
        let mut events = Vec::with_capacity(256);
        let mut in_code_block = false;
        let mut code_block_lang = String::new();
        let mut code_block_content = String::new();
        
        // Reserve capacity for code block content to reduce reallocations
        code_block_content.reserve(1024);

        for event in parser {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    in_code_block = true;
                    code_block_lang = lang.to_string();
                    code_block_content.clear();
                }
                Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)) => {
                    in_code_block = true;
                    code_block_lang = String::new(); // No language for indented code blocks
                    code_block_content.clear();
                }
                Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(_))) | 
                Event::End(Tag::CodeBlock(CodeBlockKind::Indented)) => {
                    if in_code_block {
                        // Generate syntax highlighted HTML with fallback
                        let highlighted = self.highlight_code_with_fallback(&code_block_content, &code_block_lang);
                        
                        // Create HTML event for the highlighted code
                        let class_attr = if code_block_lang.is_empty() {
                            "highlight".to_string()
                        } else {
                            format!("highlight language-{}", code_block_lang)
                        };
                        
                        let code_class = if code_block_lang.is_empty() {
                            String::new()
                        } else {
                            format!("language-{}", code_block_lang)
                        };
                        
                        events.push(Event::Html(format!(
                            "<pre class=\"{}\"><code class=\"{}\">{}</code></pre>",
                            class_attr, code_class, highlighted
                        ).into()));
                        
                        in_code_block = false;
                    }
                }
                Event::Text(text) if in_code_block => {
                    // Limit code block size to prevent memory issues
                    if code_block_content.len() + text.len() > 100_000 { // 100KB limit
                        log::warn!("Code block too large, truncating at 100KB");
                        let remaining_capacity = 100_000 - code_block_content.len();
                        if remaining_capacity > 0 {
                            code_block_content.push_str(&text[..remaining_capacity.min(text.len())]);
                        }
                    } else {
                        code_block_content.push_str(&text);
                    }
                }
                Event::Code(code) => {
                    // Handle inline code with CSS class
                    events.push(Event::Html(format!(
                        "<code class=\"inline-code\">{}</code>",
                        html_escape::encode_text(&code)
                    ).into()));
                }
                _ => {
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    pub fn sanitize_html(&self, html: &str) -> Result<String, MarkdownError> {
        // Configure ammonia to allow safe HTML tags
        let mut builder = ammonia::Builder::new();
        
        // Allow common markdown-generated tags
        builder
            .add_tags(&[
                "h1", "h2", "h3", "h4", "h5", "h6",
                "p", "br", "hr",
                "strong", "em", "u", "s", "del", "ins",
                "ul", "ol", "li",
                "blockquote",
                "code", "pre",
                "table", "thead", "tbody", "tfoot", "tr", "th", "td", "caption",
                "a", "img",
                "div", "span",
                "input"  // Allow input for task list checkboxes
            ])
            .add_tag_attributes("a", &["href", "title"])
            .add_tag_attributes("img", &["src", "alt", "title", "width", "height", "loading"])
            .add_tag_attributes("code", &["class"])
            .add_tag_attributes("pre", &["class"])
            .add_tag_attributes("div", &["class"])
            .add_tag_attributes("span", &["class"])
            .add_tag_attributes("table", &["class"])
            .add_tag_attributes("thead", &["class"])
            .add_tag_attributes("tbody", &["class"])
            .add_tag_attributes("tr", &["class"])
            .add_tag_attributes("th", &["class", "scope"])
            .add_tag_attributes("td", &["class"])
            .add_tag_attributes("input", &["type", "checked", "disabled"]);

        let cleaned = builder.clean(html).to_string();
        
        // Post-process to add security attributes and table accessibility
        let enhanced_html = self.enhance_html_security_and_accessibility(&cleaned);
        
        Ok(enhanced_html)
    }

    /// Enhance HTML with security attributes and table accessibility
    fn enhance_html_security_and_accessibility(&self, html: &str) -> String {
        let mut enhanced = html.to_string();
        
        // Add security attributes to external links (simple string replacement approach)
        // Look for external links and add security attributes
        enhanced = self.add_security_to_external_links(&enhanced);
        
        // Add loading="lazy" to images
        enhanced = self.add_lazy_loading_to_images(&enhanced);
        
        // Block dangerous links
        enhanced = self.block_dangerous_links(&enhanced);
        
        // Wrap tables in responsive container
        enhanced = enhanced.replace("<table>", "<div class=\"table-responsive\"><table class=\"markdown-table\">");
        enhanced = enhanced.replace("</table>", "</table></div>");
        
        // Add scope attributes to table headers for accessibility
        enhanced = enhanced.replace("<th>", "<th scope=\"col\">");
        
        enhanced
    }

    /// Add security attributes to external links using simple string processing
    fn add_security_to_external_links(&self, html: &str) -> String {
        let mut result = String::new();
        let mut chars = html.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '<' && chars.peek() == Some(&'a') {
                // Found potential link start
                let mut tag = String::from("<");
                
                // Collect the entire tag
                while let Some(ch) = chars.next() {
                    tag.push(ch);
                    if ch == '>' {
                        break;
                    }
                }
                
                // Check if it's an external link and add security attributes
                if tag.contains("href=\"http://") || tag.contains("href=\"https://") {
                    // Insert security attributes before the closing >
                    if let Some(pos) = tag.rfind('>') {
                        let mut secure_tag = tag[..pos].to_string();
                        if !secure_tag.contains("rel=") {
                            secure_tag.push_str(" rel=\"noopener noreferrer\"");
                        }
                        if !secure_tag.contains("target=") {
                            secure_tag.push_str(" target=\"_blank\"");
                        }
                        secure_tag.push('>');
                        result.push_str(&secure_tag);
                    } else {
                        result.push_str(&tag);
                    }
                } else {
                    result.push_str(&tag);
                }
            } else {
                result.push(ch);
            }
        }
        
        result
    }

    pub fn highlight_code(&self, code: &str, language: &str) -> Result<String, MarkdownError> {
        if code.trim().is_empty() {
            return Ok(String::new());
        }

        if language.is_empty() {
            // No language specified, return escaped plain text
            return Ok(html_escape::encode_text(code).to_string());
        }

        let syntax = self.syntax_set
            .find_syntax_by_extension(language)
            .or_else(|| self.syntax_set.find_syntax_by_name(language))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        // Use syntect to highlight the code with CSS classes
        let mut highlighter = ClassedHTMLGenerator::new_with_class_style(
            syntax, &self.syntax_set, syntect::html::ClassStyle::Spaced
        );
        
        // Process each line with error handling
        for line in LinesWithEndings::from(code) {
            if let Err(e) = highlighter.parse_html_for_line_which_includes_newline(line) {
                return Err(MarkdownError::HighlightError(format!(
                    "Failed to highlight line '{}': {}", 
                    line.trim(), 
                    e
                )));
            }
        }
        
        // Get the final HTML from the highlighter
        let html_output = highlighter.finalize();
        
        if html_output.trim().is_empty() {
            // Fallback to escaped plain text if highlighting produced empty result
            Ok(html_escape::encode_text(code).to_string())
        } else {
            Ok(html_output)
        }
    }

    /// Highlight code with fallback to plain text on error
    pub fn highlight_code_with_fallback(&self, code: &str, language: &str) -> String {
        let start_time = Instant::now();
        
        let result = match self.highlight_code(code, language) {
            Ok(html) => html,
            Err(e) => {
                log::warn!("Code highlighting failed for language '{}', falling back to plain text: {}", language, e);
                // Fallback: return HTML-escaped plain text
                html_escape::encode_text(code).to_string()
            }
        };
        
        let highlight_time = start_time.elapsed();
        if highlight_time > Duration::from_millis(100) {
            log::warn!("Code highlighting took {:.2}ms for {} bytes of {} code", 
                      highlight_time.as_millis(), code.len(), language);
        }
        
        result
    }

    /// Log performance statistics
    pub fn log_performance_stats(&self) {
        if let Ok(metrics) = self.metrics.read() {
            let cache_hit_rate = if metrics.cache_hits + metrics.cache_misses > 0 {
                (metrics.cache_hits as f64 / (metrics.cache_hits + metrics.cache_misses) as f64) * 100.0
            } else {
                0.0
            };
            
            log::info!("Markdown Service Performance Stats:");
            log::info!("  Total renders: {}", metrics.total_renders);
            log::info!("  Cache hits: {} ({:.1}%)", metrics.cache_hits, cache_hit_rate);
            log::info!("  Cache misses: {}", metrics.cache_misses);
            log::info!("  Average render time: {:.2}ms", metrics.avg_render_time_ms);
            log::info!("  Cache size: {} entries", metrics.cache_size);
            log::info!("  Memory usage: {:.2}KB", metrics.memory_usage_bytes as f64 / 1024.0);
        }
    }

    /// Optimize cache by removing least used entries
    pub fn optimize_cache(&self) {
        let start_time = Instant::now();
        self.evict_cache_entries();
        let optimize_time = start_time.elapsed();
        
        log::info!("Cache optimization completed in {:.2}ms", optimize_time.as_millis());
        self.log_performance_stats();
    }



    /// Add loading="lazy" to images
    fn add_lazy_loading_to_images(&self, html: &str) -> String {
        // Simple regex-like replacement for images
        let _result = html.to_string();
        
        // Find <img tags and add loading="lazy" if not present
        let mut new_result = String::new();
        let mut chars = html.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '<' && chars.peek() == Some(&'i') {
                // Check if this is an img tag
                let mut tag = String::from("<");
                let mut is_img_tag = false;
                
                // Collect the tag
                while let Some(ch) = chars.next() {
                    tag.push(ch);
                    if tag.len() == 4 && tag == "<img" {
                        is_img_tag = true;
                    }
                    if ch == '>' {
                        break;
                    }
                }
                
                if is_img_tag && !tag.contains("loading=") {
                    // Insert loading="lazy" before the closing >
                    if let Some(pos) = tag.rfind('>') {
                        let mut new_tag = tag[..pos].to_string();
                        new_tag.push_str(" loading=\"lazy\">");
                        new_result.push_str(&new_tag);
                    } else {
                        new_result.push_str(&tag);
                    }
                } else {
                    new_result.push_str(&tag);
                }
            } else {
                new_result.push(ch);
            }
        }
        
        new_result
    }

    /// Block dangerous links by replacing them with safe fallbacks
    fn block_dangerous_links(&self, html: &str) -> String {
        let mut result = html.to_string();
        
        // Block javascript: links
        result = result.replace("href=\"javascript:", "href=\"#\" data-blocked=\"javascript:");
        
        // Block data: links (except safe image data URLs)
        if result.contains("href=\"data:") && !result.contains("href=\"data:image/") {
            result = result.replace("href=\"data:", "href=\"#\" data-blocked=\"data:");
        }
        
        result
    }
}

impl Default for MarkdownService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_markdown_rendering() {
        let service = MarkdownService::new();
        let markdown = "# Hello World\n\nThis is a **bold** text.";
        let result = service.render_to_html(markdown).unwrap();
        
        assert!(result.contains("<h1>"));
        assert!(result.contains("<strong>"));
    }

    #[test]
    fn test_html_sanitization() {
        let service = MarkdownService::new();
        let dangerous_html = "<script>alert('xss')</script><p>Safe content</p>";
        let result = service.sanitize_html(dangerous_html).unwrap();
        
        assert!(!result.contains("<script>"));
        assert!(result.contains("<p>Safe content</p>"));
    }

    #[test]
    fn test_table_support() {
        let service = MarkdownService::new();
        let markdown = "| Header 1 | Header 2 |\n|----------|----------|\n| Cell 1   | Cell 2   |";
        let result = service.render_to_html(markdown).unwrap();
        
        println!("Table result: {}", result);
        assert!(result.contains("<table") || result.contains("class=\"markdown-table\""));
        assert!(result.contains("<th"));
        assert!(result.contains("<td"));
    }

    #[test]
    fn test_code_block_highlighting() {
        let service = MarkdownService::new();
        let markdown = "```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```";
        let result = service.render_to_html(markdown).unwrap();
        
        assert!(result.contains("class=\"highlight language-rust\""));
        assert!(result.contains("language-rust"));
        assert!(result.contains("<span"));  // Should have syntax highlighting spans
    }

    #[test]
    fn test_multiple_language_highlighting() {
        let service = MarkdownService::new();
        
        // Test JavaScript
        let js_markdown = "```javascript\nfunction hello() {\n    console.log('Hello');\n}\n```";
        let js_result = service.render_to_html(js_markdown).unwrap();
        assert!(js_result.contains("language-javascript"));
        assert!(js_result.contains("<span"));
        
        // Test Python
        let py_markdown = "```python\ndef hello():\n    print('Hello')\n```";
        let py_result = service.render_to_html(py_markdown).unwrap();
        assert!(py_result.contains("language-python"));
        assert!(py_result.contains("<span"));
        
        // Test unknown language (should still work)
        let unknown_markdown = "```unknown\nsome code\n```";
        let unknown_result = service.render_to_html(unknown_markdown).unwrap();
        assert!(unknown_result.contains("language-unknown"));
    }

    #[test]
    fn test_code_block_without_language() {
        let service = MarkdownService::new();
        let markdown = "```\nplain code block\n```";
        let result = service.render_to_html(markdown).unwrap();
        
        // Should still create a code block but without specific language class
        assert!(result.contains("<pre"));
        assert!(result.contains("<code"));
        assert!(result.contains("plain code block"));
    }

    #[test]
    fn test_inline_code() {
        let service = MarkdownService::new();
        let markdown = "This is `inline code` in a sentence.";
        let result = service.render_to_html(markdown).unwrap();
        
        println!("Inline code result: {}", result);
        assert!(result.contains("<code"));
        assert!(result.contains("inline code"));
    }

    #[test]
    fn test_links_and_images() {
        let service = MarkdownService::new();
        let markdown = "[Link](https://example.com) and ![Image](image.jpg)";
        let result = service.render_to_html(markdown).unwrap();
        
        println!("Links and images result: {}", result);
        assert!(result.contains("<a href=\"https://example.com\""));
        assert!(result.contains("<img src=\"image.jpg\""));
        assert!(result.contains("alt=\"Image\""));
    }

    #[test]
    fn test_image_only() {
        let service = MarkdownService::new();
        let markdown = "![Alt Text](image.jpg)";
        let result = service.render_to_html(markdown).unwrap();
        
        println!("Image only result: {}", result);
        assert!(result.contains("<img"));
        assert!(result.contains("src=\"image.jpg\""));
        assert!(result.contains("alt=\"Alt Text\""));
    }

    #[test]
    fn test_comprehensive_table_and_link_support() {
        let service = MarkdownService::new();
        let markdown = r#"# Table and Link Rendering Test

## Complex Table with Various Content Types

| Feature | Status | Link | Code Example |
|---------|--------|------|--------------|
| **Tables** | ✅ Working | [Docs](https://spec.commonmark.org/0.30/#tables-extension) | `\| Header \| Header \|` |
| *Links* | ✅ Working | [External](https://example.com) | `[text](url)` |
| `Code` | ✅ Working | [Internal](/docs) | `` `code` `` |
| Images | ✅ Working | ![Icon](icon.png) | `![alt](src)` |

## Link Security and Accessibility

- [Safe External](https://secure.example.com) - should have security attributes
- [Internal Link](/internal/page) - should not have target="_blank"
- [Fragment](#section) - should work normally
- [Email](mailto:test@example.com) - should be allowed

## Table Accessibility Features

| Column A | Column B | Column C |
|----------|----------|----------|
| Data 1   | Data 2   | Data 3   |
| More     | Content  | Here     |

The table above should have:
- Responsive wrapper with class="table-responsive"
- Table with class="markdown-table"
- Headers with scope="col" attributes
"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        // Test table rendering
        assert!(result.contains("<table"));
        assert!(result.contains("class=\"table-responsive\""));
        assert!(result.contains("class=\"markdown-table\""));
        assert!(result.contains("scope=\"col\""));
        assert!(result.contains("<thead>"));
        assert!(result.contains("<tbody>"));
        assert!(result.contains("<th"));
        assert!(result.contains("<td"));
        
        // Test complex table content
        assert!(result.contains("<strong>Tables</strong>"));
        assert!(result.contains("<em>Links</em>"));
        assert!(result.contains("class=\"inline-code\""));
        assert!(result.contains("✅"));
        
        // Test link security and accessibility
        assert!(result.contains("href=\"https://secure.example.com\""));
        assert!(result.contains("rel=\"noopener noreferrer\""));
        assert!(result.contains("target=\"_blank\""));
        assert!(result.contains("href=\"/internal/page\""));
        assert!(result.contains("href=\"#section\""));
        assert!(result.contains("href=\"mailto:test@example.com\""));
        
        // Test images in tables
        assert!(result.contains("<img"));
        assert!(result.contains("src=\"icon.png\""));
        assert!(result.contains("alt=\"Icon\""));
        assert!(result.contains("loading=\"lazy\""));
        
        // Test that links within tables work correctly
        assert!(result.contains("href=\"https://spec.commonmark.org/0.30/#tables-extension\""));
        assert!(result.contains("href=\"https://example.com\""));
        assert!(result.contains("href=\"/docs\""));
        
        println!("Comprehensive test passed - all table and link features working correctly");
    }

    #[test]
    fn test_strikethrough_and_tasklist() {
        let service = MarkdownService::new();
        let markdown = "~~strikethrough~~ and\n- [x] completed task\n- [ ] incomplete task";
        let result = service.render_to_html(markdown).unwrap();
        
        assert!(result.contains("<del>") || result.contains("<s>"));
        assert!(result.contains("type=\"checkbox\""));
    }

    #[test]
    fn test_render_empty_markdown() {
        let service = MarkdownService::new();
        let result = service.render_to_html("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_render_with_fallback_success() {
        let service = MarkdownService::new();
        let markdown = "# Hello World";
        let result = service.render_to_html_with_fallback(markdown);
        assert!(result.contains("<h1>"));
        assert!(result.contains("Hello World"));
    }

    #[test]
    fn test_render_with_fallback_on_empty() {
        let service = MarkdownService::new();
        let result = service.render_to_html_with_fallback("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_highlight_code_with_fallback() {
        let service = MarkdownService::new();
        let code = "fn main() { println!(\"Hello\"); }";
        let result = service.highlight_code_with_fallback(code, "rust");
        assert!(result.contains("main"));
        assert!(result.contains("println"));
    }

    #[test]
    fn test_highlight_code_with_fallback_unknown_language() {
        let service = MarkdownService::new();
        let code = "some code";
        let result = service.highlight_code_with_fallback(code, "unknown_language");
        assert!(result.contains("some code"));
    }

    #[test]
    fn test_highlight_empty_code() {
        let service = MarkdownService::new();
        let result = service.highlight_code("", "rust").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_enhanced_table_rendering() {
        let service = MarkdownService::new();
        let markdown = r#"| Name | Age | City |
|------|-----|------|
| John | 25  | NYC  |
| Jane | 30  | LA   |"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        // Check for table structure
        assert!(result.contains("<table"));
        assert!(result.contains("<thead>"));
        assert!(result.contains("<tbody>"));
        assert!(result.contains("<th"));
        assert!(result.contains("<td"));
        
        // Check for accessibility enhancements
        assert!(result.contains("class=\"table-responsive\""));
        assert!(result.contains("class=\"markdown-table\""));
        assert!(result.contains("scope=\"col\""));
        
        // Check content
        assert!(result.contains("John"));
        assert!(result.contains("Jane"));
        assert!(result.contains("NYC"));
        assert!(result.contains("LA"));
    }

    #[test]
    fn test_safe_link_rendering() {
        let service = MarkdownService::new();
        let markdown = "[Safe Link](https://example.com) and [Internal Link](/page)";
        let result = service.render_to_html(markdown).unwrap();
        
        // Check for proper link attributes
        assert!(result.contains("href=\"https://example.com\""));
        assert!(result.contains("href=\"/page\""));
        assert!(result.contains("rel=\"noopener noreferrer\""));
        assert!(result.contains("target=\"_blank\""));
    }

    #[test]
    fn test_dangerous_link_blocking() {
        let service = MarkdownService::new();
        let markdown = "[Dangerous](javascript:alert('xss')) and [Data](data:text/html,<script>alert('xss')</script>)";
        let result = service.render_to_html(markdown).unwrap();
        
        println!("Dangerous link result: {}", result);
        // Should block dangerous protocols - ammonia strips dangerous href attributes entirely
        assert!(!result.contains("javascript:alert"));
        assert!(!result.contains("data:text/html"));
        // Links should still be present but without dangerous href attributes
        assert!(result.contains("<a"));
        assert!(result.contains("Dangerous"));
        assert!(result.contains("Data"));
    }

    #[test]
    fn test_image_rendering_with_security() {
        let service = MarkdownService::new();
        let markdown = "![Alt Text](https://example.com/image.jpg \"Title\")";
        let result = service.render_to_html(markdown).unwrap();
        
        // Check for proper image attributes
        assert!(result.contains("src=\"https://example.com/image.jpg\""));
        assert!(result.contains("alt=\"Alt Text\""));
        assert!(result.contains("title=\"Title\""));
        assert!(result.contains("loading=\"lazy\""));
    }

    #[test]
    fn test_complex_table_with_formatting() {
        let service = MarkdownService::new();
        let markdown = r#"| Feature | **Status** | `Code` |
|---------|------------|--------|
| Tables  | ✅ Working | `table` |
| Links   | ✅ Working | `[link](url)` |
| *Emphasis* | ✅ Working | `*text*` |"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        // Check table structure
        assert!(result.contains("<table"));
        assert!(result.contains("class=\"markdown-table\""));
        
        // Check that formatting within table cells is preserved
        assert!(result.contains("<strong>Status</strong>"));
        assert!(result.contains("<code"));
        assert!(result.contains("<em>Emphasis</em>"));
        assert!(result.contains("✅"));
    }

    #[test]
    fn test_table_accessibility_features() {
        let service = MarkdownService::new();
        let markdown = "| Header 1 | Header 2 |\n|----------|----------|\n| Data 1   | Data 2   |";
        let result = service.render_to_html(markdown).unwrap();
        
        // Check accessibility features
        assert!(result.contains("scope=\"col\""));
        assert!(result.contains("class=\"table-responsive\""));
        assert!(result.contains("class=\"markdown-table\""));
    }

    #[test]
    fn test_mixed_content_rendering() {
        let service = MarkdownService::new();
        let markdown = r#"# Title

Here's a [link](https://example.com) and some **bold text**.

| Column 1 | Column 2 |
|----------|----------|
| Data     | More data |

```rust
fn hello() {
    println!("Hello, world!");
}
```

![Image](https://example.com/image.jpg)"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        // Check all elements are present and properly formatted
        assert!(result.contains("<h1>Title</h1>"));
        assert!(result.contains("rel=\"noopener noreferrer\""));
        assert!(result.contains("<strong>bold text</strong>"));
        assert!(result.contains("class=\"markdown-table\""));
        assert!(result.contains("class=\"highlight language-rust\""));
        assert!(result.contains("loading=\"lazy\""));
    }

    #[test]
    fn test_complex_table_scenarios() {
        let service = MarkdownService::new();
        
        // Test table with various content types
        let markdown = r#"| **Header** | `Code` | [Link](https://example.com) | ![Img](test.jpg) |
|------------|--------|------------------------------|-------------------|
| *Italic*   | `var x = 1;` | [Internal](/page) | ![Alt](local.png) |
| ~~Strike~~ | ```js\ncode\n``` | [External](https://test.com) | Image text |
| Normal     | Inline `code` | [Email](mailto:test@example.com) | No image |"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        // Verify table structure
        assert!(result.contains("class=\"table-responsive\""));
        assert!(result.contains("class=\"markdown-table\""));
        assert!(result.contains("scope=\"col\""));
        
        // Verify content formatting within table
        assert!(result.contains("<strong>Header</strong>"));
        assert!(result.contains("<em>Italic</em>"));
        assert!(result.contains("<del>Strike</del>") || result.contains("<s>Strike</s>"));
        assert!(result.contains("class=\"inline-code\""));
        
        // Verify links in table
        assert!(result.contains("href=\"https://example.com\""));
        assert!(result.contains("href=\"/page\""));
        assert!(result.contains("href=\"mailto:test@example.com\""));
        assert!(result.contains("rel=\"noopener noreferrer\""));
        
        // Verify images in table
        assert!(result.contains("src=\"test.jpg\""));
        assert!(result.contains("src=\"local.png\""));
        assert!(result.contains("loading=\"lazy\""));
    }

    #[test]
    fn test_nested_table_content() {
        let service = MarkdownService::new();
        let markdown = r#"| Feature | Description | Example |
|---------|-------------|---------|
| **Bold** | Make text bold | `**text**` |
| *Italic* | Make text italic | `*text*` |
| Links | Create hyperlinks | `[text](url)` |
| Code | Inline code | `` `code` `` |"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        // Check that nested markdown is properly rendered within table cells
        assert!(result.contains("<strong>Bold</strong>"));
        assert!(result.contains("<em>Italic</em>"));
        assert!(result.contains("class=\"inline-code\""));
        assert!(result.contains("**text**"));
        assert!(result.contains("*text*"));
    }

    #[test]
    fn test_table_edge_cases() {
        let service = MarkdownService::new();
        
        // Test empty cells
        let markdown1 = "| A | B | C |\n|---|---|---|\n| 1 |   | 3 |\n|   | 2 |   |";
        let result1 = service.render_to_html(markdown1).unwrap();
        assert!(result1.contains("<td></td>"));
        
        // Test single column table
        let markdown2 = "| Single |\n|--------|\n| Row 1  |\n| Row 2  |";
        let result2 = service.render_to_html(markdown2).unwrap();
        assert!(result2.contains("<th"));
        assert!(result2.contains("Single"));
        
        // Test table with alignment (if supported)
        let markdown3 = "| Left | Center | Right |\n|:-----|:------:|------:|\n| L1   | C1     | R1    |";
        let result3 = service.render_to_html(markdown3).unwrap();
        assert!(result3.contains("<table"));
        assert!(result3.contains("Left"));
        assert!(result3.contains("Center"));
        assert!(result3.contains("Right"));
    }

    #[test]
    fn test_link_accessibility_and_security() {
        let service = MarkdownService::new();
        let markdown = r#"Various link types:
- [External HTTPS](https://secure.example.com)
- [External HTTP](http://example.com)
- [Internal absolute](/internal/page)
- [Internal relative](../page)
- [Fragment link](#section)
- [Email](mailto:user@example.com)
- [Phone](tel:+1234567890)
- [Dangerous JS](javascript:alert('xss'))
- [Dangerous Data](data:text/html,<script>alert('xss')</script>)"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        println!("Link accessibility result: {}", result);
        
        // Check external links have security attributes
        assert!(result.contains("href=\"https://secure.example.com\""));
        assert!(result.contains("href=\"http://example.com\""));
        assert!(result.contains("rel=\"noopener noreferrer\""));
        assert!(result.contains("target=\"_blank\""));
        
        // Check internal links don't have external attributes
        assert!(result.contains("href=\"/internal/page\""));
        assert!(result.contains("href=\"../page\""));
        assert!(result.contains("href=\"#section\""));
        
        // Check safe protocols are allowed
        assert!(result.contains("href=\"mailto:user@example.com\""));
        
        // Check dangerous protocols are blocked
        assert!(!result.contains("javascript:alert"));
        assert!(!result.contains("data:text/html"));
        // Dangerous links should be stripped by ammonia, not replaced with #
        assert!(result.contains("<a") && result.contains("Dangerous"));
    }

    #[test]
    fn test_image_security_and_accessibility() {
        let service = MarkdownService::new();
        let markdown = r#"Image tests:
![Alt text](https://example.com/image.jpg "Title text")
![Local image](./local.jpg)
![Relative image](../images/test.png)
![No title](image.gif)
![Dangerous](javascript:alert('xss'))
![Data URL](data:image/svg+xml;base64,PHN2Zw==)"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        println!("Image security result: {}", result);
        
        // Check proper image attributes
        assert!(result.contains("src=\"https://example.com/image.jpg\""));
        assert!(result.contains("alt=\"Alt text\""));
        assert!(result.contains("title=\"Title text\""));
        assert!(result.contains("loading=\"lazy\""));
        
        // Check local images are allowed
        assert!(result.contains("src=\"./local.jpg\""));
        assert!(result.contains("src=\"../images/test.png\""));
        
        // Check dangerous URLs are blocked - ammonia strips them entirely
        assert!(!result.contains("javascript:alert"));
        // Data URLs are also stripped by ammonia for security (which is good)
        assert!(!result.contains("data:image/svg+xml;base64,PHN2Zw=="));
        // But the images should still be present without src attributes
        assert!(result.contains("alt=\"Dangerous\""));
        assert!(result.contains("alt=\"Data URL\""));
    }

    #[test]
    fn test_complex_markdown_combinations() {
        let service = MarkdownService::new();
        let markdown = r#"# Complex Document

## Table with Links and Code

| Component | Status | Documentation | Example |
|-----------|--------|---------------|---------|
| **Parser** | ✅ Complete | [Docs](https://docs.rs/pulldown-cmark) | `Parser::new()` |
| *Renderer* | 🚧 In Progress | [Guide](/guide) | `render_to_html()` |
| ~~Old API~~ | ❌ Deprecated | [Archive](https://old.example.com) | `legacy_render()` |

## Code with Links

```rust
// See documentation: https://docs.rs/pulldown-cmark
use pulldown_cmark::{Parser, html};

fn render_markdown(input: &str) -> String {
    let parser = Parser::new(input);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
```

## Links in Lists

1. [Primary documentation](https://example.com/docs)
2. [API reference](https://api.example.com)
   - [Getting started](/getting-started)
   - [Advanced usage](/advanced)
3. [Community forum](https://forum.example.com)

> **Note**: External links open in new tabs for security.

![Architecture Diagram](https://example.com/diagram.png "System Architecture")"#;
        
        let result = service.render_to_html(markdown).unwrap();
        
        // Verify document structure
        assert!(result.contains("<h1>Complex Document</h1>"));
        assert!(result.contains("<h2>Table with Links and Code</h2>"));
        
        // Verify table with complex content
        assert!(result.contains("class=\"markdown-table\""));
        assert!(result.contains("<strong>Parser</strong>"));
        assert!(result.contains("<em>Renderer</em>"));
        assert!(result.contains("<del>Old API</del>") || result.contains("<s>Old API</s>"));
        assert!(result.contains("class=\"inline-code\""));
        
        // Verify links in table
        assert!(result.contains("href=\"https://docs.rs/pulldown-cmark\""));
        assert!(result.contains("href=\"/guide\""));
        assert!(result.contains("rel=\"noopener noreferrer\""));
        
        // Verify code block
        assert!(result.contains("class=\"highlight language-rust\""));
        assert!(result.contains("pulldown_cmark"));
        
        // Verify lists with links
        assert!(result.contains("<ol>"));
        assert!(result.contains("href=\"https://example.com/docs\""));
        assert!(result.contains("href=\"/getting-started\""));
        
        // Verify blockquote
        assert!(result.contains("<blockquote>"));
        
        // Verify image
        assert!(result.contains("src=\"https://example.com/diagram.png\""));
        assert!(result.contains("alt=\"Architecture Diagram\""));
        assert!(result.contains("title=\"System Architecture\""));
    }
}