use super::traits::{Tool, ToolResult};
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Maximum number of search results returned.
const MAX_RESULTS: usize = 8;

/// Search the web via DuckDuckGo HTML and return results.
///
/// No API key required. Works on headless systems (Pi, etc.).
pub struct WebSearchTool {
    security: Arc<SecurityPolicy>,
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_default();

        Self { security, client }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web and return a list of results (titles, URLs, snippets). \
         Use this when you need to find information, look something up, or answer \
         questions about current events. No API key required."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;

        if query.trim().is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Search query cannot be empty".into()),
            });
        }

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

        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoded(query)
        );

        match self.client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Search request failed: HTTP {status}")),
                    });
                }

                match resp.text().await {
                    Ok(body) => {
                        let results = parse_ddg_html(&body);
                        if results.is_empty() {
                            return Ok(ToolResult {
                                success: true,
                                output: "No results found.".into(),
                                error: None,
                            });
                        }

                        let mut output = String::new();
                        for (i, r) in results.iter().take(MAX_RESULTS).enumerate() {
                            output.push_str(&format!(
                                "{}. {}\n   {}\n   {}\n\n",
                                i + 1,
                                r.title,
                                r.url,
                                r.snippet
                            ));
                        }

                        Ok(ToolResult {
                            success: true,
                            output: output.trim_end().to_string(),
                            error: None,
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!("Failed to read search response: {e}")),
                    }),
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Search request failed: {e}")),
            }),
        }
    }
}

/// Minimal percent-encoding for query parameters.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Parse DuckDuckGo HTML search results page.
///
/// DDG HTML results live in elements with class "result__a" (title/link)
/// and "result__snippet" (description). We parse these with simple string
/// scanning — no heavy HTML parser dependency.
fn parse_ddg_html(html: &str) -> Vec<SearchResult> {
    let mut results = Vec::new();

    // Each result is in a div with class "result results_links results_links_deep web-result"
    // The title link has class "result__a" and the snippet has class "result__snippet"
    //
    // Strategy: find each 'class="result__a"' occurrence, extract the href and text,
    // then look ahead for the snippet.

    let mut pos = 0;
    while let Some(idx) = html[pos..].find("class=\"result__a\"") {
        let abs = pos + idx;

        // Extract href from the <a> tag — scan backwards to find href="..."
        let tag_start = html[..abs].rfind('<').unwrap_or(abs);
        let href = extract_attr(&html[tag_start..], "href").unwrap_or_default();

        // The actual URL is often a DDG redirect; extract the uddg= param if present
        let url = if let Some(uddg_start) = href.find("uddg=") {
            let raw = &href[uddg_start + 5..];
            let end = raw.find('&').unwrap_or(raw.len());
            percent_decode(&raw[..end])
        } else if href.starts_with("http") {
            href.clone()
        } else {
            href.clone()
        };

        // Extract title text: everything between > and </a> after class="result__a"
        let after_class = &html[abs..];
        let title = if let Some(gt) = after_class.find('>') {
            let text_start = abs + gt + 1;
            if let Some(end_a) = html[text_start..].find("</a>") {
                strip_tags(&html[text_start..text_start + end_a])
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Find snippet: look for class="result__snippet" nearby
        let search_window = &html[abs..std::cmp::min(abs + 3000, html.len())];
        let snippet = if let Some(snip_idx) = search_window.find("class=\"result__snippet\"") {
            let snip_abs = abs + snip_idx;
            let after = &html[snip_abs..];
            if let Some(gt) = after.find('>') {
                let text_start = snip_abs + gt + 1;
                // Find closing tag (</a> or </td> or </div>)
                let remaining = &html[text_start..];
                let end = remaining
                    .find("</a>")
                    .or_else(|| remaining.find("</td>"))
                    .or_else(|| remaining.find("</div>"))
                    .unwrap_or(remaining.len().min(500));
                strip_tags(&remaining[..end]).trim().to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        if !title.is_empty() && !url.is_empty() && url.starts_with("http") {
            results.push(SearchResult {
                title: title.trim().to_string(),
                url,
                snippet,
            });
        }

        pos = abs + 17; // skip past current match
    }

    results
}

/// Extract an attribute value from an HTML tag fragment.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    let start = tag.find(&pattern)? + pattern.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

/// Minimal percent-decoding.
fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(val);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Strip HTML tags from a string fragment (minimal version).
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    // Decode common entities
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&#x27;", "'")
        .replace("&#x2F;", "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::{AutonomyLevel, SecurityPolicy};

    fn test_tool() -> WebSearchTool {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::Supervised,
            ..SecurityPolicy::default()
        });
        WebSearchTool::new(security)
    }

    #[test]
    fn tool_name() {
        assert_eq!(test_tool().name(), "web_search");
    }

    #[test]
    fn tool_schema_has_query() {
        let schema = test_tool().parameters_schema();
        assert!(schema["properties"]["query"].is_object());
    }

    #[test]
    fn tool_has_description() {
        assert!(!test_tool().description().is_empty());
    }

    #[test]
    fn urlencoded_spaces() {
        assert_eq!(urlencoded("hello world"), "hello+world");
    }

    #[test]
    fn urlencoded_special_chars() {
        assert_eq!(urlencoded("a&b=c"), "a%26b%3Dc");
    }

    #[test]
    fn urlencoded_preserves_safe() {
        assert_eq!(urlencoded("abc-123_x.y~z"), "abc-123_x.y~z");
    }

    #[test]
    fn percent_decode_basic() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("a%26b"), "a&b");
    }

    #[test]
    fn strip_tags_basic() {
        assert_eq!(strip_tags("<b>bold</b> text"), "bold text");
    }

    #[test]
    fn extract_attr_basic() {
        assert_eq!(
            extract_attr(r#"<a href="https://example.com" class="link">"#, "href"),
            Some("https://example.com".into())
        );
    }

    #[test]
    fn extract_attr_missing() {
        assert_eq!(extract_attr("<a class=\"link\">", "href"), None);
    }

    #[test]
    fn parse_ddg_html_extracts_results() {
        let html = r#"
        <div class="result">
            <a rel="nofollow" class="result__a" href="https://example.com">
                Example Title
            </a>
            <a class="result__snippet">This is a snippet about Example.</a>
        </div>
        <div class="result">
            <a rel="nofollow" class="result__a" href="https://other.com/page">
                Other Page
            </a>
            <a class="result__snippet">Another snippet here.</a>
        </div>
        "#;
        let results = parse_ddg_html(html);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title.trim(), "Example Title");
        assert_eq!(results[0].url, "https://example.com");
        assert!(results[0].snippet.contains("snippet about Example"));
        assert_eq!(results[1].url, "https://other.com/page");
    }

    #[test]
    fn parse_ddg_html_extracts_uddg_redirect() {
        let html = r#"
        <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Freal.com%2Fpage&amp;rut=abc">
            Real Result
        </a>
        <a class="result__snippet">The real snippet.</a>
        "#;
        let results = parse_ddg_html(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://real.com/page");
    }

    #[test]
    fn parse_ddg_html_empty() {
        assert!(parse_ddg_html("<html><body>No results</body></html>").is_empty());
    }

    #[tokio::test]
    async fn execute_blocks_readonly() {
        let security = Arc::new(SecurityPolicy {
            autonomy: AutonomyLevel::ReadOnly,
            ..SecurityPolicy::default()
        });
        let tool = WebSearchTool::new(security);
        let result = tool
            .execute(json!({"query": "test"}))
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
        let tool = WebSearchTool::new(security);
        let result = tool
            .execute(json!({"query": "test"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("rate limit"));
    }

    #[tokio::test]
    async fn execute_rejects_empty_query() {
        let tool = test_tool();
        let result = tool
            .execute(json!({"query": "  "}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("empty"));
    }
}
