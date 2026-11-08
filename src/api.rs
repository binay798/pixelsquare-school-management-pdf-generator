//! HTTP layer: router, handlers and response types.
//!
//! Endpoints:
//!
//! | Method | Path                     | Purpose                                     |
//! |--------|--------------------------|---------------------------------------------|
//! | `GET`  | `/health`                | Liveness probe.                             |
//! | `GET`  | `/api/v1/templates`      | Lists the available templates.              |
//! | `POST` | `/api/v1/pdf`            | Renders any template with a JSON payload.   |
//! | `POST` | `/api/v1/invoices`       | Renders the invoice template from typed data.|
//! | `POST` | `/api/v1/report-cards`   | Renders the report card template from typed data.|
//! | `GET`  | `/api/v1/pdfs/{name}`    | Downloads a previously generated PDF.       |
//!
//! Successful responses use `{ "success": true, "data": … }`, failures use the
//! envelope from [`crate::error`].

use std::sync::Arc;
use std::time::Instant;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::error::{AppError, AppResult};
use crate::models::{InvoicePayload, ReportCardPayload};
use crate::pdf::storage::{self, StoredPdf};
use crate::state::AppState;

/// Name of the template that is rendered by `/api/v1/invoices`.
pub const INVOICE_TEMPLATE: &str = "invoice";

/// Name of the template that is rendered by `/api/v1/report-cards`.
pub const REPORT_CARD_TEMPLATE: &str = "report_card";

/// Builds the router with all routes and middleware.
pub fn router(state: Arc<AppState>) -> Router {
    let body_limit = state.config().max_payload_bytes;

    Router::new()
        .route("/health", get(health))
        .route("/api/v1/templates", get(list_templates))
        .route("/api/v1/pdf", post(generate_pdf))
        .route("/api/v1/invoices", post(generate_invoice))
        .route("/api/v1/report-cards", post(generate_report_card))
        .route("/api/v1/pdfs/{file_name}", get(download_pdf))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(body_limit))
        .with_state(state)
}

/// Envelope for successful responses.
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// Always `true` for successful responses.
    pub success: bool,
    /// The payload of the response.
    pub data: T,
}

impl<T> ApiResponse<T> {
    fn new(data: T) -> Self {
        Self {
            success: true,
            data,
        }
    }
}

/// Body of `POST /api/v1/pdf`.
#[derive(Debug, Deserialize)]
pub struct PdfRequest {
    /// Name of the template to render, e.g. `invoice`.
    pub template: String,
    /// Arbitrary JSON that is exposed to the template as `sys.inputs.data`.
    #[serde(default)]
    pub data: Value,
}

/// Response of the render endpoints.
#[derive(Debug, Serialize)]
pub struct PdfResponse {
    /// Name of the template that was rendered.
    pub template: String,
    /// File name inside the output directory.
    pub file_name: String,
    /// Path of the generated file.
    pub path: String,
    /// Size of the PDF in bytes.
    pub size_bytes: usize,
    /// Amount of pages in the document.
    pub pages: usize,
    /// URL the document can be downloaded from.
    pub download_url: String,
    /// How long the request took, in milliseconds.
    pub duration_ms: u128,
}

impl PdfResponse {
    fn new(template: &str, stored: StoredPdf, elapsed_ms: u128) -> Self {
        Self {
            template: template.to_string(),
            file_name: stored.file_name.clone(),
            path: stored.path.display().to_string(),
            size_bytes: stored.size_bytes,
            pages: stored.pages,
            download_url: format!("/api/v1/pdfs/{}", stored.file_name),
            duration_ms: elapsed_ms,
        }
    }
}

/// Response of `GET /health`.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Always `ok`; the endpoint only answers when the service is up.
    pub status: &'static str,
    /// Version of the binary.
    pub version: &'static str,
    /// Amount of available templates.
    pub templates: usize,
}

/// Response of `GET /api/v1/templates`.
#[derive(Debug, Serialize)]
pub struct TemplatesResponse {
    /// Sorted list of template names.
    pub templates: Vec<String>,
}

/// Liveness probe.
async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        templates: state.engine().templates().list().len(),
    })
}

/// Lists the template names that can be rendered.
async fn list_templates(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<TemplatesResponse>> {
    Json(ApiResponse::new(TemplatesResponse {
        templates: state.engine().templates().list(),
    }))
}

/// Renders an arbitrary template with an arbitrary JSON payload.
async fn generate_pdf(
    State(state): State<Arc<AppState>>,
    payload: Result<Json<PdfRequest>, JsonRejection>,
) -> AppResult<Json<ApiResponse<PdfResponse>>> {
    let started = Instant::now();
    let Json(request) = payload.map_err(reject)?;

    // A payload without `data` is rendered as an empty document.
    let data = if request.data.is_null() {
        json!({})
    } else {
        request.data
    };

    let response = render_and_store(&state, &request.template, &data, started).await?;
    Ok(Json(ApiResponse::new(response)))
}

/// Renders the invoice template from a typed invoice payload.
async fn generate_invoice(
    State(state): State<Arc<AppState>>,
    payload: Result<Json<InvoicePayload>, JsonRejection>,
) -> AppResult<Json<ApiResponse<PdfResponse>>> {
    let started = Instant::now();
    let Json(invoice) = payload.map_err(reject)?;

    // Business rules are checked in the model; the template only renders.
    let data = invoice.to_template_data().map_err(AppError::validation)?;

    let response = render_and_store(&state, INVOICE_TEMPLATE, &data, started).await?;
    Ok(Json(ApiResponse::new(response)))
}

/// Renders the report card template from a typed report card payload.
async fn generate_report_card(
    State(state): State<Arc<AppState>>,
    payload: Result<Json<ReportCardPayload>, JsonRejection>,
) -> AppResult<Json<ApiResponse<PdfResponse>>> {
    let started = Instant::now();
    let Json(report_card) = payload.map_err(reject)?;

    // Grades, GPA and attendance rates are computed in the model.
    let data = report_card
        .to_template_data()
        .map_err(AppError::validation)?;

    let response = render_and_store(&state, REPORT_CARD_TEMPLATE, &data, started).await?;
    Ok(Json(ApiResponse::new(response)))
}

/// Serves a previously generated PDF.
async fn download_pdf(
    State(state): State<Arc<AppState>>,
    Path(file_name): Path<String>,
) -> AppResult<Response> {
    let dir = state.output_dir().to_path_buf();
    let name = file_name.clone();

    let bytes = spawn_blocking(move || storage::load(&dir, &name)).await?;

    let disposition = HeaderValue::from_str(&format!("inline; filename=\"{file_name}\""))
        .map_err(|err| AppError::InvalidPayload(format!("invalid document name: {err}")))?;

    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/pdf"),
            ),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response())
}

/// Compiles a document and writes it to the output directory.
async fn render_and_store(
    state: &AppState,
    template: &str,
    data: &Value,
    started: Instant,
) -> AppResult<PdfResponse> {
    let rendered = state.engine().render(template, data).await?;

    // Writing the file is blocking I/O, so it runs on a blocking thread too.
    let dir = state.output_dir().to_path_buf();
    let stored = spawn_blocking(move || storage::save(&dir, &rendered)).await?;

    let elapsed_ms = started.elapsed().as_millis();
    tracing::info!(
        template,
        file = %stored.file_name,
        pages = stored.pages,
        size_bytes = stored.size_bytes,
        duration_ms = elapsed_ms,
        "pdf generated"
    );

    // The name is echoed back so the client sees what was actually rendered.
    Ok(PdfResponse::new(template.trim(), stored, elapsed_ms))
}

/// Runs a blocking operation on the blocking thread pool.
async fn spawn_blocking<T, F>(task: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|err| AppError::Internal(format!("background task failed: {err}")))?
}

/// Turns a JSON rejection into the service error envelope.
fn reject(rejection: JsonRejection) -> AppError {
    match rejection.status() {
        StatusCode::PAYLOAD_TOO_LARGE => AppError::PayloadTooLarge,
        StatusCode::UNSUPPORTED_MEDIA_TYPE => AppError::InvalidPayload(
            "expected a `content-type: application/json` request".to_string(),
        ),
        _ => AppError::InvalidPayload(rejection.body_text()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;
    use uuid::Uuid;

    /// Builds a state that renders with the embedded fonts into a temp dir.
    fn test_state() -> (std::path::PathBuf, Arc<AppState>) {
        test_state_with(|_| {})
    }

    /// Same as [`test_state`], but lets the caller tune the configuration.
    fn test_state_with(tune: impl FnOnce(&mut AppConfig)) -> (std::path::PathBuf, Arc<AppState>) {
        let output_dir = std::env::temp_dir().join(format!("pdf-gen-api-{}", Uuid::new_v4()));

        let mut config = AppConfig {
            templates_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates"),
            output_dir: output_dir.clone(),
            max_concurrent_renders: 2,
            load_system_fonts: false,
            ..AppConfig::default()
        };

        tune(&mut config);

        let state = AppState::with_embedded_fonts(config).expect("state can be built");
        (output_dir, state)
    }

    fn invoice_payload() -> Value {
        json!({
            "invoice_number": "INV-2026-0001",
            "issue_date": "2026-11-08",
            "due_date": "2026-12-08",
            "currency": "USD",
            "seller": {
                "name": "Pixelsquare",
                "address": ["Kathmandu", "Nepal"],
                "email": "billing@pixelsquare.io"
            },
            "buyer": {
                "name": "Springfield School",
                "address": ["Main Street 1"]
            },
            "items": [
                { "description": "Tuition fee", "quantity": 2, "unit": "month", "unit_price": 1500, "tax_rate": 10 },
                { "description": "Lab fee", "unit_price": 250.5 }
            ],
            "discount": 100,
            "notes": "Thank you for your business."
        })
    }

    fn report_card_payload() -> Value {
        json!({
            "student_name": "Aarav Shrestha",
            "student_id": "STU-2026-0142",
            "grade": "Grade 8",
            "section": "A",
            "roll_number": "14",
            "academic_year": "2026/2027",
            "term": "First Term",
            "issued_date": "2026-11-08",
            "next_term_begins": "2027-01-12",
            "school": {
                "name": "Springfield International School",
                "address": ["12 Main Street", "Springfield"],
                "email": "office@springfield.edu"
            },
            "guardian": { "name": "Nisha Shrestha", "phone": "+977 9801 234 567" },
            "subjects": [
                { "name": "Mathematics", "code": "MATH-8", "credit_hours": 4, "marks_obtained": 92, "marks_total": 100, "teacher": "Mr. R. Karki" },
                { "name": "English", "code": "ENG-8", "credit_hours": 4, "marks_obtained": 84.5, "marks_total": 100 },
                { "name": "Science", "credit_hours": 3.5, "marks_obtained": 61, "marks_total": 75 }
            ],
            "attendance": { "present_days": 78, "total_days": 84 },
            "activities": ["Debate club"],
            "class_teacher": "Ms. L. Gurung",
            "class_teacher_remarks": "Aarav worked hard this term.",
            "principal": "Dr. M. Tamang",
            "class_position": 3,
            "class_size": 42
        })
    }

    async fn body_json(response: Response) -> Value {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body can be read")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("body is JSON")
    }

    async fn post_json(state: &Arc<AppState>, uri: &str, body: Value) -> Response {
        router(Arc::clone(state))
            .oneshot(
                Request::post(uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .expect("request can be built"),
            )
            .await
            .expect("handler answers")
    }

    async fn get_json(state: &Arc<AppState>, uri: &str) -> Response {
        router(Arc::clone(state))
            .oneshot(
                Request::get(uri)
                    .body(Body::empty())
                    .expect("request can be built"),
            )
            .await
            .expect("handler answers")
    }

    #[tokio::test]
    async fn health_reports_ok() {
        let (_dir, state) = test_state();

        let response = get_json(&state, "/health").await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = body_json(response).await;
        assert_eq!(body["status"], json!("ok"));
        assert_eq!(body["templates"], json!(2));
    }

    #[tokio::test]
    async fn renders_an_invoice_and_stores_the_pdf() {
        let (output_dir, state) = test_state();

        let response = post_json(&state, "/api/v1/invoices", invoice_payload()).await;
        let status = response.status();
        let body = body_json(response).await;
        assert_eq!(status, StatusCode::OK, "unexpected response: {body}");
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["template"], json!("invoice"));
        assert_eq!(body["data"]["pages"], json!(1));
        assert!(body["data"]["size_bytes"].as_u64().expect("size") > 1000);

        let file_name = body["data"]["file_name"].as_str().expect("file name");
        let bytes = std::fs::read(output_dir.join(file_name)).expect("pdf was written");
        assert!(bytes.starts_with(b"%PDF"), "expected a PDF header");

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[tokio::test]
    async fn renders_a_report_card_and_stores_the_pdf() {
        let (output_dir, state) = test_state();

        let response = post_json(&state, "/api/v1/report-cards", report_card_payload()).await;
        let status = response.status();
        let body = body_json(response).await;
        assert_eq!(status, StatusCode::OK, "unexpected response: {body}");
        assert_eq!(body["success"], json!(true));
        assert_eq!(body["data"]["template"], json!("report_card"));
        assert_eq!(body["data"]["pages"], json!(1));
        assert!(body["data"]["size_bytes"].as_u64().expect("size") > 1000);

        let file_name = body["data"]["file_name"].as_str().expect("file name");
        let bytes = std::fs::read(output_dir.join(file_name)).expect("pdf was written");
        assert!(bytes.starts_with(b"%PDF"), "expected a PDF header");

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[tokio::test]
    async fn rejects_invalid_report_cards() {
        let (_dir, state) = test_state();

        let mut payload = report_card_payload();
        payload["subjects"] = json!([]);
        payload["issued_date"] = json!("08/11/2026");

        let response = post_json(&state, "/api/v1/report-cards", payload).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = body_json(response).await;
        assert_eq!(body["error"]["code"], json!("VALIDATION_FAILED"));
        assert_eq!(
            body["error"]["details"].as_array().expect("details").len(),
            2
        );
    }

    #[tokio::test]
    async fn renders_through_the_generic_endpoint() {
        let (output_dir, state) = test_state();

        // The generic endpoint receives the view model instead of raw invoice
        // fields, so the same template can be driven by any producer.
        let invoice: InvoicePayload =
            serde_json::from_value(invoice_payload()).expect("payload is valid");
        let data = invoice.to_template_data().expect("view model can be built");

        let response = post_json(
            &state,
            "/api/v1/pdf",
            json!({ "template": "invoice", "data": data }),
        )
        .await;

        let status = response.status();
        let body = body_json(response).await;
        assert_eq!(status, StatusCode::OK, "unexpected response: {body}");
        assert_eq!(body["data"]["template"], json!("invoice"));

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[tokio::test]
    async fn rejects_invalid_invoices() {
        let (_dir, state) = test_state();

        let mut payload = invoice_payload();
        payload["items"] = json!([]);
        payload["issue_date"] = json!("08/11/2026");

        let response = post_json(&state, "/api/v1/invoices", payload).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let body = body_json(response).await;
        assert_eq!(body["success"], json!(false));
        assert_eq!(body["error"]["code"], json!("VALIDATION_FAILED"));
        assert_eq!(
            body["error"]["details"].as_array().expect("details").len(),
            2
        );
    }

    #[tokio::test]
    async fn rejects_broken_json() {
        let (_dir, state) = test_state();

        let response = router(Arc::clone(&state))
            .oneshot(
                Request::post("/api/v1/pdf")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{ not json"))
                    .expect("request can be built"),
            )
            .await
            .expect("handler answers");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = body_json(response).await;
        assert_eq!(body["error"]["code"], json!("INVALID_PAYLOAD"));
    }

    #[tokio::test]
    async fn rejects_oversized_bodies() {
        let (_dir, state) = test_state_with(|config| config.max_payload_bytes = 128);

        let response = post_json(&state, "/api/v1/invoices", invoice_payload()).await;
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let body = body_json(response).await;
        assert_eq!(body["error"]["code"], json!("PAYLOAD_TOO_LARGE"));
    }

    #[tokio::test]
    async fn reports_unknown_templates() {
        let (_dir, state) = test_state();

        let response = post_json(
            &state,
            "/api/v1/pdf",
            json!({ "template": "missing", "data": {} }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = body_json(response).await;
        assert_eq!(body["error"]["code"], json!("TEMPLATE_NOT_FOUND"));
    }

    #[tokio::test]
    async fn downloads_a_generated_pdf() {
        let (output_dir, state) = test_state();

        let generated =
            body_json(post_json(&state, "/api/v1/invoices", invoice_payload()).await).await;
        let download_url = generated["data"]["download_url"]
            .as_str()
            .expect("download url");

        let response = get_json(&state, download_url).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            HeaderValue::from_static("application/pdf")
        );

        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body can be read")
            .to_bytes();
        assert!(bytes.starts_with(b"%PDF"));

        std::fs::remove_dir_all(&output_dir).ok();
    }
}
