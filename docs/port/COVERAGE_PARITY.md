# Port Coverage Parity

Tracks every Electron `*.test.ts` file (excluding `node_modules`) and where its coverage lands in
the Rust/Tauri port.

- **destination** — `Rust: <crate>` (ported to a cargo test), `Jest (retained)` (stays a
  renderer test), `Rust #[ignore]` (integration test), or `dropped: type-only`.
- **status** — `pending` until the coverage lands, then `done`.
- **note** — what happened to the TS test file. Task 13 deleted `src/main`, `src/preload` and
  `src/test` with the Electron code, so the test files there are gone and their coverage lives
  only in the Rust tests listed (the files are in git history up to `c9a9a13` on `rust-port`).

Total: **53** files (45 unit + 8 integration). 8 integration → `#[ignore]`; 2 type-only → dropped.

| file | destination | status | note |
| ---- | ----------- | ------ | ---- |
| src/common/agents/delegation.test.ts | Rust: common | done | TS kept (the renderer uses it), so the Jest test stays too |
| src/common/mcp/registry.test.ts | Rust: mcp | done | TS kept (the renderer uses it), so the Jest test stays too |
| src/common/models/__tests__/registry.test.ts | Rust: models | done | TS kept (the renderer uses it), so the Jest test stays too |
| src/main/api/attachments/attachmentContext.test.ts | Rust: attachments | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/attachments/attachmentsManager.test.ts | Rust: attachments | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/attachments/fileNaming.test.ts | Rust: attachments | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/attachments/imageValidation.test.ts | Rust: attachments | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/bedrock/__tests__/agentService.integration.test.ts | Rust #[ignore]: bedrock | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/bedrock/__tests__/flowService.integration.test.ts | Rust #[ignore]: bedrock | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/bedrock/__tests__/guardrailService.integration.test.ts | Rust #[ignore]: bedrock | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/bedrock/__tests__/imageService.integration.test.ts | Rust #[ignore]: bedrock | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/bedrock/__tests__/modelRegionConnectivity.integration.test.ts | Rust #[ignore]: bedrock | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/bedrock/services/subAgent/SubAgentRunner.test.ts | Rust: agents | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/command/outputPatterns.test.ts | Rust: tools | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/docker/composeWriter.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/docker/dockerEngine.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/docker/naming.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/docker/sandbox.integration.test.ts | Rust #[ignore]: docker | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/docker/sandboxActivity.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/main/api/docker/sandboxTerminal.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/main/handlers/agent-handlers.test.ts | Rust: common (agent_files; dialogs/S3 in app) | done | TS test file removed with the Electron code (Task 13) |
| src/main/handlers/chat-attachments-handlers.test.ts | Rust: attachments (handlers module) | done | TS test file removed with the Electron code (Task 13) |
| src/main/handlers/docker-sandbox-handlers.test.ts | Rust: app | done | TS test file removed with the Electron code (Task 13) |
| src/main/handlers/help-handlers.test.ts | Rust: common (help) | done | TS test file removed with the Electron code (Task 13) |
| src/main/lib/userGuide.test.ts | Rust: app | done | was Jest (retained), but it tested `src/main/lib/userGuide.ts`; ported to `app` `commands::help` tests when that file was removed |
| src/preload/lib/gitignore-like-matcher.test.ts | Rust: tools | done | TS test file removed with the Electron code (Task 13) |
| src/preload/tools/handlers/interpreter/CodeInterpreterTool.integration.test.ts | Rust #[ignore]: docker | done | TS test file removed with the Electron code (Task 13) |
| src/preload/tools/handlers/interpreter/CodeInterpreterTool.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/preload/tools/handlers/interpreter/DockerExecutor.integration.test.ts | Rust #[ignore]: docker | done | TS test file removed with the Electron code (Task 13) |
| src/preload/tools/handlers/interpreter/DockerExecutor.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/renderer/src/components/icons/AgentIconView.test.ts | Jest (retained) | done | |
| src/renderer/src/components/icons/iconCollections.test.ts | Jest (retained) | done | |
| src/renderer/src/lib/contextLength/__tests__/index.test.ts | Jest (retained) | done | |
| src/renderer/src/lib/pricing/modelPricing.test.ts | Jest (retained) | done | |
| src/renderer/src/lib/routeMatching.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/components/AgentForm/McpServerSection/mcpMarket.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/components/AgentForm/McpServerSection/mcpSearchQueries.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/components/AgentList/agentOrder.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/components/SandboxPanel/composeDiagram.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/constants/helpAgent.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/constants/pageBackedAgents.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/lib/attachmentContext.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/lib/hostCommandApproval.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/runners/sessionRunners.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/utils/agentMentions.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/ChatPage/utils/sanitizeTitle.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/DiagramGeneratorPage/utils/__tests__/xmlParser.test.ts | Jest (retained) | done | |
| src/renderer/src/pages/SettingPage/settingTabs.test.ts | Jest (retained) | done | |
| src/test/agent-schema-validation.test.ts | Rust: common | done | TS test file removed with the Electron code (Task 13) |
| src/test/directory-agents-validation.test.ts | Rust: common | done | TS test file removed with the Electron code (Task 13) |
| src/test/sandbox/sts.client.test.ts | Rust: docker | done | TS test file removed with the Electron code (Task 13) |
| src/test/src/preload/tools.test.ts | dropped: type-only | done | TS test file removed with the Electron code (Task 13) |
| src/types/tools.test.ts | dropped: type-only | done | TS test file removed with the Electron code (Task 13) |
