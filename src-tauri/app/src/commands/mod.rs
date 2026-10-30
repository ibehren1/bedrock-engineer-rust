//! Tauri commands, one module per renderer bridge namespace (see `docs/port/BRIDGE.md`).
//!
//! To add commands: create `commands/<area>.rs`, declare it below, and append its commands to the
//! list in [`handler`]. Command names and argument keys must match the bridge table.

pub mod agent_files;
pub mod agents;
pub mod app_update;
pub mod attachments;
pub mod background;
pub mod bedrock;
pub mod camera;
pub mod chat_history;
pub mod docker;
pub mod file;
pub mod help;
pub mod knowledge;
pub mod local_terminal;
pub mod logger;
pub mod mcp;
pub mod memory;
pub mod screen;
pub mod store;
pub mod strands;
pub mod todo;
pub mod tools;
pub mod usage;
pub mod video;
pub mod window;
pub mod workspace_files;

use serde_json::Value;
use tauri::ipc::{InvokeBody, Request};

/// The whole argument object, for commands that take the Electron handler's params object as-is
/// (BRIDGE.md "params object as-is"). `window.ipc.invoke(channel, x)` with a non-object `x`
/// arrives as `{ args: [x] }`; that is unwrapped to `x`.
pub(crate) fn ipc_params(request: &Request<'_>) -> Value {
    let body = match request.body() {
        InvokeBody::Json(v) => v.clone(),
        InvokeBody::Raw(_) => Value::Null,
    };
    unwrap_args(body)
}

fn unwrap_args(body: Value) -> Value {
    match body {
        Value::Object(mut o) if o.len() == 1 && o.get("args").is_some_and(Value::is_array) => {
            match o.remove("args") {
                Some(Value::Array(mut a)) if !a.is_empty() => a.swap_remove(0),
                _ => Value::Null,
            }
        }
        other => other,
    }
}

/// The invoke handler registering every command.
pub fn handler() -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        // window.store
        store::store_get,
        store::store_set,
        store::store_delete,
        store::store_all,
        crate::store_sync::store_flush_done,
        // window.logger
        logger::logger_log,
        // window.chatHistory
        chat_history::chat_history_snapshot,
        chat_history::chat_history_create_session,
        chat_history::chat_history_add_message,
        chat_history::chat_history_get_session,
        chat_history::chat_history_update_session_title,
        chat_history::chat_history_delete_session,
        chat_history::chat_history_delete_sessions,
        chat_history::chat_history_delete_all_sessions,
        chat_history::chat_history_get_recent_sessions,
        chat_history::chat_history_get_all_session_metadata,
        chat_history::chat_history_set_active_session,
        chat_history::chat_history_get_active_session_id,
        chat_history::chat_history_update_message_content,
        chat_history::chat_history_delete_message,
        chat_history::chat_history_set_use_memory_items,
        chat_history::chat_history_add_touched_files,
        // window.workspaceFiles (Files panel)
        workspace_files::workspace_files_list,
        workspace_files::workspace_files_read_text,
        workspace_files::workspace_files_write_text,
        workspace_files::workspace_files_open,
        workspace_files::workspace_files_reveal,
        workspace_files::workspace_files_exists,
        workspace_files::workspace_files_watch,
        workspace_files::workspace_files_unwatch,
        // window.memory
        memory::memory_list,
        memory::memory_add,
        memory::memory_update,
        memory::memory_delete,
        // window.file / window.api (files and dialogs)
        file::open_file,
        file::open_directory,
        file::get_local_image,
        file::read_project_ignore,
        file::write_project_ignore,
        file::save_chat_to_markdown,
        file::save_chat_to_docx,
        file::save_chat_to_pdf,
        file::file_read_shared_agents,
        file::file_read_directory_agents,
        // window.file (agent files)
        agent_files::save_shared_agent,
        agent_files::delete_shared_agent,
        agent_files::export_agent_yaml,
        agent_files::import_agent_file,
        agent_files::load_organization_agents,
        agent_files::save_agent_to_organization,
        // lib/api.ts (the Express routes) and window.api.bedrock
        bedrock::converse_stream,
        bedrock::converse,
        bedrock::converse_cancel,
        bedrock::retrieve_and_generate,
        bedrock::list_models,
        bedrock::list_agent_tags,
        bedrock::get_structured_output,
        bedrock::get_website_recommendations,
        bedrock::bedrock_apply_guardrail,
        bedrock::bedrock_list_application_inference_profiles,
        bedrock::bedrock_translate_text,
        bedrock::bedrock_translate_batch,
        bedrock::bedrock_get_translation_cache,
        bedrock::bedrock_clear_translation_cache,
        bedrock::bedrock_get_translation_cache_stats,
        bedrock::bedrock_get_model_max_tokens,
        bedrock::bedrock_generate_image,
        bedrock::bedrock_recognize_image,
        bedrock::bedrock_retrieve,
        bedrock::bedrock_invoke_agent,
        bedrock::bedrock_invoke_flow,
        // bedrock:*Video* (Nova Reel)
        video::bedrock_generate_video,
        video::bedrock_start_video_generation,
        video::bedrock_check_video_status,
        video::bedrock_download_video,
        // tools
        tools::bedrock_execute_tool,
        tools::tools_execute,
        tools::tools_get_tool_specs,
        // window.api.chatAttachments
        attachments::chat_attachments_list,
        attachments::chat_attachments_with_files,
        attachments::chat_attachments_add,
        attachments::chat_attachments_add_from_picker,
        attachments::chat_attachments_remove,
        attachments::chat_attachments_remove_all,
        attachments::chat_attachments_remove_every_folder,
        attachments::chat_attachments_rename,
        attachments::chat_attachments_build_context,
        attachments::chat_attachments_open_folder,
        // window.api.help
        help::help_prepare_user_guide,
        help::help_user_guide,
        help::help_search,
        // window.api.todo
        todo::todo_init,
        todo::todo_update,
        todo::get_todo_list,
        todo::delete_todo_list,
        todo::get_recent_todos,
        todo::get_all_todo_metadata,
        todo::set_active_todo_list,
        todo::get_active_todo_list_id,
        // window.api.mcp
        mcp::mcp_init,
        mcp::mcp_get_tools,
        mcp::mcp_execute_tool,
        mcp::mcp_test_connection,
        mcp::mcp_test_all_connections,
        mcp::mcp_search_registry,
        mcp::mcp_cleanup,
        // window.api.subAgent
        agents::sub_agent_invoke,
        // window.api.strandsConverter
        strands::convert_agent_to_strands,
        // window.api.screen / screen:* channels
        screen::screen_list_available_windows,
        screen::screen_capture,
        screen::screen_check_permissions,
        // window.api.camera (+ the cameraCapture tool's webview frames)
        camera::camera_save_captured_image,
        camera::camera_show_preview_window,
        camera::camera_hide_preview_window,
        camera::camera_close_preview_window,
        camera::camera_update_preview_settings,
        camera::camera_get_preview_status,
        camera::camera_capture_response,
        // window.api.window
        window::window_open_task_history,
        window::window_open_file_viewer,
        // app menu / context menu (src/main/index.ts createMenu, context-menu)
        crate::menu::context_menu_popup,
        crate::menu::app_menu_action,
        // window.api.backgroundAgent
        background::background_agent_chat,
        background::background_agent_create_session,
        background::background_agent_delete_session,
        background::background_agent_list_sessions,
        background::background_agent_get_session_history,
        background::background_agent_get_session_stats,
        background::background_agent_get_all_sessions_metadata,
        background::background_agent_get_sessions_by_project,
        background::background_agent_get_sessions_by_agent,
        background::background_agent_schedule_task,
        background::background_agent_update_task,
        background::background_agent_cancel_task,
        background::background_agent_toggle_task,
        background::background_agent_list_tasks,
        background::background_agent_get_task,
        background::background_agent_get_task_execution_history,
        background::background_agent_execute_task_manually,
        background::background_agent_get_scheduler_stats,
        background::background_agent_continue_session,
        background::background_agent_get_task_system_prompt,
        background::background_agent_task_notification,
        // window.api.pubsub
        background::pubsub_subscribe,
        background::pubsub_unsubscribe,
        background::pubsub_publish,
        background::pubsub_stats,
        // window.api.dockerSandbox / codeInterpreter
        docker::check_docker_availability,
        docker::docker_sandbox_availability,
        docker::docker_sandbox_create,
        docker::docker_sandbox_status,
        docker::docker_sandbox_start,
        docker::docker_sandbox_stop,
        docker::docker_sandbox_remove,
        docker::docker_sandbox_rename,
        docker::docker_sandbox_logs,
        docker::docker_sandbox_exec,
        docker::docker_sandbox_send_input,
        docker::docker_sandbox_has_pid,
        docker::docker_sandbox_list,
        docker::docker_sandbox_open_folder,
        docker::docker_sandbox_open_port,
        docker::docker_sandbox_insights,
        docker::docker_sandbox_compose,
        docker::docker_sandbox_activity,
        docker::docker_sandbox_terminal_capability,
        docker::docker_sandbox_terminal_open,
        docker::docker_sandbox_terminal_attach,
        docker::docker_sandbox_terminal_input,
        docker::docker_sandbox_terminal_resize,
        docker::docker_sandbox_terminal_backlog,
        docker::docker_sandbox_terminal_close,
        // window.api.localTerminal (main window only: capabilities/main-window.json)
        local_terminal::local_terminal_open,
        local_terminal::local_terminal_attach,
        local_terminal::local_terminal_input,
        local_terminal::local_terminal_resize,
        local_terminal::local_terminal_close,
        local_terminal::local_terminal_list,
        local_terminal::local_terminal_backlog,
        app_update::app_update_current_version,
        app_update::app_update_check,
        app_update::app_update_download,
        app_update::app_update_restart,
        // window.api.knowledge (main window only: capabilities/main-window.json)
        knowledge::knowledge_list,
        knowledge::knowledge_create,
        knowledge::knowledge_update,
        knowledge::knowledge_delete,
        knowledge::knowledge_delete_agent,
        knowledge::knowledge_copy_agent,
        knowledge::knowledge_rebuild,
        knowledge::knowledge_list_sources,
        knowledge::knowledge_pick_files,
        knowledge::knowledge_pick_folder,
        knowledge::knowledge_add_url,
        knowledge::knowledge_update_source_options,
        knowledge::knowledge_resync_source,
        knowledge::knowledge_remove_source,
        knowledge::knowledge_jobs,
        knowledge::knowledge_cancel_job,
        knowledge::knowledge_search,
        knowledge::knowledge_stats,
        knowledge::knowledge_optimize,
        knowledge::knowledge_storage_overview,
        knowledge::knowledge_purge_orphans,
        knowledge::knowledge_local_model_status,
        knowledge::knowledge_download_local_model,
        knowledge::knowledge_delete_local_model,
        // window.api.usage (main window only: capabilities/main-window.json)
        usage::usage_summary,
        usage::usage_breakdown,
        usage::usage_months,
        usage::usage_events,
        usage::usage_info,
        usage::usage_export_csv,
        usage::usage_budget_status,
        usage::session_usage_costs,
    ]
}

#[cfg(test)]
mod acl_tests;

#[cfg(test)]
mod tests {
    use super::unwrap_args;
    use serde_json::json;

    #[test]
    fn params_object_is_passed_through() {
        let v = json!({ "agentId": "a", "task": "t" });
        assert_eq!(unwrap_args(v.clone()), v);
    }

    #[test]
    fn positional_ipc_args_are_unwrapped() {
        assert_eq!(
            unwrap_args(json!({ "args": [{ "x": 1 }, 2] })),
            json!({ "x": 1 })
        );
        assert_eq!(unwrap_args(json!({ "args": [] })), json!(null));
        // A real `args` key next to others is data, not the positional wrapper.
        let v = json!({ "args": [1], "other": true });
        assert_eq!(unwrap_args(v.clone()), v);
    }
}
