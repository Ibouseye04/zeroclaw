use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Maximum characters of body text returned to the LLM.
const MAX_BODY_CHARS: usize = 16_000;

/// Fetch a web page and return its content as readable text.
///
/// Uses the same URL-validation and domain-allowlist logic as `browser_open`
/// but fetches programmatically (no display required).
pub struct WebFetchTool {
    security: Arc<SecurityPolicy>,
    allowed_domains: Vec<String>,
    client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new(security: Arc<SecurityPolicy>, allowed_domains: Vec<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();

        Self {
            security,
            allowed_domains: super::browser_open::normalize_allowed_domains_pub(allowed_domains),
            client,
        }
    }

    fn validate_url(&self, raw_url: &str) -> anyhow::Result<String> {
        let url = raw_url.trim();

        if url.is_empty() {
            anyhow::bail!("URL cannot be empty");
        }

        if url.chars().any(char::is_whitespace) {
            anyhow::bail!("URL cannot contain whitespace");
        }

        if !url.starts_with("https://") {
            anyhow::bail!("Only https:// URLs are allowed");
        }

        if self.allowed_domains.is_empty() {
            anyhow::bail!(
                "No allowed_domains configured. Add [browser].allowed_domains in config.toml"
            );
        }

        let host = super::browser_open::extract_host_pub(url)?;

        if super::browser_open::is_private_or_local_host_pub(&host) {
            anyhow::bail!("Blocked local/private host: {host}");
        }

        if !super::browser_open::host_matches_allowlist_pub(&host, &self.allowed_domains) {
            anyhow::bail!("Host '{host}' is not in browser.allowed_domains");
        }

        Ok(url.to_string())
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a web page or API endpoint and return its content as text. \
         Works on headless systems. HTTPS only, domain allowlist enforced."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "HTTPS URL to fetch"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;

        if !self.security.can_act() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Action blocked: autonomy is read-only".into()),
            });
        }

        if !self.security.record_action() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Action blocked: rate limit exceeded".into()),
            });
        }

        let validated = match self.validate_url(url) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(e.to_string()),
                })
            }
        };

        match self.client.get(&validated).send().await {
            Ok(resp) => {
                let status = resp.status();
                let content_type = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();

                match resp.text().await {
                    Ok(body) => {
                        let text = if content_type.contains("json") {
                            // JSON — return as-is (pretty-print if compact)
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
                                serde_json::to_string_pretty(&val).unwrap_or(body)
                            } else {
                                body
                            }
                        } else if content_type.contains("html") {
                            strip_html(&body)
                        } else {
                            // Plain text, XML, etc.
                            body
                        };

                        let truncated = if text.len() > MAX_BODY_CHARS {
                            format!(
                                "{}\n\n[Truncated — showing first {MAX_BODY_CHARS} of {} chars]",
                                &text[..MAX_BODY_CHARS],
                                text.len()
                            )
                        } else {
                            text
                        };

                        Ok(ToolResult {
                            success: status.is_success(),
                            output: format!("HTTP {status}\n\n{truncated}"),
                            error: if status.is_success() {
                                None
                            } else {
                                Some(format!("HTTP {status}"))
                            },
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to read response body: {e}")),
                    }),
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Request failed: {e}")),
            }),
        }
    }
}

/// Minimal HTML-to-text conversion: strip tags, decode common entities,
/// collapse whitespace. Good enough for LLM consumption without adding
/// a heavy dependency.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut tag_buf = String::new();

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }

        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag_lower = tag_buf.to_lowercase();
                let tag_name = tag_lower.split_whitespace().next().unwrap_or("");

                if tag_name == "script" {
                    in_script = true;
                } else if tag_name == "/script" {
                    in_script = false;
                } else if tag_name == "style" {
                    in_style = true;
                } else if tag_name == "/style" {
                    in_style = false;
                }

                // Block-level elements get newlines
                if matches!(
                    tag_name,
                    "br" | "br/"
                        | "p"
                        | "/p"
                        | "div"
                        | "/div"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "/h1"
                        | "/h2"
                        | "/h3"
                        | "/h4"
                        | "/h5"
                        | "/h6"
                        | "li"
                        | "tr"
                        | "/tr"
                ) {
                    out.push('\n');
                }
            } else {
                tag_buf.push(ch);
            }
            continue;
        }

        if in_script || in_style {
            continue;
        }

        out.push(ch);
    }

    // Decode common HTML entities
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/");

    // Collapse whitespace: multiple blanks → single, multiple newlines → double
    let mut result = String::with_capacity(out.len());
    let mut prev_newline = false;
    let mut prev_space = false;

    for ch in out.chars() {
        if ch == '\n' {
            if !prev_newline {
                result.push('\n');
            }
            prev_newline = true;
            prev_space = false;
        } else if ch.is_whitespace() {
            if !prev_space && !prev_newline {
                result.push(' ');
            }
            prev_space = true;
        } else {
            prev_newline = false;
            prev_space = false;
            result.push(ch);
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{AutonomyLevel, SecurityPolicy};

    fn test_tool(domains: Vec<&str>) -> WebFetchTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            ..SecurityPolicy::default()
        });
        WebFetchTool::new(security, domains.into_iter().map(String::from).collect())
    }

    #[test]
    fn validate_accepts_allowed_domain() {
        let tool = test_tool(vec!["example.com"]);
        assert!(tool.validate_url("https://example.com/page").is_ok());
    }

    #[test]
    fn validate_accepts_subdomain() {
        let tool = test_tool(vec!["example.com"]);
        assert!(tool.validate_url("https://api.example.com/v1").is_ok());
    }

    #[test]
    fn validate_wildcard_allows_any_domain() {
        let tool = test_tool(vec!["*"]);
        assert!(tool.validate_url("https://anything.com/page").is_ok());
    }

    #[test]
    fn validate_wildcard_still_blocks_private_ip() {
        let tool = test_tool(vec!["*"]);
        let err = tool.validate_url("https://127.0.0.1").unwrap_err();
        assert!(err.to_string().contains("local/private"));
    }

    #[test]
    fn validate_rejects_http() {
        let tool = test_tool(vec!["example.com"]);
        let err = tool.validate_url("http://example.com").unwrap_err();
        assert!(err.to_string().contains("https://"));
    }

    #[test]
    fn validate_rejects_unlisted_domain() {
        let tool = test_tool(vec!["example.com"]);
        let err = tool.validate_url("https://evil.com").unwrap_err();
        assert!(err.to_string().contains("allowed_domains"));
    }

    #[test]
    fn validate_rejects_localhost() {
        let tool = test_tool(vec!["localhost"]);
        let err = tool.validate_url("https://localhost").unwrap_err();
        assert!(err.to_string().contains("local/private"));
    }

    #[test]
    fn validate_rejects_private_ip() {
        let tool = test_tool(vec!["192.168.1.1"]);
        let err = tool.validate_url("https://192.168.1.1").unwrap_err();
        assert!(err.to_string().contains("local/private"));
    }

    #[test]
    fn validate_rejects_empty_allowlist() {
        let tool = test_tool(vec![]);
        let err = tool.validate_url("https://example.com").unwrap_err();
        assert!(err.to_string().contains("allowed_domains"));
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            ..SecurityPolicy::default()
        });
        let tool = WebFetchTool::new(security, vec!["example.com".into()]);
        let result = tool
            .execute(json!({"url": "https://example.com"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("read-only"));
    }

    #[tokio::test]
    async fn execute_blocks_rate_limited() {
        let security = Arc::new(SecurityPolicy {
            max_actions_per_hour: 0,
            ..SecurityPolicy::default()
        });
        let tool = WebFetchTool::new(security, vec!["example.com".into()]);
        let result = tool
            .execute(json!({"url": "https://example.com"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("rate limit"));
    }

    #[test]
    fn strip_html_basic() {
        assert_eq!(strip_html("<p>Hello <b>world</b></p>"), "Hello world");
    }

    #[test]
    fn strip_html_entities() {
        assert_eq!(strip_html("&lt;tag&gt; &amp; &quot;stuff&quot;"), "<tag> & \"stuff\"");
    }

    #[test]
    fn strip_html_script_removed() {
        let html = "before<script>var x = 1;</script>after";
        assert_eq!(strip_html(html), "beforeafter");
    }

    #[test]
    fn strip_html_style_removed() {
        let html = "before<style>.x{color:red}</style>after";
        assert_eq!(strip_html(html), "beforeafter");
    }

    #[test]
    fn strip_html_block_elements_get_newlines() {
        let html = "<h1>Title</h1><p>Para 1</p><p>Para 2</p>";
        let text = strip_html(html);
        assert!(text.contains("Title\n"));
        assert!(text.contains("Para 1\n"));
    }

    #[test]
    fn strip_html_collapses_whitespace() {
        let html = "  lots   of    spaces  ";
        assert_eq!(strip_html(html), "lots of spaces");
    }

    #[test]
    fn strip_html_empty() {
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn strip_html_plain_text_passthrough() {
        assert_eq!(strip_html("no tags here"), "no tags here");
    }

    #[test]
    fn tool_name() {
        let tool = test_tool(vec!["example.com"]);
        assert_eq!(tool.name(), "web_fetch");
    }

    #[test]
    fn tool_schema_has_url() {
        let tool = test_tool(vec!["example.com"]);
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["url"].is_object());
    }
}
