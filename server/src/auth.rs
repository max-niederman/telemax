use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

#[derive(Clone, Debug)]
pub struct TailscaleIdentity {
    pub login: String,
    pub name: Option<String>,
}

pub async fn require_tailscale_auth(mut req: Request, next: Next) -> Response {
    let login = req
        .headers()
        .get("Tailscale-User-Login")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let name = req
        .headers()
        .get("Tailscale-User-Name")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    match login {
        Some(login) => {
            let identity = TailscaleIdentity { login, name };
            req.extensions_mut().insert(identity);
            next.run(req).await
        }
        None => (
            StatusCode::FORBIDDEN,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: "AUTH_REQUIRED",
                    message: "Tailscale-User-Login header missing. Access via Tailscale Serve.",
                },
            }),
        )
            .into_response(),
    }
}
