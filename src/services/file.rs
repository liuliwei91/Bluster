use serde::{Deserialize, Serialize};
use gray_matter::{Matter, engine::YAML};
use crate::models::Article;

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("File format not supported: {0}")]
    UnsupportedFormat(String),
    #[error("Front matter parsing failed: {0}")]
    #[allow(dead_code)] // Reserved for future front matter error handling
    FrontMatterError(String),
    #[error("File size too large: {0} bytes")]
    FileTooLarge(usize),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkdownFile {
    pub title: String,
    pub content: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FrontMatterData {
    title: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

pub struct FileService;

impl FileService {
    pub fn new() -> Self {
        Self
    }

    pub fn validate_file_size(content: &str, max_size_mb: usize) -> Result<(), FileError> {
        let size_bytes = content.len();
        let max_size_bytes = max_size_mb * 1024 * 1024;
        
        if size_bytes > max_size_bytes {
            return Err(FileError::FileTooLarge(size_bytes));
        }
        
        Ok(())
    }

    pub fn validate_file_extension(filename: &str) -> Result<(), FileError> {
        if !filename.to_lowercase().ends_with(".md") && !filename.to_lowercase().ends_with(".markdown") {
            return Err(FileError::UnsupportedFormat(
                format!("File must have .md or .markdown extension, got: {}", filename)
            ));
        }
        Ok(())
    }

    pub fn parse_markdown_file(content: &str) -> Result<MarkdownFile, FileError> {
        // Validate file format - ensure it's not empty and contains valid UTF-8
        if content.trim().is_empty() {
            return Err(FileError::UnsupportedFormat("Empty file".to_string()));
        }

        // Check for extremely large content that might cause issues
        if content.len() > 50 * 1024 * 1024 { // 50MB limit for parsing
            return Err(FileError::FileTooLarge(content.len()));
        }

        // Use gray_matter to parse front matter
        let matter = Matter::<YAML>::new();
        
        // Attempt to parse with structured front matter
        match matter.parse_with_struct::<FrontMatterData>(content) {
            Some(parsed) => {
                let front_matter = parsed.data;
                
                // Validate and sanitize title
                let title = front_matter.title
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| {
                        // Try to extract title from first heading in content
                        Self::extract_title_from_content(&parsed.content)
                    });

                // Validate content is not empty after front matter removal
                if parsed.content.trim().is_empty() {
                    return Err(FileError::UnsupportedFormat(
                        "File contains only front matter without content".to_string()
                    ));
                }

                Ok(MarkdownFile {
                    title: Self::sanitize_title(&title),
                    content: parsed.content.to_string(),
                    created_at: front_matter.created_at,
                    updated_at: front_matter.updated_at,
                })
            }
            None => {
                // If structured parsing fails, try basic parsing
                let parsed = matter.parse(content);
                
                let title = Self::extract_title_from_content(&parsed.content);
                
                // Validate content exists
                if parsed.content.trim().is_empty() {
                    return Err(FileError::UnsupportedFormat(
                        "File contains no readable content".to_string()
                    ));
                }

                Ok(MarkdownFile {
                    title: Self::sanitize_title(&title),
                    content: parsed.content.to_string(),
                    created_at: None,
                    updated_at: None,
                })
            }
        }
    }

    /// Extract title from markdown content, looking for first heading
    fn extract_title_from_content(content: &str) -> String {
        content
            .lines()
            .find(|line| line.starts_with("# "))
            .map(|line| {
                line.trim_start_matches("# ")
                    .trim()
                    .to_string()
            })
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    /// Sanitize title to ensure it's safe and reasonable
    fn sanitize_title(title: &str) -> String {
        let sanitized = title.trim();
        if sanitized.is_empty() {
            "Untitled".to_string()
        } else if sanitized.len() > 200 {
            // Truncate very long titles
            format!("{}...", &sanitized[..197])
        } else {
            sanitized.to_string()
        }
    }

    pub fn generate_markdown_export(article: &Article) -> Result<String, FileError> {
        // Validate article data
        if article.title.trim().is_empty() {
            return Err(FileError::UnsupportedFormat("Article title is empty".to_string()));
        }

        if article.content.trim().is_empty() {
            return Err(FileError::UnsupportedFormat("Article content is empty".to_string()));
        }

        // Escape quotes in title for YAML front matter
        let escaped_title = article.title.replace("\"", "\\\"");
        
        let export_content = format!(
            "---\ntitle: \"{}\"\ncreated_at: \"{}\"\nupdated_at: \"{}\"\n---\n\n{}",
            escaped_title, article.created_at, article.updated_at, article.content
        );

        // Validate the generated content isn't too large
        if export_content.len() > 100 * 1024 * 1024 { // 100MB limit
            return Err(FileError::FileTooLarge(export_content.len()));
        }

        Ok(export_content)
    }

    /// Generate markdown export with fallback on error
    #[allow(dead_code)] // Reserved for future use in export functionality
    pub fn generate_markdown_export_with_fallback(article: &Article) -> String {
        match Self::generate_markdown_export(article) {
            Ok(content) => content,
            Err(e) => {
                log::warn!("Failed to generate proper markdown export, using fallback: {}", e);
                // Fallback: simple format without front matter
                format!("# {}\n\n{}", article.title, article.content)
            }
        }
    }

    pub fn sanitize_filename(title: &str) -> Result<String, FileError> {
        if title.trim().is_empty() {
            return Err(FileError::UnsupportedFormat("Empty filename".to_string()));
        }

        let sanitized = title
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                c if c.is_control() => '_', // Replace control characters
                c => c,
            })
            .collect::<String>()
            .trim()
            .to_string();

        // Ensure filename isn't too long (most filesystems have 255 char limit)
        let final_name = if sanitized.len() > 200 {
            format!("{}...", &sanitized[..197])
        } else {
            sanitized
        };

        // Ensure we don't have an empty result
        if final_name.is_empty() {
            Ok("untitled".to_string())
        } else {
            Ok(final_name)
        }
    }

    /// Sanitize filename with fallback on error
    pub fn sanitize_filename_with_fallback(title: &str) -> String {
        match Self::sanitize_filename(title) {
            Ok(filename) => filename,
            Err(e) => {
                log::warn!("Failed to sanitize filename '{}', using fallback: {}", title, e);
                "untitled".to_string()
            }
        }
    }
}

impl Default for FileService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_with_front_matter() {
        let content = r#"---
title: "Test Article"
created_at: "2024-01-01"
updated_at: "2024-01-02"
---

# Hello World

This is test content."#;

        let result = FileService::parse_markdown_file(content).unwrap();
        assert_eq!(result.title, "Test Article");
        assert_eq!(result.created_at, Some("2024-01-01".to_string()));
        assert_eq!(result.updated_at, Some("2024-01-02".to_string()));
        assert!(result.content.contains("# Hello World"));
        assert!(result.content.contains("This is test content."));
    }

    #[test]
    fn test_parse_markdown_without_front_matter() {
        let content = "# Hello World\n\nThis is test content.";
        let result = FileService::parse_markdown_file(content).unwrap();
        
        assert_eq!(result.title, "Hello World"); // Should extract from first heading
        assert_eq!(result.created_at, None);
        assert_eq!(result.updated_at, None);
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_parse_markdown_without_front_matter_no_heading() {
        let content = "This is just plain content without heading.";
        let result = FileService::parse_markdown_file(content).unwrap();
        
        assert_eq!(result.title, "Untitled");
        assert_eq!(result.created_at, None);
        assert_eq!(result.updated_at, None);
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_sanitize_filename() {
        let dangerous_name = "My Article: <Test>?";
        let safe_name = FileService::sanitize_filename(dangerous_name).unwrap();
        assert_eq!(safe_name, "My Article_ _Test__");
    }

    #[test]
    fn test_sanitize_filename_with_fallback() {
        let dangerous_name = "My Article: <Test>?";
        let safe_name = FileService::sanitize_filename_with_fallback(dangerous_name);
        assert_eq!(safe_name, "My Article_ _Test__");
        
        // Test empty filename fallback
        let empty_name = "";
        let fallback_name = FileService::sanitize_filename_with_fallback(empty_name);
        assert_eq!(fallback_name, "untitled");
    }

    #[test]
    fn test_generate_markdown_export() {
        use crate::models::Article;
        
        let article = Article {
            id: 1,
            title: "Test Title".to_string(),
            content: "Test content".to_string(),
            author_id: Some(1),
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-02".to_string(),
        };
        
        let result = FileService::generate_markdown_export(&article).unwrap();
        
        assert!(result.contains("title: \"Test Title\""));
        assert!(result.contains("created_at: \"2024-01-01\""));
        assert!(result.contains("updated_at: \"2024-01-02\""));
        assert!(result.contains("Test content"));
    }

    #[test]
    fn test_validate_file_size_success() {
        let content = "Small content";
        let result = FileService::validate_file_size(content, 5); // 5MB limit
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_file_size_failure() {
        let large_content = "x".repeat(6 * 1024 * 1024); // 6MB content
        let result = FileService::validate_file_size(&large_content, 5); // 5MB limit
        assert!(result.is_err());
        match result.unwrap_err() {
            FileError::FileTooLarge(size) => assert_eq!(size, 6 * 1024 * 1024),
            _ => panic!("Expected FileTooLarge error"),
        }
    }

    #[test]
    fn test_validate_file_extension_success() {
        assert!(FileService::validate_file_extension("test.md").is_ok());
        assert!(FileService::validate_file_extension("test.markdown").is_ok());
        assert!(FileService::validate_file_extension("TEST.MD").is_ok());
    }

    #[test]
    fn test_validate_file_extension_failure() {
        let result = FileService::validate_file_extension("test.txt");
        assert!(result.is_err());
        match result.unwrap_err() {
            FileError::UnsupportedFormat(msg) => assert!(msg.contains("test.txt")),
            _ => panic!("Expected UnsupportedFormat error"),
        }
    }

    #[test]
    fn test_parse_empty_file() {
        let result = FileService::parse_markdown_file("");
        assert!(result.is_err());
        match result.unwrap_err() {
            FileError::UnsupportedFormat(msg) => assert_eq!(msg, "Empty file"),
            _ => panic!("Expected UnsupportedFormat error"),
        }
    }

    #[test]
    fn test_generate_markdown_export_with_empty_title() {
        use crate::models::Article;
        
        let article = Article {
            id: 1,
            title: "".to_string(), // Empty title
            content: "Test content".to_string(),
            author_id: Some(1),
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-02".to_string(),
        };
        
        let result = FileService::generate_markdown_export(&article);
        assert!(result.is_err());
        match result.unwrap_err() {
            FileError::UnsupportedFormat(msg) => assert!(msg.contains("title is empty")),
            _ => panic!("Expected UnsupportedFormat error"),
        }
    }

    #[test]
    fn test_generate_markdown_export_with_fallback() {
        use crate::models::Article;
        
        let article = Article {
            id: 1,
            title: "".to_string(), // This will cause an error
            content: "Test content".to_string(),
            author_id: Some(1),
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-02".to_string(),
        };
        
        let result = FileService::generate_markdown_export_with_fallback(&article);
        // Should fallback to simple format
        assert!(result.contains("# "));
        assert!(result.contains("Test content"));
    }

    #[test]
    fn test_sanitize_filename_empty() {
        let result = FileService::sanitize_filename("");
        assert!(result.is_err());
        match result.unwrap_err() {
            FileError::UnsupportedFormat(msg) => assert!(msg.contains("Empty filename")),
            _ => panic!("Expected UnsupportedFormat error"),
        }
    }

    #[test]
    fn test_sanitize_filename_very_long() {
        let long_name = "a".repeat(300);
        let result = FileService::sanitize_filename(&long_name).unwrap();
        assert!(result.len() <= 200);
        assert!(result.ends_with("..."));
    }
}