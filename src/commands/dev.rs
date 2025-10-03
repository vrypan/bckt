use std::env;
use std::fs;
use std::net::ToSocketAddrs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use percent_encoding::percent_decode_str;
use tiny_http::{Header, Response, Server};

use crate::cli::DevArgs;
use crate::render::{BuildMode, RenderPlan, render_site};

const LIVE_RELOAD_ID: &str = "__bucket3_live_reload__";
const LIVE_RELOAD_SNIPPET: &str = r#"<script id=\"__bucket3_live_reload__\">(function(){if(window.__bucket3LiveReload){return;}window.__bucket3LiveReload=true;let last=0;async function poll(){try{const res=await fetch('/__bucket3__/poll?since='+last+'&_='+(Date.now()),{cache:'no-store'});if(res.ok){const data=await res.json();if(typeof data.timestamp==='number'){last=data.timestamp;}if(data.reload){window.location.reload();return;}}}catch(e){}setTimeout(poll,1000);}poll();})();</script>"#;

pub fn run_dev_command(args: DevArgs) -> Result<()> {
    let root = env::current_dir().context("failed to determine current directory")?;
    let html_root = root.join("html");
    fs::create_dir_all(&html_root).context("failed to create html directory")?;

    let initial_plan = RenderPlan {
        posts: true,
        static_assets: true,
        mode: if args.changed {
            BuildMode::Changed
        } else {
            BuildMode::Full
        },
        verbose: args.verbose,
    };
    render_site(&root, initial_plan).context("initial render before dev server failed")?;

    let latest_change = Arc::new(AtomicU64::new(now_timestamp()));
    let (tx, rx) = mpsc::channel();

    let watcher_tx = tx.clone();
    let mut watcher = notify::recommended_watcher(move |event| match event {
        Ok(_event) => {
            let _ = watcher_tx.send(());
        }
        Err(err) => {
            eprintln!("[bucket3::dev] watcher error: {err}");
        }
    })?;

    register_watch(&mut watcher, root.join("posts"))?;
    register_watch(&mut watcher, root.join("templates"))?;
    register_watch(&mut watcher, root.join("skel"))?;
    register_watch_file(&mut watcher, root.join("bucket3.yaml"))?;

    let rebuild_root = root.clone();
    let rebuild_verbose = args.verbose;
    let rebuild_mode = if args.changed {
        BuildMode::Changed
    } else {
        BuildMode::Full
    };
    let rebuild_latest = Arc::clone(&latest_change);

    thread::spawn(move || {
        while let Ok(()) = rx.recv() {
            while rx.try_recv().is_ok() {}
            let plan = RenderPlan {
                posts: true,
                static_assets: true,
                mode: rebuild_mode,
                verbose: rebuild_verbose,
            };
            if let Err(error) = render_site(&rebuild_root, plan) {
                eprintln!("[bucket3::dev] render error: {error}");
                continue;
            }
            rebuild_latest.store(now_timestamp(), Ordering::SeqCst);
        }
    });

    let address = format!("{}:{}", args.host, args.port);
    let listener_addr = address
        .to_socket_addrs()
        .context("invalid host/port combination")?
        .next()
        .context("failed to resolve dev server address")?;
    println!(
        "bucket3 dev server running at http://{}:{}",
        listener_addr.ip(),
        listener_addr.port()
    );

    let server = Server::http(listener_addr)
        .map_err(|err| anyhow::anyhow!("failed to start HTTP server: {err}"))?;

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let (path, query) = split_url(&url);
        if path == "/__bucket3__/poll" {
            let response = handle_poll(query, &latest_change);
            if let Err(err) = request.respond(response) {
                eprintln!("[bucket3::dev] respond error: {err}");
            }
            continue;
        }

        let response = serve_path(&html_root, path, &latest_change);
        if let Err(err) = request.respond(response) {
            eprintln!("[bucket3::dev] respond error: {err}");
        }
    }

    Ok(())
}

fn register_watch(watcher: &mut RecommendedWatcher, path: PathBuf) -> Result<()> {
    if path.exists() {
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", path.display()))?;
    }
    Ok(())
}

fn register_watch_file(watcher: &mut RecommendedWatcher, path: PathBuf) -> Result<()> {
    if path.exists() {
        watcher
            .watch(&path, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch {}", path.display()))?;
    }
    Ok(())
}

fn serve_path(
    html_root: &Path,
    raw_path: &str,
    latest_change: &Arc<AtomicU64>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match resolve_path(html_root, raw_path) {
        Ok((resolved, is_html)) => {
            if !resolved.exists() {
                return not_found();
            }
            if resolved.is_dir() {
                return not_found();
            }
            if is_html {
                match fs::read_to_string(&resolved) {
                    Ok(contents) => {
                        let body = inject_live_reload(&contents, latest_change);
                        let mut response = Response::from_string(body);
                        add_header(&mut response, "Content-Type", "text/html; charset=utf-8");
                        add_header(&mut response, "Cache-Control", "no-store, max-age=0");
                        response
                    }
                    Err(err) => internal_error(err.to_string()),
                }
            } else {
                match fs::read(&resolved) {
                    Ok(bytes) => {
                        let mut response = Response::from_data(bytes);
                        let mime = mime_guess::from_path(&resolved).first_or_octet_stream();
                        add_header(&mut response, "Content-Type", mime.essence_str());
                        add_header(&mut response, "Cache-Control", "no-store, max-age=0");
                        response
                    }
                    Err(err) => internal_error(err.to_string()),
                }
            }
        }
        Err(err) => {
            eprintln!("[bucket3::dev] path resolution error: {err}");
            forbidden()
        }
    }
}

fn resolve_path(html_root: &Path, raw_path: &str) -> Result<(PathBuf, bool)> {
    let mut path = raw_path.split('?').next().unwrap_or("");
    if path.starts_with('/') {
        path = &path[1..];
    }
    let decoded = percent_decode_str(path)
        .decode_utf8()
        .context("failed to decode URL path")?;
    let mut safe = PathBuf::new();
    if decoded.is_empty() {
        safe.push("index.html");
    } else {
        for component in Path::new(decoded.as_ref()).components() {
            match component {
                Component::Normal(part) => safe.push(part),
                Component::CurDir => {}
                _ => bail!("invalid path component"),
            }
        }
    }
    let candidate = html_root.join(&safe);
    if candidate.is_dir() {
        let fallback = candidate.join("index.html");
        Ok((fallback, true))
    } else {
        let is_html = candidate
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("html"))
            .unwrap_or(false);
        Ok((candidate, is_html))
    }
}

fn inject_live_reload(original: &str, latest_change: &Arc<AtomicU64>) -> String {
    if original.contains(LIVE_RELOAD_ID) {
        return original.to_string();
    }
    let mut rendered = original.to_string();
    let snippet = LIVE_RELOAD_SNIPPET.replace(
        "last=0",
        &format!("last={}", latest_change.load(Ordering::SeqCst)),
    );
    if let Some(index) = rendered.rfind("</body>") {
        rendered.insert_str(index, &snippet);
    } else if let Some(index) = rendered.rfind("</html>") {
        rendered.insert_str(index, &snippet);
    } else {
        rendered.push_str(&snippet);
    }
    rendered
}

fn handle_poll(
    query: Option<&str>,
    latest_change: &Arc<AtomicU64>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let since = query.and_then(|q| parse_since(q).ok()).unwrap_or(0);
    let current = latest_change.load(Ordering::SeqCst);
    let reload = current > since;
    let payload = format!(
        "{{\"reload\":{},\"timestamp\":{}}}",
        if reload { "true" } else { "false" },
        current
    );
    let mut response = Response::from_string(payload);
    add_header(&mut response, "Content-Type", "application/json");
    add_header(&mut response, "Cache-Control", "no-store, max-age=0");
    response
}

fn parse_since(query: &str) -> Result<u64> {
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=')
            && key == "since"
        {
            let decoded = percent_decode_str(value).decode_utf8()?;
            let parsed = decoded.parse::<u64>()?;
            return Ok(parsed);
        }
    }
    bail!("since not found")
}

fn split_url(url: &str) -> (&str, Option<&str>) {
    if let Some((path, query)) = url.split_once('?') {
        (path, Some(query))
    } else {
        (url, None)
    }
}

fn not_found() -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string("Not Found").with_status_code(404)
}

fn forbidden() -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string("Forbidden").with_status_code(403)
}

fn internal_error(message: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(message).with_status_code(500)
}

fn add_header(response: &mut Response<std::io::Cursor<Vec<u8>>>, key: &str, value: &str) {
    if let Ok(header) = Header::from_bytes(key.as_bytes(), value.as_bytes()) {
        response.add_header(header);
    }
}

fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_snippet_before_body() {
        let html = "<html><body><p>Hi</p></body></html>";
        let timestamp = Arc::new(AtomicU64::new(42));
        let with_reload = inject_live_reload(html, &timestamp);
        assert!(with_reload.contains(LIVE_RELOAD_ID));
        assert!(with_reload.contains("last=42"));
        assert!(with_reload.ends_with("</body></html>"));
    }

    #[test]
    fn injects_snippet_at_end_when_no_body() {
        let html = "<html><p>Hi</p></html>";
        let timestamp = Arc::new(AtomicU64::new(7));
        let with_reload = inject_live_reload(html, &timestamp);
        assert!(with_reload.contains(LIVE_RELOAD_ID));
        assert!(with_reload.ends_with("</html>"));
    }

    #[test]
    fn does_not_duplicate_snippet() {
        let html = format!("<html><body><div></div>{LIVE_RELOAD_SNIPPET}</body></html>");
        let timestamp = Arc::new(AtomicU64::new(99));
        let result = inject_live_reload(&html, &timestamp);
        let count = result.matches(LIVE_RELOAD_ID).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn parse_since_reads_query_value() {
        let since = parse_since("since=123&foo=bar").unwrap();
        assert_eq!(since, 123);
    }

    #[test]
    fn parse_since_fails_without_value() {
        assert!(parse_since("foo=bar").is_err());
    }
}
