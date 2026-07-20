use std::{
	collections::HashSet,
	net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
	str::FromStr,
	time::Duration,
};

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{
	header::{CONTENT_TYPE, LOCATION},
	redirect::Policy,
	Client, Response, Url,
};
use scraper::{Html, Selector};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use tokio::net::lookup_host;

use crate::{
	error::AppError,
	providers::{ToolCall, ToolDefinition, ToolFunctionDefinition},
	tools::{ToolExecution, ToolExecutor},
};

pub const SEARCH_WEB_TOOL_NAME: &str = "search_web";
pub const FETCH_WEB_PAGE_TOOL_NAME: &str = "fetch_web_page";
const BING_SEARCH_URL: &str = "https://www.bing.com/search";
const USER_AGENT: &str = "Taurus/0.1 (+local desktop assistant)";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PAGE_CHARACTERS: usize = 16_000;
const MAX_AGENT_PAGE_CHARACTERS: usize = 3_000;
const MAX_REDIRECTS: usize = 5;
const AUTO_FETCH_RESULT_LIMIT: usize = 2;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WebSearchResult {
	pub title: String,
	pub url: String,
	pub snippet: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WebPage {
	pub url: String,
	pub title: Option<String>,
	pub content: String,
}

#[derive(Serialize)]
struct WebPageEvidence<'a> {
	url: &'a str,
	title: Option<&'a str>,
	content_excerpt: String,
	content_truncated: bool,
}

#[derive(Debug, Deserialize)]
struct SearchWebArguments {
	query: String,
	#[serde(default = "default_max_results")]
	max_results: usize,
}

#[derive(Debug, Deserialize)]
struct FetchWebPageArguments {
	url: String,
}

#[derive(Debug, Deserialize)]
struct BingRssResponse {
	channel: BingRssChannel,
}

#[derive(Debug, Deserialize)]
struct BingRssChannel {
	#[serde(default, rename = "item")]
	items: Vec<BingRssItem>,
}

#[derive(Debug, Deserialize)]
struct BingRssItem {
	#[serde(default)]
	title: String,
	#[serde(default)]
	link: String,
	#[serde(default)]
	description: String,
}

pub struct WebTools {
	search_client: Client,
}

impl WebTools {
	pub fn new() -> Result<Self, AppError> {
		let search_client = Client::builder()
			.user_agent(USER_AGENT)
			.connect_timeout(CONNECT_TIMEOUT)
			.timeout(REQUEST_TIMEOUT)
			.redirect(Policy::limited(MAX_REDIRECTS))
			.build()?;
		Ok(Self { search_client })
	}

	pub fn definitions() -> Vec<ToolDefinition> {
		vec![search_tool_definition(), fetch_tool_definition()]
	}

	async fn search(
		&self,
		query: &str,
		max_results: usize,
	) -> Result<Vec<WebSearchResult>, AppError> {
		let query = validate_query(query)?;
		let result_limit = max_results.clamp(1, 10);
		let response = self
			.search_client
			.get(BING_SEARCH_URL)
			.query(&[("q", query), ("format", "rss")])
			.send()
			.await
			.map_err(|error| AppError::WebTool(format!("Web search request failed: {error}")))?;
		ensure_success(&response, "Web search")?;
		let bytes = read_limited_body(response).await?;
		let rss = String::from_utf8_lossy(&bytes);
		parse_search_results(&rss, result_limit)
	}

	async fn fetch(&self, raw_url: &str) -> Result<WebPage, AppError> {
		let initial_url = parse_public_url(raw_url)?;
		let (final_url, response) = follow_public_redirects(initial_url).await?;
		parse_page_response(final_url, response).await
	}
}

#[async_trait]
impl ToolExecutor for WebTools {
	fn definitions(&self) -> Vec<ToolDefinition> {
		Self::definitions()
	}

	fn describe(&self, call: &ToolCall) -> (String, String) {
		describe_tool_call(call)
	}

	async fn execute(&self, call: &ToolCall) -> Result<ToolExecution, AppError> {
		match call.function.name.as_str() {
			SEARCH_WEB_TOOL_NAME => {
				let arguments: SearchWebArguments = parse_arguments(&call.function.arguments)?;
				let results = self.search(&arguments.query, arguments.max_results).await?;
				let follow_up_calls = page_fetch_calls(&results);
				let detail =
					format_search_detail(&arguments.query, &results, follow_up_calls.len());
				let content = serde_json::to_string(&results).map_err(|error| {
					AppError::WebTool(format!("Could not encode search results: {error}"))
				})?;
				Ok(ToolExecution {
					content,
					detail,
					follow_up_calls,
				})
			}
			FETCH_WEB_PAGE_TOOL_NAME => {
				let arguments: FetchWebPageArguments = parse_arguments(&call.function.arguments)?;
				let page = self.fetch(&arguments.url).await?;
				let detail = format_page_detail(&page);
				let content = serialize_page_evidence(&page)?;
				Ok(ToolExecution {
					content,
					detail,
					follow_up_calls: Vec::new(),
				})
			}
			name => Err(AppError::WebTool(format!("Unknown web tool '{name}'."))),
		}
	}
}

fn page_fetch_calls(results: &[WebSearchResult]) -> Vec<ToolCall> {
	results
		.iter()
		.take(AUTO_FETCH_RESULT_LIMIT)
		.map(|result| ToolCall {
			tool_type: "function".to_string(),
			function: crate::providers::ToolFunctionCall {
				index: None,
				name: FETCH_WEB_PAGE_TOOL_NAME.to_string(),
				arguments: json!({ "url": result.url }),
			},
		})
		.collect()
}

fn format_search_detail(query: &str, results: &[WebSearchResult], pages_to_fetch: usize) -> String {
	let next_action = if pages_to_fetch == 0 {
		"No result pages are available to fetch.".to_string()
	} else {
		format!("Next: Taurus will fetch the {pages_to_fetch} highest-ranked result page(s).")
	};
	let mut lines = vec![
		format!("Query: {}", query.trim()),
		format!("Found {} web search result(s):", results.len()),
		next_action,
	];
	for (index, result) in results.iter().enumerate() {
		lines.push(format!(
			"{}. {}\n{}\n{}",
			index + 1,
			result.title,
			result.url,
			result.snippet
		));
	}
	lines.join("\n\n")
}

fn format_page_detail(page: &WebPage) -> String {
	let title = page.title.as_deref().unwrap_or("Untitled page");
	format!(
		"URL: {}\nTitle: {title}\nFetched content ({} characters):\n\n{}",
		page.url,
		page.content.chars().count(),
		page.content
	)
}

fn serialize_page_evidence(page: &WebPage) -> Result<String, AppError> {
	let character_count = page.content.chars().count();
	let evidence = WebPageEvidence {
		url: &page.url,
		title: page.title.as_deref(),
		content_excerpt: truncate_characters(&page.content, MAX_AGENT_PAGE_CHARACTERS),
		content_truncated: character_count > MAX_AGENT_PAGE_CHARACTERS,
	};
	serde_json::to_string(&evidence)
		.map_err(|error| AppError::WebTool(format!("Could not encode fetched page: {error}")))
}

pub fn describe_tool_call(call: &ToolCall) -> (String, String) {
	match call.function.name.as_str() {
		SEARCH_WEB_TOOL_NAME => {
			let query = argument_string(&call.function.arguments, "query");
			("Searching the web".to_string(), format!("Query: {query}"))
		}
		FETCH_WEB_PAGE_TOOL_NAME => {
			let url = argument_string(&call.function.arguments, "url");
			let label = Url::parse(&url)
				.ok()
				.and_then(|parsed| parsed.host_str().map(str::to_string))
				.map(|host| format!("Reading {host}"))
				.unwrap_or_else(|| "Reading a web page".to_string());
			(label, format!("URL: {url}"))
		}
		name => (
			format!("Running {name}"),
			"The model requested a tool.".to_string(),
		),
	}
}

fn search_tool_definition() -> ToolDefinition {
	ToolDefinition {
		tool_type: "function".to_string(),
		function: ToolFunctionDefinition {
			name: SEARCH_WEB_TOOL_NAME.to_string(),
			description: "Search the public web for current or external information. Returns titles, URLs, and snippets.".to_string(),
			parameters: json!({
				"type": "object",
				"required": ["query"],
				"properties": {
					"query": { "type": "string", "description": "A focused web search query." },
					"max_results": { "type": "integer", "minimum": 1, "maximum": 10, "default": 5 }
				}
			}),
		},
	}
}

fn fetch_tool_definition() -> ToolDefinition {
	ToolDefinition {
		tool_type: "function".to_string(),
		function: ToolFunctionDefinition {
			name: FETCH_WEB_PAGE_TOOL_NAME.to_string(),
			description: "Fetch a public HTTP or HTTPS web page and return its readable text. Use it to inspect promising search results.".to_string(),
			parameters: json!({
				"type": "object",
				"required": ["url"],
				"properties": {
					"url": { "type": "string", "description": "The absolute public HTTP or HTTPS URL to fetch." }
				}
			}),
		},
	}
}

fn default_max_results() -> usize {
	5
}

fn validate_query(query: &str) -> Result<&str, AppError> {
	let trimmed = query.trim();
	if trimmed.is_empty() {
		return Err(AppError::WebTool(
			"The web search query cannot be empty.".to_string(),
		));
	}
	if trimmed.chars().count() > 500 {
		return Err(AppError::WebTool(
			"The web search query is too long.".to_string(),
		));
	}
	Ok(trimmed)
}

fn parse_arguments<T: DeserializeOwned>(value: &serde_json::Value) -> Result<T, AppError> {
	let normalized = match value {
		serde_json::Value::String(raw) => serde_json::from_str(raw).map_err(|error| {
			AppError::WebTool(format!("Tool arguments were not valid JSON: {error}"))
		})?,
		other => other.clone(),
	};
	serde_json::from_value(normalized)
		.map_err(|error| AppError::WebTool(format!("Tool arguments were invalid: {error}")))
}

fn argument_string(arguments: &serde_json::Value, key: &str) -> String {
	let normalized = match arguments {
		serde_json::Value::String(raw) => serde_json::from_str(raw).unwrap_or_default(),
		other => other.clone(),
	};
	normalized
		.get(key)
		.and_then(serde_json::Value::as_str)
		.unwrap_or("Unavailable")
		.to_string()
}

fn parse_search_results(rss: &str, limit: usize) -> Result<Vec<WebSearchResult>, AppError> {
	let payload: BingRssResponse = quick_xml::de::from_str(rss).map_err(|error| {
		AppError::WebTool(format!(
			"The web search service returned an invalid results feed: {error}"
		))
	})?;
	let mut seen_urls = HashSet::new();
	let results = payload
		.channel
		.items
		.into_iter()
		.filter_map(|item| {
			let title = normalize_string(&item.title);
			let url = public_http_url(item.link.trim())?.to_string();
			if title.is_empty() || !seen_urls.insert(url.clone()) {
				return None;
			}
			Some(WebSearchResult {
				title,
				url,
				snippet: normalize_string(&item.description),
			})
		})
		.take(limit)
		.collect();
	Ok(results)
}

fn parse_web_page(url: &str, html: &str) -> Result<WebPage, AppError> {
	let document = Html::parse_document(html);
	let title = document
		.select(&selector("title")?)
		.next()
		.map(|element| normalized_text(element.text()))
		.filter(|value| !value.is_empty());
	let root_selector = selector("main, article, body")?;
	let content_selector = selector("h1, h2, h3, h4, p, li, blockquote, pre, td, th")?;
	let content = document
		.select(&root_selector)
		.next()
		.map(|root| {
			root.select(&content_selector)
				.map(|element| normalized_text(element.text()))
				.filter(|text| !text.is_empty())
				.collect::<Vec<_>>()
				.join("\n")
		})
		.unwrap_or_default();
	if content.is_empty() {
		return Err(AppError::WebTool(
			"The page did not contain readable text.".to_string(),
		));
	}
	Ok(WebPage {
		url: url.to_string(),
		title,
		content: truncate_characters(&content, MAX_PAGE_CHARACTERS),
	})
}

fn selector(value: &str) -> Result<Selector, AppError> {
	Selector::parse(value)
		.map_err(|error| AppError::WebTool(format!("Invalid HTML selector '{value}': {error}")))
}

fn normalized_text<'a>(parts: impl Iterator<Item = &'a str>) -> String {
	parts
		.collect::<Vec<_>>()
		.join(" ")
		.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
}

fn normalize_string(value: &str) -> String {
	value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_characters(value: &str, max_characters: usize) -> String {
	if value.chars().count() <= max_characters {
		return value.to_string();
	}
	let notice = "\n[Page content truncated]";
	let retained_characters = max_characters.saturating_sub(notice.chars().count());
	let mut truncated: String = value.chars().take(retained_characters).collect();
	truncated.push_str(notice);
	truncated
}

fn public_http_url(raw_url: &str) -> Option<Url> {
	let parsed = Url::parse(raw_url).ok()?;
	match parsed.scheme() {
		"http" | "https" if parsed.host_str().is_some() => Some(parsed),
		_ => None,
	}
}

fn parse_public_url(raw_url: &str) -> Result<Url, AppError> {
	let parsed = public_http_url(raw_url.trim()).ok_or_else(|| {
		AppError::WebTool("Only absolute HTTP and HTTPS page URLs are allowed.".to_string())
	})?;
	if !parsed.username().is_empty() || parsed.password().is_some() {
		return Err(AppError::WebTool(
			"Page URLs containing credentials are not allowed.".to_string(),
		));
	}
	validate_hostname(parsed.host_str().unwrap_or_default())?;
	Ok(parsed)
}

fn validate_hostname(host: &str) -> Result<(), AppError> {
	let normalized = host.trim_end_matches('.').to_ascii_lowercase();
	if normalized == "localhost"
		|| normalized.ends_with(".localhost")
		|| normalized.ends_with(".local")
	{
		return Err(AppError::WebTool(
			"Local and private network pages cannot be fetched.".to_string(),
		));
	}
	if let Ok(address) = IpAddr::from_str(&normalized) {
		ensure_public_ip(address)?;
	}
	Ok(())
}

async fn send_public_request(url: &Url) -> Result<Response, AppError> {
	let host = url
		.host_str()
		.ok_or_else(|| AppError::WebTool("The page URL has no host.".to_string()))?;
	validate_hostname(host)?;
	let port = url
		.port_or_known_default()
		.ok_or_else(|| AppError::WebTool("The page URL uses an unsupported port.".to_string()))?;
	let addresses: Vec<SocketAddr> = lookup_host((host, port))
		.await
		.map_err(|error| AppError::WebTool(format!("Could not resolve page host: {error}")))?
		.collect();
	if addresses.is_empty() {
		return Err(AppError::WebTool(
			"The page host did not resolve to an address.".to_string(),
		));
	}
	for address in &addresses {
		ensure_public_ip(address.ip())?;
	}
	let mut builder = Client::builder()
		.user_agent(USER_AGENT)
		.connect_timeout(CONNECT_TIMEOUT)
		.timeout(REQUEST_TIMEOUT)
		.redirect(Policy::none());
	if IpAddr::from_str(host).is_err() {
		builder = builder.resolve(host, addresses[0]);
	}
	let client = builder.build()?;
	client
		.get(url.clone())
		.send()
		.await
		.map_err(|error| AppError::WebTool(format!("Page fetch request failed: {error}")))
}

async fn follow_public_redirects(mut current_url: Url) -> Result<(Url, Response), AppError> {
	for redirect_count in 0..=MAX_REDIRECTS {
		let response = send_public_request(&current_url).await?;
		if !response.status().is_redirection() {
			return Ok((current_url, response));
		}
		if redirect_count == MAX_REDIRECTS {
			return Err(AppError::WebTool(
				"The page redirected too many times.".to_string(),
			));
		}
		current_url = redirect_url(&current_url, &response)?;
	}
	Err(AppError::WebTool(
		"The page could not be fetched.".to_string(),
	))
}

async fn parse_page_response(final_url: Url, response: Response) -> Result<WebPage, AppError> {
	ensure_success(&response, "Page fetch")?;
	validate_content_type(&response)?;
	let declared_html = response_is_html(&response);
	let bytes = read_limited_body(response).await?;
	let content_type = if declared_html {
		PageContentType::Html
	} else {
		sniff_content_type(&bytes)
	};
	let body = String::from_utf8_lossy(&bytes);
	if content_type == PageContentType::Html {
		parse_web_page(final_url.as_str(), &body)
	} else {
		Ok(WebPage {
			url: final_url.to_string(),
			title: None,
			content: truncate_characters(body.trim(), MAX_PAGE_CHARACTERS),
		})
	}
}

fn ensure_public_ip(address: IpAddr) -> Result<(), AppError> {
	let is_public = match address {
		IpAddr::V4(ipv4) => is_public_ipv4(ipv4),
		IpAddr::V6(ipv6) => is_public_ipv6(ipv6),
	};
	if is_public {
		Ok(())
	} else {
		Err(AppError::WebTool(
			"Local and private network pages cannot be fetched.".to_string(),
		))
	}
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
	is_standard_public_ipv4(address) && !is_special_purpose_ipv4(address)
}

fn is_standard_public_ipv4(address: Ipv4Addr) -> bool {
	!(address.is_private()
		|| address.is_loopback()
		|| address.is_link_local()
		|| address.is_broadcast()
		|| address.is_documentation()
		|| address.is_multicast()
		|| address.is_unspecified())
}

fn is_special_purpose_ipv4(address: Ipv4Addr) -> bool {
	let [first, second, ..] = address.octets();
	first == 0
		|| first >= 240
		|| (first == 100 && (64..=127).contains(&second))
		|| (first == 192 && second == 0)
		|| (first == 198 && (18..=19).contains(&second))
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
	if let Some(ipv4) = address.to_ipv4() {
		return is_public_ipv4(ipv4);
	}
	let first_segment = address.segments()[0];
	!(address.is_loopback()
		|| address.is_unspecified()
		|| address.is_unique_local()
		|| address.is_unicast_link_local()
		|| address.is_multicast()
		|| (first_segment == 0x2001 && address.segments()[1] == 0x0db8))
		&& first_segment & 0xffc0 != 0xfec0
}

fn redirect_url(current_url: &Url, response: &Response) -> Result<Url, AppError> {
	let location = response
		.headers()
		.get(LOCATION)
		.ok_or_else(|| AppError::WebTool("The page returned an invalid redirect.".to_string()))?
		.to_str()
		.map_err(|_| AppError::WebTool("The page returned an invalid redirect.".to_string()))?;
	let redirected = current_url.join(location).map_err(|error| {
		AppError::WebTool(format!(
			"The page returned an invalid redirect URL: {error}"
		))
	})?;
	parse_public_url(redirected.as_str())
}

fn ensure_success(response: &Response, action: &str) -> Result<(), AppError> {
	if response.status().is_success() {
		Ok(())
	} else {
		Err(AppError::WebTool(format!(
			"{action} failed with HTTP status {}.",
			response.status()
		)))
	}
}

fn validate_content_type(response: &Response) -> Result<(), AppError> {
	let Some(content_type) = response.headers().get(CONTENT_TYPE) else {
		return Ok(());
	};
	let content_type = content_type
		.to_str()
		.unwrap_or_default()
		.to_ascii_lowercase();
	if content_type.starts_with("text/") || content_type.contains("application/xhtml+xml") {
		Ok(())
	} else {
		Err(AppError::WebTool(format!(
			"The URL returned unsupported content type '{content_type}'."
		)))
	}
}

fn response_is_html(response: &Response) -> bool {
	response
		.headers()
		.get(CONTENT_TYPE)
		.and_then(|value| value.to_str().ok())
		.is_some_and(|value| {
			let normalized = value.to_ascii_lowercase();
			normalized.contains("text/html") || normalized.contains("application/xhtml+xml")
		})
}

async fn read_limited_body(response: Response) -> Result<Vec<u8>, AppError> {
	if response
		.content_length()
		.is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
	{
		return Err(AppError::WebTool(
			"The web response is too large to process safely.".to_string(),
		));
	}
	let mut stream = response.bytes_stream();
	let mut body = Vec::new();
	while let Some(chunk) = stream.next().await {
		let chunk = chunk
			.map_err(|error| AppError::WebTool(format!("Could not read web response: {error}")))?;
		if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
			return Err(AppError::WebTool(
				"The web response is too large to process safely.".to_string(),
			));
		}
		body.extend_from_slice(&chunk);
	}
	Ok(body)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageContentType {
	Html,
	PlainText,
}

fn sniff_content_type(bytes: &[u8]) -> PageContentType {
	let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_ascii_lowercase();
	if prefix.contains("<!doctype html") || prefix.contains("<html") {
		PageContentType::Html
	} else {
		PageContentType::PlainText
	}
}

#[cfg(test)]
mod tests {
	use std::net::{IpAddr, Ipv4Addr};

	use super::{
		format_page_detail, format_search_detail, is_public_ipv4, page_fetch_calls,
		parse_search_results, parse_web_page, serialize_page_evidence, WebPage, WebSearchResult,
		WebTools, MAX_AGENT_PAGE_CHARACTERS,
	};

	#[test]
	fn exposes_exactly_the_two_web_tools() {
		let definitions = WebTools::definitions();
		let names: Vec<&str> = definitions
			.iter()
			.map(|definition| definition.function.name.as_str())
			.collect();
		assert_eq!(names, vec!["search_web", "fetch_web_page"]);
	}

	#[test]
	fn parses_bing_rss_results_and_decodes_entities() {
		let rss = r#"<?xml version="1.0" encoding="utf-8" ?>
			<rss version="2.0"><channel><title>Bing</title>
				<item>
					<title>Example &amp; guide</title>
					<link>https://example.com/guide?a=1&amp;b=2</link>
					<description>A useful guide for testing.</description>
				</item>
			</channel></rss>"#;
		let results = parse_search_results(rss, 5).expect("search RSS should parse");
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].title, "Example & guide");
		assert_eq!(results[0].url, "https://example.com/guide?a=1&b=2");
		assert_eq!(results[0].snippet, "A useful guide for testing.");
	}

	#[test]
	fn accepts_a_valid_search_feed_with_no_results() {
		let rss = r#"<?xml version="1.0"?><rss version="2.0"><channel><title>Bing</title></channel></rss>"#;
		let results = parse_search_results(rss, 5).expect("empty search RSS should parse");
		assert!(results.is_empty());
	}

	#[tokio::test]
	#[ignore = "requires live Bing search access"]
	async fn live_bing_search_returns_page_urls() {
		let tools = WebTools::new().expect("web tools should initialize");
		let results = tools
			.search("actualites France aujourd'hui", 5)
			.await
			.expect("live web search should succeed");
		assert!(!results.is_empty());
		assert!(results.iter().all(|result| result.url.starts_with("http")));
	}

	#[test]
	fn extracts_readable_page_content_without_scripts() {
		let html = r#"
			<html><head><title> Example article </title><script>secret()</script></head>
			<body><main><h1>Heading</h1><p>First paragraph.</p><script>ignored()</script></main></body></html>
		"#;
		let page = parse_web_page("https://example.com", html).expect("page should parse");
		assert_eq!(page.title.as_deref(), Some("Example article"));
		assert_eq!(page.content, "Heading\nFirst paragraph.");
	}

	#[test]
	fn search_detail_includes_query_urls_and_snippets() {
		let results = vec![WebSearchResult {
			title: "Example result".to_string(),
			url: "https://example.com/news".to_string(),
			snippet: "Example snippet".to_string(),
		}];
		let detail = format_search_detail("French news", &results, 1);
		assert!(detail.contains("Query: French news"));
		assert!(detail.contains("Example result"));
		assert!(detail.contains("https://example.com/news"));
		assert!(detail.contains("Example snippet"));
		assert!(detail.contains("will fetch the 1 highest-ranked"));
		let follow_up_calls = page_fetch_calls(&results);
		assert_eq!(follow_up_calls.len(), 1);
		assert_eq!(follow_up_calls[0].function.name, "fetch_web_page");
		assert_eq!(
			follow_up_calls[0].function.arguments["url"],
			"https://example.com/news"
		);
	}

	#[test]
	fn page_detail_includes_the_fetched_content() {
		let page = WebPage {
			url: "https://example.com/article".to_string(),
			title: Some("Example article".to_string()),
			content: "Full fetched article text.".to_string(),
		};
		let detail = format_page_detail(&page);
		assert!(detail.contains("URL: https://example.com/article"));
		assert!(detail.contains("Title: Example article"));
		assert!(detail.contains("Full fetched article text."));
	}

	#[test]
	fn agent_page_evidence_is_compact_while_the_detail_remains_complete() {
		let full_content = "x".repeat(MAX_AGENT_PAGE_CHARACTERS + 100);
		let page = WebPage {
			url: "https://example.com/long-article".to_string(),
			title: Some("Long article".to_string()),
			content: full_content.clone(),
		};
		let encoded = serialize_page_evidence(&page).expect("page evidence should encode");
		let evidence: serde_json::Value =
			serde_json::from_str(&encoded).expect("page evidence should be JSON");
		assert_eq!(
			evidence["content_excerpt"]
				.as_str()
				.expect("content excerpt should be text")
				.chars()
				.count(),
			MAX_AGENT_PAGE_CHARACTERS
		);
		assert_eq!(evidence["content_truncated"], true);
		assert!(format_page_detail(&page).contains(&full_content));
	}

	#[test]
	fn rejects_private_and_special_ipv4_ranges() {
		assert!(!is_public_ipv4(Ipv4Addr::new(127, 0, 0, 1)));
		assert!(!is_public_ipv4(Ipv4Addr::new(10, 0, 0, 1)));
		assert!(!is_public_ipv4(Ipv4Addr::new(169, 254, 1, 1)));
		assert!(!is_public_ipv4(Ipv4Addr::new(100, 64, 0, 1)));
		assert!(!is_public_ipv4(Ipv4Addr::new(255, 255, 255, 254)));
		assert!(is_public_ipv4(Ipv4Addr::new(93, 184, 216, 34)));
		let _: IpAddr = "93.184.216.34".parse().expect("public IP should parse");
	}
}
