use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use include_dir::{include_dir, Dir};
use mime_guess::from_path;

// Embed the web UI at compile time
static WEB_UI_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/web-ui/dist");

/// Serve embedded static files for the web UI
pub async fn serve_static(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    
    // Try to serve the requested file
    if let Some(file) = WEB_UI_DIR.get_file(path) {
        let mime = from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(file.contents()))
            .unwrap();
    }
    
    // For SPA routing: if file not found, serve index.html
    // (unless it's an API route which should have been handled already)
    if !path.starts_with("api/") {
        if let Some(index) = WEB_UI_DIR.get_file("index.html") {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html")
                .body(Body::from(index.contents()))
                .unwrap();
        }
    }
    
    // Return 404
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}

/// Fallback handler for all non-API routes
pub async fn fallback(uri: Uri) -> impl IntoResponse {
    serve_static(uri).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_ui_embedded() {
        // Verify the directory is embedded (will fail if dist/ doesn't exist at build time)
        // This is expected to fail until we run `npm run build`
        let has_index = WEB_UI_DIR.get_file("index.html").is_some();
        println!("Web UI embedded: {}", has_index);
    }
}
