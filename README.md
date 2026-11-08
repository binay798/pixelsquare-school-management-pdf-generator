# pixelsquare-school-management-pdf-generator

A small Rust microservice that turns JSON into PDFs. It renders Typst templates
(`templates/*.typ`) through the [Typst](https://typst.app) compiler and stores the
result in `pdfs/`, so other services can render invoices, student report cards
(and future documents) without embedding a typesetting engine themselves.

* HTTP API: `axum`
* Rendering: `typst` + `typst-pdf`
* Fonts: the fonts embedded in `typst-assets` (Libertinus Serif, DejaVu, New Computer Modern, …)
  plus the host fonts when they are enabled.

## Running

```sh
cargo run                 # listens on 0.0.0.0:8080
RUST_LOG=debug cargo run  # verbose logs
```

Smoke test with the bundled samples:

```sh
curl -s -X POST http://localhost:8080/api/v1/invoices \
  -H 'content-type: application/json' \
  --data @samples/invoice.json

curl -s -X POST http://localhost:8080/api/v1/report-cards \
  -H 'content-type: application/json' \
  --data @samples/report_card.json
```

```json
{
  "success": true,
  "data": {
    "template": "invoice",
    "file_name": "cab46aaa-fa98-4384-b602-fd98ed588cef.pdf",
    "path": "pdfs/cab46aaa-fa98-4384-b602-fd98ed588cef.pdf",
    "size_bytes": 32781,
    "pages": 1,
    "download_url": "/api/v1/pdfs/cab46aaa-fa98-4384-b602-fd98ed588cef.pdf",
    "duration_ms": 97
  }
}
```

## API

| Method | Path                   | Description                                                     |
| ------ | ---------------------- | --------------------------------------------------------------- |
| `GET`  | `/health`              | Liveness probe, also reports the number of available templates   |
| `GET`  | `/api/v1/templates`    | Lists the templates that can be rendered                         |
| `POST` | `/api/v1/pdf`          | Renders any template: `{ "template": "invoice", "data": { … } }` |
| `POST` | `/api/v1/invoices`     | Validates an invoice payload, then renders the `invoice` template |
| `POST` | `/api/v1/report-cards` | Validates a report card payload, then renders the `report_card` template |
| `GET`  | `/api/v1/pdfs/{name}`  | Downloads a previously generated PDF (`application/pdf`)         |

`POST /api/v1/invoices` accepts the invoice fields (`invoice_number`, `issue_date`,
`due_date`, `currency`, `status`, `po_number`, `seller`, `buyer`, `items`,
`discount`, `shipping`, `notes`, `payment_terms`) — see `samples/invoice.json` and
`src/models/invoice.rs`. The service validates the payload, computes every amount
and hands preformatted strings (`amount_display`, `total_display`, …) to the
template, so the template never does arithmetic.

`POST /api/v1/report-cards` accepts the report card fields (`student_name`,
`student_id`, `grade`, `section`, `roll_number`, `academic_year`, `term`,
`issued_date`, `next_term_begins`, `school`, `guardian`, `subjects`, `attendance`,
`activities`, `class_teacher`, `class_teacher_remarks`, `principal`,
`class_position`, `class_size`) — see `samples/report_card.json` and
`src/models/report_card.rs`. Every subject carries `marks_obtained`/`marks_total`
and an optional `credit_hours` weight; the service derives the subject percentage,
letter grade and grade points, the credit weighted GPA, the overall percentage and
result (`PASS`/`FAIL`), the attendance rate and a low-attendance flag, and sends
them to the template as `*_display` strings.

The grading scale (`A+` ≥ 90% … `F` < 35%), the 35% pass mark and the 75% attendance
warning threshold live in `src/models/report_card.rs` and are rendered as a legend
on the document, so API and template never disagree.

Errors always use the same envelope:

```json
{ "success": false, "error": { "code": "VALIDATION_FAILED", "message": "…", "details": ["…"] } }
```

| Status | `code`                   | Meaning                                          |
| ------ | ------------------------ | ------------------------------------------------ |
| 400    | `INVALID_PAYLOAD`        | Body is not valid JSON or has the wrong shape    |
| 400    | `INVALID_TEMPLATE_NAME`  | Template name contains illegal characters        |
| 404    | `TEMPLATE_NOT_FOUND`     | No such template                                 |
| 404    | `DOCUMENT_NOT_FOUND`     | No such PDF in the output directory              |
| 413    | `PAYLOAD_TOO_LARGE`      | Body exceeds `PDF_GEN_MAX_PAYLOAD_BYTES`         |
| 422    | `VALIDATION_FAILED`      | Payload violates the business rules              |
| 422    | `TEMPLATE_ERROR`         | Typst failed to compile the template             |
| 500    | `PDF_EXPORT_ERROR`       | Typst could not export the compiled document     |
| 500    | `STORAGE_ERROR`          | The PDF could not be written or read             |
| 500    | `INTERNAL_ERROR`         | Unexpected failure                               |

Renders are CPU bound: at most `PDF_GEN_MAX_CONCURRENT_RENDERS` documents are
compiled at the same time, further requests wait in the queue.

## Configuration

Every value can be overridden through an environment variable:

| Variable                         | Default            | Description                                                |
| -------------------------------- | ------------------ | ---------------------------------------------------------- |
| `PDF_GEN_HOST`                   | `0.0.0.0`          | Bind address                                               |
| `PDF_GEN_PORT`                   | `8080`             | Bind port                                                  |
| `PDF_GEN_TEMPLATES_DIR`          | `templates`        | Directory with the `.typ` templates                         |
| `PDF_GEN_OUTPUT_DIR`             | `pdfs`             | Directory the PDFs are written to                           |
| `PDF_GEN_MAX_PAYLOAD_BYTES`      | `2097152` (2 MiB)  | Maximum request body size                                   |
| `PDF_GEN_MAX_CONCURRENT_RENDERS` | CPU count          | Parallel Typst compilations                                 |
| `PDF_GEN_DISABLE_SYSTEM_FONTS`   | `false`            | Set to `1` to only use the embedded fonts                   |
| `PDF_GEN_FONT_DIRS`              | –                  | Extra font directories, separated by `:` (`;` on Windows)   |
| `RUST_LOG`                       | `info`             | Log filter                                                  |

## Layout

```
src/
  main.rs            entry point: logging, state, axum server, graceful shutdown
  api.rs             routes, request/response envelopes, HTTP tests
  state.rs           shared application state
  config.rs          environment based configuration
  error.rs           AppError + JSON error envelope
  models/
    format.rs        shared number/date/percentage formatting
    invoice.rs       invoice payload, validation, view-model construction
    report_card.rs   report card payload, grading, view-model construction
  pdf/
    engine.rs        TypstEngine: semaphore + blocking compile + PDF export
    templates.rs     template discovery, name validation, virtual paths
    world.rs         typst::World implementation (inputs, files, fonts, time)
    fonts.rs         FontStore wrapper (embedded + optional system fonts)
    storage.rs       atomic writes to the output directory
templates/invoice.typ      the invoice layout
templates/report_card.typ  the student report card layout
samples/invoice.json       example invoice payload
samples/report_card.json   example report card payload
```

## Tests

```sh
cargo test            # unit tests + HTTP integration tests (tower::oneshot)
cargo clippy --all-targets
```

The tests render real PDFs with the embedded fonts, so no network access or
installed fonts are required.
