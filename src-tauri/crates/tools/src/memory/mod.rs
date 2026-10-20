//! The global memory list (`<userData>/memory.json`) and the `addMemory` / `updateMemory` /
//! `removeMemory` tools. The app's `memory_*` commands share the same [`MemoryService`].

mod service;
mod tools;

pub use service::{
    MemoryItem, MemoryKind, MemoryListener, MemoryPatch, MemoryService, MemorySource, MEMORY_FILE,
};
pub use tools::{create_memory_tools, AddMemoryTool, RemoveMemoryTool, UpdateMemoryTool};
