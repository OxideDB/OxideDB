//! Plugin admin page discovery and asset serving.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use mime_guess::from_path;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{debug, warn};

use super::{ensure_plugin_superuser, types::*};
use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};
use oxide_core::{
    plugin_config::{PluginConfiguration, PluginStatus as PersistedPluginStatus},
    site_settings::BrandingSettings,
};

const PLUGIN_ADMIN_STYLE: &str = r#"
:root,
:root[data-oxide-theme="light"] {
  color-scheme: light;
  --background: hsl(210 24% 98%);
  --foreground: hsl(222 39% 11%);
  --card: hsl(0 0% 100%);
  --card-foreground: hsl(222 39% 11%);
  --primary: hsl(187 79% 29%);
  --primary-foreground: hsl(0 0% 100%);
  --secondary: hsl(210 22% 93%);
  --secondary-foreground: hsl(222 39% 14%);
  --muted: hsl(210 24% 95%);
  --muted-foreground: hsl(215 13% 40%);
  --border: hsl(214 22% 88%);
  --input: hsl(214 22% 88%);
  --ring: hsl(187 79% 29%);
  --destructive: hsl(0 84.2% 60.2%);
  --success: hsl(154 58% 35%);
  --warning: hsl(39 92% 47%);
  --radius: 8px;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-oxide-theme="light"]) {
    color-scheme: dark;
    --background: hsl(220 13% 8%);
    --foreground: hsl(210 24% 96%);
    --card: hsl(220 13% 10%);
    --card-foreground: hsl(210 24% 96%);
    --primary: hsl(184 70% 52%);
    --primary-foreground: hsl(220 22% 8%);
    --secondary: hsl(220 11% 16%);
    --secondary-foreground: hsl(210 24% 96%);
    --muted: hsl(220 11% 16%);
    --muted-foreground: hsl(216 12% 70%);
    --border: hsl(220 10% 22%);
    --input: hsl(220 10% 22%);
    --ring: hsl(184 70% 52%);
    --destructive: hsl(0 62.8% 45%);
    --success: hsl(151 60% 45%);
    --warning: hsl(42 92% 55%);
  }
}

:root[data-oxide-theme="dark"] {
  color-scheme: dark;
  --background: hsl(220 13% 8%);
  --foreground: hsl(210 24% 96%);
  --card: hsl(220 13% 10%);
  --card-foreground: hsl(210 24% 96%);
  --primary: hsl(184 70% 52%);
  --primary-foreground: hsl(220 22% 8%);
  --secondary: hsl(220 11% 16%);
  --secondary-foreground: hsl(210 24% 96%);
  --muted: hsl(220 11% 16%);
  --muted-foreground: hsl(216 12% 70%);
  --border: hsl(220 10% 22%);
  --input: hsl(220 10% 22%);
  --ring: hsl(184 70% 52%);
  --destructive: hsl(0 62.8% 45%);
  --success: hsl(151 60% 45%);
  --warning: hsl(42 92% 55%);
}

* { box-sizing: border-box; }

html,
body {
  min-height: 100%;
  margin: 0;
}

body {
  background: var(--background);
  color: var(--foreground);
  font-size: 14px;
  line-height: 1.5;
  text-rendering: optimizeLegibility;
}

a { color: var(--primary); }

.oxide-page {
  display: grid;
  gap: 16px;
  max-width: 1120px;
  margin: 0 auto;
  padding: 20px;
}

.oxide-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.oxide-title {
  margin: 0;
  color: var(--foreground);
  font-size: 22px;
  font-weight: 650;
  line-height: 1.2;
}

.oxide-description {
  max-width: 720px;
  margin: 6px 0 0;
  color: var(--muted-foreground);
}

.oxide-panel {
  border: 1px solid var(--border);
  border-radius: var(--radius);
  background: var(--card);
  color: var(--card-foreground);
  box-shadow: 0 1px 2px rgb(15 23 42 / 0.04);
}

.oxide-panel-header {
  display: grid;
  gap: 4px;
  padding: 16px 18px;
  border-bottom: 1px solid var(--border);
}

.oxide-panel-title {
  margin: 0;
  font-size: 15px;
  font-weight: 650;
}

.oxide-panel-description {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 13px;
}

.oxide-panel-body {
  display: grid;
  gap: 14px;
  padding: 18px;
}

.oxide-grid {
  display: grid;
  gap: 14px;
}

@media (min-width: 760px) {
  .oxide-grid-2 { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .oxide-grid-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); }
}

.oxide-label {
  display: block;
  margin-bottom: 6px;
  color: var(--foreground);
  font-size: 13px;
  font-weight: 600;
}

.oxide-input,
.oxide-select,
.oxide-textarea {
  width: 100%;
  min-height: 38px;
  border: 1px solid var(--input);
  border-radius: 6px;
  background: var(--background);
  color: var(--foreground);
  padding: 8px 10px;
  font: inherit;
}

.oxide-textarea {
  min-height: 96px;
  resize: vertical;
}

.oxide-input:focus,
.oxide-select:focus,
.oxide-textarea:focus {
  outline: 2px solid color-mix(in srgb, var(--ring) 30%, transparent);
  outline-offset: 2px;
  border-color: var(--ring);
}

.oxide-button {
  display: inline-flex;
  min-height: 36px;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: var(--primary);
  color: var(--primary-foreground);
  padding: 8px 12px;
  font: inherit;
  font-weight: 600;
  cursor: pointer;
}

.oxide-button.secondary {
  border-color: var(--border);
  background: var(--secondary);
  color: var(--secondary-foreground);
}

.oxide-muted {
  color: var(--muted-foreground);
}

.oxide-badge {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--muted);
  color: var(--muted-foreground);
  padding: 2px 8px;
  font-size: 12px;
  font-weight: 600;
}
"#;

/// Query options accepted when serving a plugin admin page asset.
#[derive(Debug, Default, Deserialize)]
pub struct AdminPageAssetQuery {
    oxide_theme: Option<String>,
}

/// List admin pages contributed by installed plugins.
pub async fn list_admin_pages(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PluginAdminPageInfo>>>, ApiError> {
    ensure_plugin_superuser(&authenticated_user, "list plugin admin pages")?;

    let plugin_configs = state
        .plugin_config_service
        .list_plugin_configs()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configurations: {}", e)))?;

    let mut pages = Vec::new();
    for config in plugin_configs {
        pages.extend(admin_pages_from_config(&config));
    }
    pages.sort_by(|a, b| {
        a.nav_group
            .cmp(&b.nav_group)
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.plugin_name.cmp(&b.plugin_name))
    });

    Ok(Json(ApiResponse::success(pages)))
}

/// Serve the shared stylesheet available to packaged plugin admin pages.
pub async fn serve_admin_page_styles(State(state): State<AppState>) -> impl IntoResponse {
    let settings = state.runtime_settings.current().await;
    let stylesheet = plugin_admin_style(&settings.branding);

    Response::builder()
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(stylesheet))
        .unwrap_or_else(|error| {
            warn!(
                "Failed to build plugin admin stylesheet response: {}",
                error
            );
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to build stylesheet response"))
                .unwrap_or_else(|_| Response::new(Body::empty()))
        })
}

/// Serve a packaged admin page asset for an enabled plugin.
pub async fn serve_admin_page_asset(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(params): Path<HashMap<String, String>>,
    Query(query): Query<AdminPageAssetQuery>,
) -> Result<Response<Body>, ApiError> {
    ensure_plugin_superuser(&authenticated_user, "view plugin admin pages")?;

    let plugin_name = params
        .get("plugin_name")
        .ok_or_else(|| ApiError::bad_request("Missing plugin name".to_string()))?;
    let asset_path = params
        .get("path")
        .ok_or_else(|| ApiError::bad_request("Missing plugin admin asset path".to_string()))?;

    debug!(
        "Serving plugin admin asset: {} -> {}",
        plugin_name, asset_path
    );

    if !is_allowed_admin_asset_path(asset_path) {
        return Err(ApiError::bad_request(format!(
            "Invalid plugin admin asset path '{}'",
            asset_path
        )));
    }

    let content = state
        .plugin_config_service
        .load_plugin_admin_asset(plugin_name, asset_path)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to load plugin admin asset: {}", e)))?;

    let mime_type = from_path(asset_path).first_or_octet_stream();
    let body = if is_admin_html_asset(asset_path) {
        let settings = state.runtime_settings.current().await;
        Body::from(inject_admin_page_theme(
            content,
            query.oxide_theme.as_deref(),
            settings.updated_at,
        ))
    } else {
        Body::from(content)
    };

    Response::builder()
        .header(header::CONTENT_TYPE, mime_type.as_ref())
        .header(header::CACHE_CONTROL, "private, max-age=300")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .map_err(|e| ApiError::internal(format!("Failed to build asset response: {}", e)))
}

fn plugin_admin_style(branding: &BrandingSettings) -> String {
    let mut style = PLUGIN_ADMIN_STYLE.to_string();
    let branding_style = plugin_admin_branding_style(branding);

    if !branding_style.is_empty() {
        style.push('\n');
        style.push_str(&branding_style);
    }

    style
}

fn plugin_admin_branding_style(branding: &BrandingSettings) -> String {
    let mut style = String::new();
    let mut overrides: Vec<(&str, String)> = Vec::new();

    if let Some(primary_color) = hsl_triplet_from_hex(branding.primary_color.as_deref()) {
        overrides.push(("--primary", primary_color.clone()));
        overrides.push(("--ring", primary_color));
    }

    if let Some(secondary_color) = hsl_triplet_from_hex(branding.secondary_color.as_deref()) {
        overrides.push(("--secondary", secondary_color));
    }

    if !overrides.is_empty() {
        write_branding_rule(
            &mut style,
            ":root,\n:root[data-oxide-theme=\"light\"],\n:root[data-oxide-theme=\"dark\"]",
            &overrides,
            "",
        );
        style.push_str("\n@media (prefers-color-scheme: dark) {\n");
        write_branding_rule(
            &mut style,
            ":root:not([data-oxide-theme=\"light\"])",
            &overrides,
            "  ",
        );
        style.push_str("}\n");
    }

    if let Some(custom_css) = branding.custom_css.as_deref().map(str::trim) {
        if !custom_css.is_empty() {
            if !style.is_empty() {
                style.push('\n');
            }
            style.push_str("/* Site settings custom CSS */\n");
            style.push_str(custom_css);
            style.push('\n');
        }
    }

    style
}

fn write_branding_rule(
    style: &mut String,
    selector: &str,
    overrides: &[(&str, String)],
    indent: &str,
) {
    for line in selector.lines() {
        style.push_str(indent);
        style.push_str(line);
        style.push('\n');
    }
    style.push_str(indent);
    style.push_str("{\n");

    for (variable, value) in overrides {
        style.push_str(indent);
        style.push_str("  ");
        style.push_str(variable);
        style.push_str(": hsl(");
        style.push_str(value);
        style.push_str(");\n");
    }

    style.push_str(indent);
    style.push_str("}\n");
}

fn hsl_triplet_from_hex(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    let hex = value.strip_prefix('#')?;

    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }

    let red = f64::from(u8::from_str_radix(&hex[0..2], 16).ok()?) / 255.0;
    let green = f64::from(u8::from_str_radix(&hex[2..4], 16).ok()?) / 255.0;
    let blue = f64::from(u8::from_str_radix(&hex[4..6], 16).ok()?) / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = (max + min) / 2.0;
    let delta = max - min;

    if delta == 0.0 {
        return Some(format!("0 0% {}%", percent(lightness)));
    }

    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let mut hue = if max == red {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if max == green {
        60.0 * ((blue - red) / delta + 2.0)
    } else {
        60.0 * ((red - green) / delta + 4.0)
    };

    if hue < 0.0 {
        hue += 360.0;
    }

    Some(format!(
        "{} {}% {}%",
        hue.round() as i32,
        percent(saturation),
        percent(lightness)
    ))
}

fn percent(value: f64) -> i32 {
    (value * 100.0).round() as i32
}

fn inject_admin_page_theme(
    content: Vec<u8>,
    theme: Option<&str>,
    settings_version: i64,
) -> Vec<u8> {
    let Some(theme) = normalized_admin_page_theme(theme) else {
        return content;
    };

    let mut html = match String::from_utf8(content) {
        Ok(html) => html,
        Err(error) => return error.into_bytes(),
    };

    rewrite_admin_page_style_links(&mut html, settings_version);

    let Some(insert_at) = head_insert_position(&html) else {
        return html.into_bytes();
    };

    let mut themed_html = String::with_capacity(html.len() + 120);
    themed_html.push_str(&html[..insert_at]);
    themed_html.push_str("\n    <script>document.documentElement.dataset.oxideTheme = \"");
    themed_html.push_str(theme);
    themed_html.push_str("\";</script>");
    themed_html.push_str(&html[insert_at..]);
    themed_html.into_bytes()
}

fn normalized_admin_page_theme(theme: Option<&str>) -> Option<&'static str> {
    match theme {
        Some("light") => Some("light"),
        Some("dark") => Some("dark"),
        Some("system") | None => Some("system"),
        Some(_) => None,
    }
}

fn head_insert_position(html: &str) -> Option<usize> {
    let lower_html = html.to_ascii_lowercase();
    let head_start = lower_html.find("<head")?;
    let head_end = lower_html[head_start..].find('>')?;

    Some(head_start + head_end + 1)
}

fn is_admin_html_asset(path: &str) -> bool {
    path.ends_with(".html")
}

fn rewrite_admin_page_style_links(html: &mut String, settings_version: i64) {
    let cache_busted_href = format!(
        "/admin/plugin-pages/style.css?oxide_settings_version={}",
        settings_version
    );

    *html = html
        .replace(
            r#"href="/admin/plugin-pages/style.css""#,
            &format!(r#"href="{}""#, cache_busted_href),
        )
        .replace(
            "href='/admin/plugin-pages/style.css'",
            &format!("href='{}'", cache_busted_href),
        );
}

pub(super) fn admin_pages_from_config(config: &PluginConfiguration) -> Vec<PluginAdminPageInfo> {
    let Some(manifest) = manifest_from_metadata(&config.metadata) else {
        return Vec::new();
    };

    admin_pages_from_manifest(
        &config.name,
        &config.version,
        config.enabled && matches!(config.status, PersistedPluginStatus::Enabled),
        &manifest,
    )
}

pub(super) fn admin_pages_from_manifest(
    plugin_name: &str,
    plugin_version: &str,
    enabled: bool,
    manifest: &PluginManifest,
) -> Vec<PluginAdminPageInfo> {
    manifest
        .admin
        .as_ref()
        .map(|admin| {
            admin
                .pages
                .iter()
                .filter(|page| {
                    is_valid_admin_page_slug(&page.slug) && is_allowed_admin_entry_path(&page.entry)
                })
                .map(|page| {
                    let asset_path = admin_asset_public_path(&page.entry);
                    PluginAdminPageInfo {
                        plugin_name: plugin_name.to_string(),
                        plugin_version: plugin_version.to_string(),
                        slug: page.slug.clone(),
                        title: page.title.clone(),
                        description: page.description.clone(),
                        icon: page.icon.clone(),
                        nav_group: page
                            .nav_group
                            .clone()
                            .unwrap_or_else(|| manifest.plugin.name.clone()),
                        admin_path: format!("/plugins/{}/pages/{}", plugin_name, page.slug),
                        source_url: format!(
                            "/admin/plugin-pages/assets/{}/{}",
                            plugin_name, asset_path
                        ),
                        enabled,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn validate_admin_pages(
    manifest: &PluginManifest,
    extraction_path: &std::path::Path,
) -> Result<(), ApiError> {
    let Some(admin) = &manifest.admin else {
        return Ok(());
    };

    for page in &admin.pages {
        if !is_valid_admin_page_slug(&page.slug) {
            return Err(ApiError::bad_request(format!(
                "Invalid admin page slug '{}': must be lowercase alphanumeric with hyphens/underscores",
                page.slug
            )));
        }

        if !is_allowed_admin_entry_path(&page.entry) {
            return Err(ApiError::bad_request(format!(
                "Invalid admin page entry '{}': entry must be an HTML file under the admin/ directory",
                page.entry
            )));
        }

        let asset_path = admin_asset_public_path(&page.entry);
        let entry_path = extraction_path.join("admin").join(&asset_path);
        if !entry_path.is_file() {
            return Err(ApiError::bad_request(format!(
                "Admin page '{}' declares missing entry file '{}'",
                page.slug, page.entry
            )));
        }
    }

    Ok(())
}

fn manifest_from_metadata(metadata: &serde_json::Value) -> Option<PluginManifest> {
    metadata
        .get("manifest")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

fn admin_asset_public_path(entry: &str) -> String {
    entry
        .trim_start_matches("./")
        .strip_prefix("admin/")
        .unwrap_or_else(|| entry.trim_start_matches("./"))
        .to_string()
}

fn is_valid_admin_page_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn is_allowed_admin_entry_path(path: &str) -> bool {
    let normalized = path.trim_start_matches("./");
    normalized.starts_with("admin/")
        && normalized.ends_with(".html")
        && is_allowed_admin_asset_path(normalized.strip_prefix("admin/").unwrap_or(normalized))
}

fn is_allowed_admin_asset_path(path: &str) -> bool {
    let mut has_component = false;

    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.starts_with('.') {
            return false;
        }

        if !part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return false;
        }

        has_component = true;
    }

    has_component
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_entry_paths_must_be_html_under_admin() {
        assert!(is_allowed_admin_entry_path("admin/index.html"));
        assert!(is_allowed_admin_entry_path("./admin/settings/index.html"));

        assert!(!is_allowed_admin_entry_path("index.html"));
        assert!(!is_allowed_admin_entry_path("admin/../index.html"));
        assert!(!is_allowed_admin_entry_path("admin/app.js"));
        assert!(!is_allowed_admin_entry_path("admin/.secret.html"));
    }

    #[test]
    fn shared_style_uses_site_branding_colors() {
        let branding = BrandingSettings {
            primary_color: Some("#ff0000".to_string()),
            secondary_color: Some("#00ff00".to_string()),
            ..Default::default()
        };

        let style = plugin_admin_style(&branding);

        assert!(style.contains("--primary: hsl(0 100% 50%);"));
        assert!(style.contains("--ring: hsl(0 100% 50%);"));
        assert!(style.contains("--secondary: hsl(120 100% 50%);"));
        assert!(style.contains(r#":root[data-oxide-theme="dark"]"#));
        assert!(style.contains(r#":root:not([data-oxide-theme="light"])"#));
    }

    #[test]
    fn shared_style_includes_custom_css_from_site_settings() {
        let branding = BrandingSettings {
            custom_css: Some(".oxide-panel { border-width: 2px; }".to_string()),
            ..Default::default()
        };

        let style = plugin_admin_style(&branding);

        assert!(style.contains(".oxide-panel { border-width: 2px; }"));
    }

    #[test]
    fn html_assets_receive_theme_hint_in_head() {
        let html = br#"<!doctype html><html><head><title>Demo</title></head><body></body></html>"#
            .to_vec();

        let themed = inject_admin_page_theme(html, Some("dark"), 42);
        let themed = String::from_utf8(themed);

        assert!(matches!(
            themed,
            Ok(content)
                if content.contains(r#"document.documentElement.dataset.oxideTheme = "dark""#)
                    && matches!(
                        (content.find("oxideTheme"), content.find("<title>")),
                        (Some(theme_at), Some(title_at)) if theme_at < title_at
                    )
        ));
    }

    #[test]
    fn invalid_theme_hint_is_ignored() {
        let html = br#"<!doctype html><html><head><title>Demo</title></head><body></body></html>"#
            .to_vec();

        let themed = inject_admin_page_theme(html, Some("surprise"), 42);
        let themed = String::from_utf8(themed);

        assert!(matches!(themed, Ok(content) if !content.contains("oxideTheme")));
    }

    #[test]
    fn html_assets_cache_bust_shared_style_with_settings_version() {
        let html = br#"<!doctype html><html><head><link rel="stylesheet" href="/admin/plugin-pages/style.css" /></head><body></body></html>"#
            .to_vec();

        let themed = inject_admin_page_theme(html, Some("system"), 1234);
        let themed = String::from_utf8(themed);

        assert!(matches!(
            themed,
            Ok(content)
                if content.contains(
                    r#"href="/admin/plugin-pages/style.css?oxide_settings_version=1234""#
                )
        ));
    }

    #[test]
    fn admin_pages_build_asset_urls_from_manifest() {
        let manifest = PluginManifest {
            plugin: PluginMetadata {
                name: "demo-plugin".to_string(),
                version: "1.0.0".to_string(),
                description: "Demo".to_string(),
                author: "OxideDB".to_string(),
                homepage: None,
                license: None,
                min_oxide_version: None,
                keywords: None,
                categories: None,
                changelog: None,
                build_timestamp: "2026-01-01T00:00:00Z".to_string(),
                wasm_file: "demo.wasm".to_string(),
            },
            security: PluginSecurity {
                required_capabilities: Vec::new(),
                recommended_trust_level: "Untrusted".to_string(),
                signature: None,
                certificate_chain: None,
                security_contact: None,
                security_advisories: None,
                audit_info: None,
            },
            admin: Some(PluginAdmin {
                pages: vec![PluginAdminPage {
                    slug: "settings".to_string(),
                    title: "Settings".to_string(),
                    entry: "admin/settings/index.html".to_string(),
                    description: None,
                    icon: Some("settings".to_string()),
                    nav_group: None,
                }],
            }),
            dependencies: None,
            config: None,
        };

        let pages = admin_pages_from_manifest("demo-plugin", "1.0.0", true, &manifest);

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].admin_path, "/plugins/demo-plugin/pages/settings");
        assert_eq!(
            pages[0].source_url,
            "/admin/plugin-pages/assets/demo-plugin/settings/index.html"
        );
        assert_eq!(pages[0].nav_group, "demo-plugin");
    }
}
