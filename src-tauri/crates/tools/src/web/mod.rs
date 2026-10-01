//! Web tools (`src/preload/tools/handlers/web`) and the main-process `fetch-website` /
//! `save-website-content` handlers they call.

mod fetch_website;
mod http;
mod save;
mod tavily;

pub use fetch_website::{extract_main_content, truncate_content_middle_out, FetchWebsiteTool};
pub use http::{fetch_website, FetchOptions, FetchResponse};
pub use save::{save_website_content, SaveFormat, SaveResult};
pub use tavily::{TavilySearchTool, TAVILY_ENDPOINT};
