//! The Nova Reel `bedrock:*Video*` IPC handlers (`src/main/handlers/bedrock-handlers.ts`), for
//! `window.ipc.invoke`. Each reads the handler's params object as-is; settings come from the
//! store on every call. Timestamps in the results are ISO strings.

use super::ipc_params;
use crate::backend::Backend;
use crate::errors;
use crate::settings::aws_settings;
use crate::state::{store_all, StoreMutex};
use bedrock::video::{
    AsyncInvocationStatus, DownloadedVideo, GenerateMovieRequest, GeneratedMovie,
};
use serde_json::Value;
use tauri::ipc::Request;
use tauri::State;

fn movie_request(params: &Value) -> Result<GenerateMovieRequest, String> {
    serde_json::from_value(params.clone()).map_err(errors::plain)
}

fn str_param(params: &Value, key: &str) -> String {
    params
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// `bedrock:generateVideo` (`{ prompt, durationSeconds, outputPath?, seed?, s3Uri }`): blocks
/// until the job finishes (30 s polls, 30 min limit) and downloads to `outputPath`. Like the
/// TS handler, `inputImages` / `prompts` are not forwarded.
#[tauri::command]
pub async fn bedrock_generate_video(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<GeneratedMovie, String> {
    let mut req = movie_request(&ipc_params(&request))?;
    req.input_images = None;
    req.prompts = None;
    let aws = aws_settings(&store_all(&store));
    bedrock::video::generate_video(backend.sdk.as_ref(), &aws, &req)
        .await
        .map_err(errors::bedrock)
}

/// `bedrock:startVideoGeneration` (`GenerateMovieRequest`).
#[tauri::command]
pub async fn bedrock_start_video_generation(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<GeneratedMovie, String> {
    let req = movie_request(&ipc_params(&request))?;
    let aws = aws_settings(&store_all(&store));
    bedrock::video::start_video_generation(backend.sdk.as_ref(), &aws, &req)
        .await
        .map_err(errors::bedrock)
}

/// `bedrock:checkVideoStatus` (`{ invocationArn }`).
#[tauri::command]
pub async fn bedrock_check_video_status(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<AsyncInvocationStatus, String> {
    let arn = str_param(&ipc_params(&request), "invocationArn");
    let aws = aws_settings(&store_all(&store));
    bedrock::video::get_job_status(backend.sdk.as_ref(), &aws, &arn)
        .await
        .map_err(errors::bedrock)
}

/// `bedrock:downloadVideo` (`{ s3Uri, localPath }`) → `{ downloadedPath, fileSize }`.
#[tauri::command]
pub async fn bedrock_download_video(
    backend: State<'_, Backend>,
    store: State<'_, StoreMutex>,
    request: Request<'_>,
) -> Result<DownloadedVideo, String> {
    let params = ipc_params(&request);
    let aws = aws_settings(&store_all(&store));
    bedrock::video::download_video(
        backend.sdk.as_ref(),
        &aws,
        &str_param(&params, "s3Uri"),
        &str_param(&params, "localPath"),
    )
    .await
    .map_err(errors::bedrock)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn params_map_to_the_request() {
        let r = movie_request(&json!({
            "prompt": "p", "durationSeconds": 12, "s3Uri": "s3://b/v", "seed": 4,
            "inputImages": ["/a.png"], "prompts": ["x"], "outputPath": "/o"
        }))
        .unwrap();
        assert_eq!(r.prompt, "p");
        assert_eq!(r.duration_seconds, 12.0);
        assert_eq!(r.seed, Some(4.0));
        assert_eq!(r.output_path.as_deref(), Some("/o"));
        assert_eq!(r.prompts.as_deref(), Some(&["x".to_string()][..]));
        // Missing keys fall through to the service's own validation.
        let r = movie_request(&json!({})).unwrap();
        assert!(r.prompt.is_empty() && r.s3_uri.is_empty());
        assert!(movie_request(&json!({ "prompt": 1 })).is_err());
        assert_eq!(
            str_param(&json!({ "invocationArn": "arn:x" }), "invocationArn"),
            "arn:x"
        );
        assert_eq!(str_param(&json!({}), "s3Uri"), "");
    }
}
