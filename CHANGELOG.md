# Update summary

Dated list of changes made in this fork of [aws-samples/bedrock-engineer](https://github.com/aws-samples/bedrock-engineer).
Newest first. The topmost dated section is published as the release notes for each build, so add a
dated section here for anything user-visible.

See the [README](./README.md#whats-different-in-this-fork) for a feature-by-feature
overview of the fork with screenshots.

### 2026-11-10

- The chat scrolls to each new response again. After a long answer had stopped with its first
  line at the top, the next message you sent was no longer followed. The chat now also follows
  thinking text and tool input as they stream, and no longer pulls you back to the start of the
  answer when you scroll down while it is still being written.

### 2026-11-09

- The `@mention` list in the message box now opens above the line you are typing on, tall enough
  to show the whole list (it scrolls past about ten rows), instead of squeezing into the strip near the
  bottom of the window.
- The tool switches on an agent's Tools tab are larger and color-coded, a soft green when the tool
  is on and a soft red when it is off, so they are easy to spot and read at a glance.
- An agent or knowledge base you `@mention` now stays on for the rest of the chat. Before, a mention
  only lasted for the message it was in, so a follow-up such as "now move the note" left the agent
  unable to reach the other agent. Write `disable @` and the name to turn one off again for that
  chat; mentioning it again turns it back on. Two new buttons next to the brain button above the
  message box, a robot for agents and a database for knowledge bases, show how many are on and list
  them with an on / off switch each, so you can also turn them on and off there.
- Security updates: the built-in SQLite (used by local knowledge bases and the usage ledger) is
  updated for a reported SQLite vulnerability, and the MCP client is updated to fix two reported
  issues: an address check that could be bypassed when connecting to servers that sign in with
  OAuth, and unlimited memory use when a streamable HTTP server sends a very large response.
- Every chat now keeps its files in one folder in your project, `chats/<chat-title>/`, named after
  the chat and renamed with it. New work the agent does for a chat is saved there — code, notes,
  diagrams, generated images, Code Interpreter output — and files you attach go in that folder's
  `attachments/`. Before, each chat made up its own folder names at the top of your project folder,
  so a day of chats left a pile of stray folders behind; now one chat is one folder, and deleting
  the chat cleans it up. The agent still edits your project's own files whenever you ask it to.
- Opening a chat from an older build moves the files it left scattered into its folder, and the
  Files tab shows them at their new paths. Files that are not plainly that chat's own are left
  exactly where they are: anything another chat also touched, anything git tracks, and anything
  older than the chat. Nothing is ever overwritten.
- The window no longer freezes for a moment while you work in a long chat history. The chat list's
  check for which chats have attached files used to read and parse every saved conversation on the
  main thread every fifteen seconds, and opening a big chat read it there too.
- Long chats stay responsive while the agent works, and opening an older chat from the sidebar no
  longer locks the window while it loads. Each saved message used to re-read the whole
  conversation, every file the agent changed rewrote the saved chat on the spot, and switching
  chats copied the whole conversation several times. Now the chat loads in the background, a new
  message is added to the copy already in memory, and changed files are saved in batches (the
  Files tab still marks them straight away).
- Added Claude Fable 5.2, Anthropic's newest frontier model for technical work, with a 1M-token
  context window, 128K output and always-on adaptive thinking. It is offered through the Global and
  US routes. Bedrock has not published its price yet, so the cost shown uses Fable 5.1's rates until
  it does.
- Removed the Claude 4.x models (Haiku 4.5, Sonnet 4.6, Opus 4.6, 4.7 and 4.8) and the OpenAI GPT-5.6
  models (Sol, Terra and Luna). If one was selected — as the chat model, the light-processing model,
  the image recognition model, a scheduled background task's model or in your Visible Models list —
  it now uses the closest current model on the same route: Haiku 5.5, Sonnet 5.5 or Opus 5.5 for
  Claude, and GPT-6.1 Sol, GPT-6.1 Terra or GPT-6 Luna for GPT-5.6 Sol, Terra and Luna. Usage you
  already recorded with these models keeps its price on the Accounting page. The chat's
  **Interleave** toggle is gone with them, since only the Claude 4 models offered it; the Claude 5
  models already interleave their thinking with their answers. Strands agents you export now use
  Claude Sonnet 5.5 and no longer set temperature or top_p, which that model rejects.

### 2026-11-06

- Chat history folders can be put in any order: drag a folder by its name above or below another
  one, or use **Move up** and **Move down** in the folder's menu. The order is kept between launches.
- Every passage a local knowledge base returns is now tagged with its source (title and section,
  file path or page URL, knowledge base and its position in the document) plus a ready-made
  citation link. Agents are asked to put that link right after each sentence or bullet that uses the
  passage, so each fact carries its reference where you read it rather than in a list at the end.
  Clicking a cited file opens it with its default app (only files that are in one of your knowledge
  bases), and the knowledge base search box on the agent editor shows the same source details.
- Knowledge bases can be `@mentioned` in the message box, like agents. The `@` list shows every
  agent's knowledge bases (the current agent's first); mentioning one makes the agent search only
  the mentioned knowledge bases for that message, even ones that belong to another agent.

### 2026-11-05

- On an agent's Knowledge Base tab, each knowledge base now has a bolder outline and a little more
  space around it, so with several knowledge bases it is easy to see where one ends and the next
  begins.
- Knowledge bases on an agent's Knowledge Base tab now start out folded. While folded, the four
  statistics boxes (passages, storage, searches and search time) sit to the right of the name and
  details; unfolding moves them back to the top of the expanded view.
- The chat history now has folders. Create one with **New folder** in the Chat History menu, then
  drag chats onto it, or use **Move to folder** in a chat's menu. Drag a chat onto the heading or
  the main list to take it out again. Folders can be folded, renamed and deleted; deleting a folder
  keeps its chats and moves them back to the main list.

### 2026-11-04

- Upgraded the tools used to build and check the app: the code checker (ESLint 8, which is no
  longer maintained, to ESLint 9), the test runner (Jest 29 to 30) and the build tool (Vite 6 to 8,
  with its React and SVG plugins). The app works the same; installers now build their interface in
  seconds instead of about half a minute. All checks and the 444 automated tests pass on the new
  versions.
- Three open security alerts on the app's dependencies can't be fixed by upgrading yet. Two are in
  development tools only and aren't included in the installed app: minimatch (no fixed release for
  the versions the code checker and test runner use) and deepmerge (no fixed release at all). The third, glib, is part of the Linux
  window toolkit, and Tauri, the framework the app is built on, still requires the affected version
  in its latest release. These will be revisited as fixed versions come out.
- Fixed English counts that were always singular, such as "3 tool call" under a sub-agent's answer
  and "2 file" in the attachments menu. The translation library is also upgraded to its current
  version.
- Upgraded the libraries the app's interface runs on to their current major versions: React 18 to
  19, React Router 7 to 8, the Terminal tab's terminal (xterm 5 to 6), the Markdown renderer, the
  animation library, the loading animations and the emoji picker. Nothing is meant to look or work
  differently. The command palette's library hasn't been updated for React 19, so it is allowed to
  run on it; it uses nothing that React 19 removed.
- Diagrams in chats are drawn with Mermaid 12, which lays them out differently: lines run at right
  angles instead of curving, and flowcharts and state diagrams come out more evenly spaced, though
  taller. Colors are unchanged. Mermaid 12 needs a current web engine: Safari 17.4 or later on macOS
  (available as a Safari update back to macOS 12) and WebKitGTK 2.44 or later on Linux (Ubuntu
  24.04 has it). On older versions diagrams don't render; the PDF viewer already had the same
  requirement.
- Moved the interface's styling to Tailwind CSS 4 and its component library to Flowbite React
  0.12. Every page, dialog and appearance was compared against the previous build screenshot by
  screenshot, and differences were fixed until they matched, so nothing should look different.
  One small fix came out of it: the IAM policies dialog in Settings now covers the whole window
  instead of leaving a thin strip uncovered at the top.
- The app now includes the ELK graph layout engine, which Mermaid 12 uses for diagrams. It is
  open source under the Eclipse Public License 2.0 and ships unmodified; its license text is with
  the app's other third-party notices, and the app's NOTICE file says where its source is.
- The tour (Home → Open Tour) now walks through the whole sidebar instead of stopping after one
  step: it starts with Settings, where you enter your AWS credentials, then introduces Chat, My
  Agents, Memory, the Agent Directory, Background Agents, the three generators and Accounting, and
  ends on Help with the command palette shortcut. Items you've hidden from the sidebar are skipped.
- Updated the spreadsheet reader used by local knowledge bases and the document tools (calamine
  0.37 to 0.38). The old version could crash or run out of memory on a malformed legacy Excel
  (.xls) file (RUSTSEC-2026-0436).

### 2026-11-02

- Fixed web pages saved by the agent sometimes ending up as empty files: the save could finish
  before the page's contents were actually written.
- An agent can only use the tools enabled for it. Before, a reply could call any of the app's
  tools by name, even ones the agent's settings turned off, for example a "read-only" agent writing
  files or running commands. Such calls are now refused, in chats and in background and sub-agent
  runs.
- MCP server tools are now checked when a server connects. A tool is left out if its name is one
  of the app's own tools (a server could otherwise put a look-alike next to the real one to steer
  the agent), if its name isn't valid for Amazon Bedrock, or if its description of inputs is
  unreasonably large; very long tool descriptions are shortened. When two of an agent's servers
  offer a tool with the same name, only the first server's is used.
- On Windows, MCP servers that run as a command are now started directly instead of through the
  command prompt, which could reinterpret characters such as `&`, `|` and `%` in the server's
  command and arguments.
- The first time an agent tries to take a screenshot or use the camera, the app asks you whether
  that agent may, and remembers the answer for that agent. Background agents and sub-agents can no
  longer capture the screen or the camera at all. Until now, once the app had the system permission,
  any agent with the tool could look at your screen, including a scheduled agent with nobody
  watching.
- The agent's web page fetch now gives up after 60 seconds and reads at most 20 MB, so a hostile
  site can no longer hang or crash the app. It also never contacts cloud metadata addresses
  (169.254.169.254 and other link-local addresses), which on a cloud machine hand out its temporary
  AWS credentials; this is checked on every redirect and on the addresses a site name resolves to.
- New setting **Settings → Workspace → Local network access for web tools**. Turn it on to stop the
  agent's web tools from reaching this computer and your local networks (dev servers, admin pages,
  routers, intranet sites), which text the agent reads could otherwise steer it into. It is off by
  default, so fetching a local dev server or an intranet page keeps working.
- The knowledge base website crawler applies the same checks: it never contacts cloud metadata
  addresses, and with **Block web tools from reaching this computer and local networks** turned on it
  stays off this computer and your local networks, on redirects too. A website source whose address
  redirects to a different site is now refused with a message naming where it leads, instead of
  indexing whatever that other site serves.
- MCP servers reached by URL no longer follow a redirect to a different site, so the API keys or
  tokens configured in a server's headers are never sent to another host. Connecting to an older SSE
  server now times out instead of waiting forever for a server that doesn't answer, and a single
  oversized message from such a server ends the connection instead of filling memory.
- New section **Settings → Sandbox → Sandbox restrictions** with two optional switches for new
  sandbox containers, both off by default. Each one lists what it turns off.
  - **Lock down sandbox containers** keeps only the Linux capabilities package managers need,
    blocks privilege gain through `sudo` and other setuid programs, and limits containers to 1,024
    processes. Installing packages still works; `sudo`, `ping` and similar tools don't.
  - **Start sandboxes with no network access** cuts sandboxes off from the internet, this
    computer's services, your local network and a cloud machine's credentials. That also stops
    package installs, downloads and published ports.
- Docker sandbox containers no longer restart on their own, whatever the agent's compose file
  asks for, so a sandbox can't come back by itself after you quit the app.
- Images in replies that come from other websites now wait for a click: they show a placeholder
  naming the site and a **Load** button. A reply steered by a web page, file or tool result could
  otherwise hide parts of your conversation in an image address and send them to that site the
  moment the reply appeared. To load such images automatically again, turn on **Settings → Chat →
  Remote images in chat**. Images from your project and attachments, and images in the help guide,
  changelog and your own files, still load straight away.
- Your AWS secret access key, session token and proxy password are no longer handed to the app's
  windows. They stay in the app's background process, which uses them as before. In **Settings →
  AWS** a saved secret now shows as **Saved (type to replace)**: type a new one to replace it, or
  click **Remove saved value**; leaving the field empty keeps it. Before, every window held these
  values in memory, so any fault in the app's pages could have exposed them.
- Links in search results, knowledge base results and other app content that would open something
  other than a web page or an email (for example a `file:` or `smb:` address, or a link handled by
  another app) now show the address and ask before opening. Web and email links open as before.
- Word documents previewed in the Files panel no longer keep links that would run script code
  (`javascript:` and similar) or other unusual link types; their text stays but isn't clickable.
  Web, email and in-document links work as before.
- The Tavily search API key is now kept the same way: it stays in the app's background process,
  and the settings show **Saved (type to replace)** with **Remove saved value** once it is saved.
- On macOS and Linux, the app's settings file (which can hold your AWS keys), chat history and other
  files it keeps for itself are now readable only by your user account, as is the app's data folder.
  Before, the settings file could be readable by other accounts on the same computer.
- Word, PowerPoint and Excel files that would expand to an enormous size when opened (a trick that
  makes a small file fill all memory) are now refused when attached or indexed into a knowledge base,
  instead of crashing the app. Ordinary documents are far below the limits (256 MB per part, 1 GB per
  spreadsheet).
- Exporting an agent as a Strands Agents project no longer copies the values of its MCP servers'
  environment variables or headers, which often hold API keys or tokens. The generated `agent.py`
  reads each variable from the environment it runs in, and `config.yaml` lists header names without
  their values. Agent names and descriptions are also written so they can't break the generated
  Python code.
- Scheduled background agent runs now stop after 100 tool calls instead of 500, since nobody is
  watching them. Change the limit (1 to 1,000) in the new **Settings → Chat → Background agents**
  section.
- When a proxy is turned on, the app no longer connects directly anywhere. Calls to AWS, the web
  tools, MCP servers reached by URL, knowledge base website sources, the on-device model download and
  update checks all go through the proxy. If the proxy settings can't be used (no host, a protocol
  other than HTTP or HTTPS, a malformed host), these connections fail with a message pointing to
  **Settings → AWS → Proxy Settings**. Before, several of them quietly connected without the proxy,
  update checks always did, and MCP servers kept connecting the old way after the proxy was changed
  until the app restarted. Proxy hosts written as IPv6 addresses now work.
- Pages and images shown inside the app are now blocked, with a notice, whenever a proxy is on
  that they aren't actually using. That covers images in chats and search results, the Website
  Generator preview, draw.io diagrams, HTML previews and avatars. It happens after you change the
  proxy until you restart the app, and always on macOS 13 and older, which can't send this content
  through a proxy. Before, this content connected without the proxy in those cases. Chat PDF exports
  leave out remote images in the same situation.
- Updated the certificate-checking libraries behind the app's secure connections, so certificates
  from authorities that browsers no longer trust are rejected, and two cryptography libraries with
  timing fixes.
- Removed an old copy of the ZIP library that the in-app updater pulled in on Windows, which had
  open security advisories (one critical). The updater never used it for this app's installers, so
  updates work as before.

### 2026-11-01

- Ports the Docker sandbox publishes are now reachable only from this computer. They used to be
  open on every network interface, so anyone on the same Wi-Fi or office network could reach a
  dev server, database or file listing the agent started in the sandbox, which has your project
  folder mounted.
- MCP servers started from a command no longer pick up a `node_modules/.bin` program from whatever
  folder the app was launched in, which let a program planted there stand in for `npx` or `uvx`.
  A server command given as a relative path such as `./server` is now refused with a message asking
  for an absolute path or a command name on your PATH, and only executable files count when the app
  looks a command name up in the usual install folders.
- The task history window and pop-out file viewers can no longer run tools, reach the Docker
  sandbox or its terminal, or change settings; only the main window can. Neither window offered
  any of these, so nothing you use there changes, but a problem in one of those windows can no
  longer be used to run commands or rewrite your agents and credentials. Sandbox output and the
  sandbox terminal are now sent to the main window only.
- On Windows, commands the agent runs on your computer no longer search `C:\Users\Public\chocolatey\bin`
  for programs. Every account on the machine can write to that folder, so another user could have
  left a program there that the agent would run as you. Chocolatey's real install folder,
  `C:\ProgramData\chocolatey\bin`, is still searched.
- Docker sandboxes now keep their compose file, settings and data folders in the app's data folder
  instead of a `docker-sandboxes` folder inside your project. The container can write anywhere in
  your project, so it could rewrite its own compose file there, for example to run privileged or to
  mount your whole disk, and the next start would have used it. The app also re-checks a sandbox's
  mounts every time it starts one. Existing sandboxes are not carried over: a chat that had one gets
  a fresh sandbox the next time the agent needs it, so anything installed in it has to be installed
  again. The old `docker-sandboxes` folder in your project, with its data, is left in place for you
  to copy from or delete. Folder paths in a sandbox's compose file now resolve against your project
  folder.
- Commands the agent runs on your computer must now be a single program with its arguments.
  Anything chained with `;`, `&&` or `||`, piped with `|`, redirected with `>` or `<`, sent to the
  background with `&`, or using `$(...)` or backticks is refused, whatever your allowed commands
  say. Before, an allowed pattern such as `ls *` let through `ls && curl … | sh`, because only the
  first word was checked and the whole line went to the shell. Quoted text such as
  `git commit -m "a; b"` still works. Patterns are also stricter: a `*` at the end stands for any
  further arguments, so `npm run *` still allows `npm run dev` but no longer allows `npm install`,
  and a `*` in the middle stands for exactly one argument.
- The app now asks before it starts an MCP server that runs as a command on your computer. The
  dialog shows the program, its arguments and the names of its environment variables, and warns you
  when the server will receive your AWS access keys. Until now, an agent shared in a project you
  cloned, imported from a file or loaded from your organization could start any program as soon as
  you selected it. The app remembers each server you allow, and asks again if its program, arguments
  or environment change. Servers you already use ask once after this update. If you choose
  **Don't allow**, its tools stay unavailable until you restart the app. Servers reached by URL
  don't ask.
- The first time an agent runs a command on your computer through one of its Allowed Commands
  patterns, the app asks you to confirm that pattern for that agent. After you allow it, matching
  commands run without asking, as before. Until now the Allowed Commands list was the only check,
  and an agent shared in a project or imported from a file brings its own list. Background agents
  and sub-agents never ask: they can only use patterns you have already allowed for that agent, so
  run a scheduled agent in a chat once to approve the commands it needs. Patterns you already use
  ask once after this update.
- A command the agent runs on your computer is now stopped when it times out, unless it is a server
  that has started or a program waiting for input. Before, it kept running in the background with
  nothing tracking it. Each running command also keeps only its most recent output (1 MB) in memory.
- Updated libraries with published security fixes: the ZIP reader used for PowerPoint, Word and
  Excel files (one advisory was critical), the TLS library behind every network connection, the
  async runtime, and the library that validates agent files and MCP server settings.
- The agent's file tools (reading, writing, editing, moving, copying, listing and saving web pages)
  now work only inside your project folder and any folders you add under the new **Settings →
  Workspace → Agent folders**. Before, they could read and write anywhere on your computer, so a
  prompt hidden in a web page or file could have the agent read your SSH keys or change your shell
  startup files. The app's own data folder and credential folders such as `~/.ssh` and `~/.aws` are
  refused even inside an allowed folder. Relative paths now resolve against the project folder.
  Reading a file over 50 MB is refused, and folder listings stop after 10,000 entries. A web page
  saved with a file name containing a folder is refused, and saving never replaces an existing file.
- Images shown in chat from files on your computer now load only from your project folder, your
  agent folders and the folder screenshots and camera photos are saved to, and only if the file is
  an image. Before, a reply could name any file on your computer as an image and the app would read
  it. Screenshots and camera photos are always saved to the temporary folder.
- The Code Interpreter only mounts input files from your project folder and agent folders, and now
  really mounts them read-only, as it always said. Before, the agent could mount any file on your
  computer, such as your AWS credentials, and the code could change it. The code also runs as an
  unprivileged user with no extra Linux capabilities and a cap on the number of processes, even when
  the app falls back to the stock Python image.
- **Settings → Workspace → Allowed commands** shows how many command patterns your agents allow,
  and **Manage…** opens a dialog listing every agent's Allowed Commands in one place, where you can
  add and remove patterns. Agents shared through a project folder are shown read-only.

### 2026-10-30

- Local knowledge bases have an **Add folder** button next to **Add files**. The folder becomes one
  source that follows its contents: files added to the folder later are indexed, edited files are
  re-read, and files deleted from it are removed from the knowledge base. Hidden files, dependency and
  build folders such as `node_modules`, and files over 100 MB are left out. A folder with 500 or more
  files asks before it is added.
- File and folder sources now stay up to date on their own. The app checks them about 30 seconds after
  it starts and then every hour, and re-syncs only those whose files changed, so an unchanged
  knowledge base costs nothing to check. Turn it off or change the interval under **Settings →
  Knowledge Base → Automatic re-sync**. Re-syncing also skips files whose modification date hasn't
  changed, so it no longer re-reads every file. Websites are still re-synced only when you ask.
- Fixed the window becoming unresponsive while a knowledge base was being indexed. Every embedded
  passage is recorded on the Accounting page, and each new record made the sidebar's budget badge
  recalculate the month's total on the thread that draws the window, many times a second. Those
  updates now come at most once a second, run in the background, and the month's total is
  much quicker to work out. Indexing also reuses one connection to Bedrock instead of opening a
  new one for every batch of passages.
- Knowledge base indexing with Amazon Titan embeddings now records its Bedrock calls on the Accounting
  page one entry per batch of passages instead of one per passage, so a large folder no longer adds
  tens of thousands of entries. Call counts and costs are unchanged; a batched entry shows how many
  calls it stands for (for example **×32**), and the CSV export has a new `calls` column.

### 2026-10-27

- **OpenAI GPT-6.1 Terra** is now in the model dropdown, as **(US)** and **(Global)**, in US East
  (N. Virginia, Ohio) and US West (Oregon). It follows GPT-5.6 Terra and OpenAI reports it doing
  about as well as GPT-6 Astra on agent tasks while finishing them faster, at $1.10 per million
  tokens in and $5.50 per million out (Global is about 9% cheaper), half the price of GPT-6.1 Sol.
  It takes text and images, has a 1.05M-token context window, can return up to 128,000 tokens in
  one answer, and thinks by effort level, so **Deeper** asks it to think as hard as it can. Prompt
  content is cached automatically and there are no cache settings to set. Prompts longer than
  272,000 tokens bill at a higher rate, and the Accounting page prices them that way.

### 2026-10-25

- **Costs by agent, per chat and per background task run.** The Accounting page has a table of
  cost by agent (click one to see only its usage). The chat history list now shows what each chat
  has cost, and a background task's history shows the cost of each run.
- **Prompt caching savings.** The Accounting page shows how much prompt caching saved this month.
- **More accurate cost estimates.** Calls through your own application inference profiles are now
  priced as the model the profile uses instead of being left unpriced. For models whose price
  differs between the global and geography profiles, or rises for very long prompts, the
  Accounting page now uses the rate the request actually billed at.
- **Help works offline and opens the user guide first.** The Help button now opens the user guide
  inside the app, with a table of contents, working section links and a search box. It needs no AWS
  access or internet connection, so it helps even before anything is set up. **Ask a question**
  opens the Help chat, which now answers from the guide sections that match each question rather
  than sending the whole guide every time, so Help answers cost much less. The guide's search index
  ships with the app; if you have downloaded the on-device model for local knowledge bases, it
  matches by meaning as well as by keyword.
- **The AWS Solutions Architect agent moved to the Agent Directory.** It is no longer a built-in
  agent, so it can be edited: add it from the Agent Directory to change its prompt, tools or MCP
  servers. If you already had it, your copy stays in My Agents and is now editable like your own
  agents.

### 2026-10-24

- **New built-in AWS Solutions Architect agent.** It answers AWS questions from the official AWS
  documentation and links each point to its source. It comes with four AWS MCP servers already set
  up: AWS Documentation, the hosted AWS Knowledge server (documentation and Regional availability),
  Infrastructure as Code (validate CloudFormation templates, check compliance, troubleshoot failed
  deployments, look up CDK guidance, and never deploy), and AWS Pricing for cost estimates. Three
  of the servers run with `uvx`, so install uv first. Built-in agents with MCP servers now also get
  those servers' tools in chat; before, their tools were fetched and then dropped.
- **MCP servers can use your AWS settings.** In a server's environment variables, `{{awsProfile}}`
  and `{{awsRegion}}` are filled in from Settings → AWS each time the server starts. If you sign in
  with an access key instead of a profile, the key pair is passed in place of the profile. Only
  servers that use these placeholders receive your credentials, and the values take priority over
  AWS variables already in the app's environment.
- **New Accounting page.** A page in the left sidebar shows the estimated Amazon Bedrock cost of
  the app's model calls, month by month. Pick a month to see its total and tables by model, by
  feature (chat, the generators, background agents, sub-agents, chat titles, knowledge base
  indexing and search, and so on) and by day. Recording starts with this version and nothing is
  ever deleted, so you can look back at earlier months. Costs are estimates from list prices at
  the time of each call; calls to models without a known price are counted but not added to the
  totals. Embedding calls for knowledge bases that use Titan or Cohere models are included.
- **Accounting charts, filters, CSV export and past chats.** The Accounting page now charts cost
  per day, stacked by model, and cost per month; click a month's bar to open it. Clicking a model
  or feature in its table narrows the whole page to it, and the list of individual calls at the
  bottom shows each one's time, model, tokens and cost. **Export CSV** saves the calls of the month
  or filter. On first launch the page also imports the usage your chats already saved, so earlier
  months aren't empty (they show chat cost only) and older models that are no longer offered are
  still priced.
- **Monthly budget alerts.** Set a monthly budget on the Accounting page and you get a
  notification the first time the month's estimated Bedrock cost reaches 80% and 100% of it (you
  can choose other levels). While you're over budget the `$` icon in the sidebar shows a red dot,
  and the page shows how much of the budget is used and the projected month-end cost.
- **More Bedrock costs counted.** The Accounting page now also records image generation (per
  image), Nova Reel videos (per second), Bedrock Agents, guardrail checks, Amazon Translate and
  image recognition with Claude models, with their images, seconds or characters shown next to
  the cost. Knowledge Base retrieval and Flows are counted too, though AWS reports no usage for
  them to price.

### 2026-10-23

- **The Files panel's file list can be resized.** Drag the divider between the file list and the
  preview to widen it and read long file names, or narrow it to give the preview more room. The
  arrow keys move the divider when it has focus, a double-click puts it back to the default width,
  and the width you choose is remembered.
- **Select all in a file preview.** After clicking in a previewed file in the Files panel (or its
  pop-out window), ⌘A (Ctrl+A on Windows and Linux) selects just that file's content, ready to
  copy. Before, it selected everything in the window.
- **Line numbers in source views are back.** File previews and the Source views of HTML, Mermaid
  and draw.io blocks showed "false" on every line instead of its number. The numbers also no
  longer get highlighted when you select the code.
- **Choose the Docker sandbox's operating system.** A new Operating system menu offers Debian 13,
  Ubuntu 26.04, Amazon Linux 2023, Amazon Linux 2027, AlmaLinux 9, Rocky Linux 9 or Red Hat UBI 9
  (real RHEL, limited to Red Hat's freely available packages). The agent can also ask for a
  different one when it creates a sandbox. **Debian 13 is now the default**, replacing Ubuntu: in
  testing it was the quickest to download and to install common tools on. Chats that already have a
  sandbox keep their current OS until the sandbox is removed.
- **Sandbox settings have their own tab.** Settings now has a Sandbox tab with Docker's status, the
  default container image and the container limits (memory, CPU and command timeout). The Docker
  Sandbox tool's panel shows a summary of these with a button that opens the tab.
- **Local knowledge bases for agents.** A new Knowledge Base tab in the agent editor (after MCP
  Servers) lets you give an agent one or more knowledge bases built from documents (text,
  Markdown, code, PDF, Word, HTML, Excel and PowerPoint files) and websites. A website is crawled
  by the app itself, staying on that site, up to a link depth and page limit you choose, and
  honoring robots.txt. Everything is chunked, embedded and stored on your computer. An agent with a
  knowledge base gets a built-in MCP server whose search tool finds the most relevant passages
  (combining meaning and keyword matches) and cites their file or page; the MCP Servers tab shows
  it as a built-in server with a connection test, and the Tools tab lists its tools. Each source can be
  re-synced (unchanged documents are skipped) or removed, a website's link depth and page limit
  are shown with it and can be changed later, and the tab has a search box for trying
  a knowledge base out. Knowledge bases are copied when you duplicate an agent and deleted with it.
  They are not included in shared or exported agents.
- **Knowledge Base settings.** A new Settings tab picks the embedding model for new knowledge bases:
  Amazon Titan Text Embeddings V2 or Cohere Embed through Bedrock, or multilingual-e5-small, which
  runs on your computer and works offline after a one-time download of about 465 MB. It also holds
  chunk size, the number of search results, the balance between meaning and keyword matches, the
  crawler's defaults, and a list of every knowledge base on disk with its size. From that list you
  can delete knowledge bases, including any left behind by deleted agents. A knowledge base keeps
  the model it was built with, and the agent editor offers a rebuild when Settings name a
  different one.
- **Knowledge base dashboards and upkeep.** Each knowledge base in the agent editor shows tiles
  for its passages, storage, how often the agent searched it over the last 30 days (with a
  per-day trend) and its typical search time, plus a health badge. The Settings tab opens with
  an overview of every knowledge base: totals, then a row per knowledge base with the same
  figures. Health checks flag sources that failed to sync, websites not synced for 30 days, a
  missing local model, a model that differs from Settings, and free space in the file.
  **Optimize** (per knowledge base, or all at once) compacts the file, tidies the keyword index
  and checks the file for errors; it also runs by itself after an update that leaves a lot of
  free space.
- **Help knows the whole user guide again.** The guide had grown past the size limit for an
  attached file, so the Help chat only saw its first part. Help now gets the full guide, which was
  also brought up to date: every Settings tab, the agent editor's four tabs, the chat tabs, the
  Files and Terminal tabs, memory, and new chapters on knowledge bases and memory.
- **MCP tools in the agent editor say which server they come from.** The Tools tab showed the
  tool's own name after "From:" instead of the server's.
- Fixed a typo in the chat history menu ("Geneate title").
- **Page shortcuts work with Ctrl on Windows and Linux.** Ctrl+K opens the page search, Ctrl+,
  opens Settings and Ctrl+1 to Ctrl+9 open the sidebar pages; before, they answered only to ⌘,
  which those keyboards don't have. Hints and sidebar tooltips show the right key for your system,
  and pages past the ninth no longer show a shortcut that doesn't exist.
- **The Voice Chat prompt button is gone** from the agent editor's system prompt, along with the
  remaining mentions of voice chat. Voice chat itself was removed earlier.

### 2026-10-22

- **Rules and memories now mean different things.** Rules are instructions the assistant always
  follows unless you tell it otherwise in that chat. Memories are facts it keeps in mind and uses
  when they are relevant, without treating them as instructions. Before, both were given to the
  model with the same "follow these" wording, so the choice of kind made little difference. The
  Memory page now explains the two kinds, with an example of each.
- **Updates are checked every hour** instead of every few hours, so a new release reaches you
  sooner.
- **Updates have their own Settings tab.** The update controls moved from General to a new
  **Updates** tab, which also lists what changed in the version you are running, with earlier
  changes a click away. When a new version has been downloaded, its release notes appear there too.
- **Settings… in the macOS app menu.** The Bedrock Engineer menu now has a **Settings…** item
  (⌘,) that brings the app window forward and opens its Settings page.

### 2026-10-21

- **The Files source view is readable in every appearance.** Syntax-highlighted source (code
  files, and the **Source** view of Markdown and HTML) now sits on a dark background in all themes.
  Before, the pale highlight colours were drawn straight onto light backgrounds and were hard or
  impossible to read in the Light, Newspaper and Dim appearances.
- **Chat tabs replace the side panel.** The chat now has tabs across the top: **Chat**, **Files**,
  **Sandbox**, **Todo**, **Cost** and **System prompt**. Each one uses the full width of the chat
  instead of sliding in from the right. The todo list, the token and cost charts and the system
  prompt used to be pop-ups and are now tabs too. A tab only appears when it has something to show,
  the message box stays at the bottom on every tab, and sending a message switches back to Chat. The
  Todo tab shows how many items are done and the Cost tab shows the running cost. New files and a new
  sandbox are marked on their tabs until you look, and each chat remembers the tab you left it on.
- **The chat history collapses to a rail.** Instead of sliding away completely, the history on the
  left collapses to a narrow strip with a New chat button and your recent chats as initials. Click
  one to switch to it, or expand the full list again.
- **Memory items have a toggle button.** The Use Memory Items checkbox is now a brain button (the
  Memory page's icon) next to the attach and export buttons. It is highlighted, with the number of
  items, while memory is on, and its tooltip says whether memory is on for the chat. It is still on
  for every new chat.
- The buttons above the message box (stop, files, export, new chat, memory, Plan and Act) now use
  the same plain tooltip as the attachments and sandbox buttons instead of a dark pop-up bubble.
- **Terminal tab.** A terminal button next to the memory button opens a shell on your own computer
  (your usual shell, started in the project folder) in a new **Terminal** tab. Each chat can have
  several, shown as sub-tabs with **+** to add one and **×** to close one. Shells keep running while
  you switch tabs or chats and end when you close them, delete the chat or quit the app. The agent
  has no access to them.
- **Prompt icons in terminals.** The app now ships the Nerd Fonts symbol font, so the icons that
  shell prompts like Starship and Powerlevel10k draw (git branch, language and cloud icons) show up
  in the Terminal and Sandbox terminals instead of empty boxes. Letters still use your code font.
- **Mermaid diagrams render completely.** Diagrams with line breaks in their labels (`<br/>`) lost
  every node after the first such label, leaving only the arrows and the subgraph boxes. The same
  diagrams also went missing from PNG downloads and from Markdown, Word and PDF exports. All of
  these now come out whole.
- The **Source** view of Mermaid, HTML and draw.io blocks in chat now sits on a dark background in
  every appearance, like the Files source view, so the highlighted code is readable.
- **Wide Mermaid flowcharts fit the chat.** A flowchart that comes out much wider than tall (a
  long left-to-right chain, say) is now drawn top-to-bottom when that fits the chat column
  better, so it can be read without scrolling or squinting. An **As written** button on the diagram
  switches back to the direction in the source, and PNG downloads and exports use the same layout.
  Flowcharts are also drawn with less space between boxes. Other diagram types are unchanged.
- **Copying source leaves out the line numbers.** Selecting and copying from the Source view of
  Mermaid, HTML and draw.io blocks, or from a file in the Files tab, now copies only the code. The
  Mermaid Source view also has a **Copy** button.
- **Automatic updates.** The app now checks for new versions at launch and every few hours,
  downloads them in the background, and offers to restart into the new version (or installs it the
  next time you quit). Every download is checked against the app's update signing key first. On
  macOS, updates installed this way don't need the Terminal signing steps from the first install.
  **Settings → General → Updates** shows your version, turns automatic checks off, and checks on
  demand. This starts working with the first release published after this one.

### 2026-10-20

- **Memory.** Tell the assistant to "add a rule" or "remember" something and it saves the item to a
  memory list that every chat can use. Ask it to "forget" something and it removes the item. When an
  item is saved, changed or removed, the reply shows a small note with **View** and **Undo**. The new
  **Memory** page in the sidebar lists every item. There you can add items yourself, edit them, mark
  each one as a rule or a fact, switch single items off, or delete them. A **Use Memory Items**
  checkbox above the message box adds all switched-on items to the chat's context. It is checked for
  every new chat, and unchecking it only affects that chat.
- **Files panel.** A new panel on the right of the chat shows your project folder, so you no longer
  have to find the agent's output in Finder or Explorer. Click a file to view it in the panel. Text,
  code, Markdown, HTML, images, PDF and Word documents all preview there, and text files can be
  edited and saved. Each file has buttons to open it in its default app or show it in Finder or
  Explorer. **This chat** narrows the list to the files the agent created or changed in the current
  chat, which are also marked with a fuchsia dot. The list updates as files change, and file paths in
  the agent's replies can be clicked to open the file. The panel can be resized by dragging its
  edge, and the Docker sandbox now appears as a second tab in it.
- Files in the Files panel can be popped out into a window of their own with the new **Pop out**
  button, for reading or editing next to the chat. Each file gets one window, and popping out the
  same file again brings its window to the front. The window updates when the file changes on disk,
  says so if the file is deleted, and asks before closing if there are unsaved edits.
- The strip at the right edge of the chat that opens the Files and Sandbox panel is now easy to
  spot: while the panel is closed it is drawn in the accent colour, pulses briefly when it first
  appears in a chat, when the chat gets a sandbox and when the agent changes files, and shows a dot
  until the panel is opened. The pulse is skipped when your system is set to reduce motion.
- In the Files panel's project tree, the folders that hold the files the agent created or changed
  in the chat now open on their own, so those files are visible without clicking through. Folders
  you collapse stay collapsed.
- The conversation now re-flows to fit when a side panel is opened or resized, instead of growing
  a sideways scroll bar. Long words, links and paths wrap at the edge. Wide tables, code blocks and
  equations scroll within their own box.
- Routine library updates to the latest point releases: the Tauri app framework and its plugins,
  the AWS SDK used for Bedrock, the MCP client, and the code editor and router in the interface.
  Nothing should look or behave differently.

### 2026-10-16

- Security updates for bundled libraries: KaTeX, the math renderer used in chat and in Mermaid
  diagrams, moves to 0.18 to close a prototype-pollution bypass of its trust settings. Libraries
  used only to build the app (source maps, CSS selector parsing, test tooling) were also updated.
  Math should look the same as before.
- Fixed "Failed to generate title" (and similar failures in other quick background requests) when
  the lightweight model is a recent Claude model: Opus 4.7 and 4.8, Sonnet 5, Opus 5, and Claude
  Sonnet, Opus and Haiku 5.5. Bedrock no longer accepts the temperature and Top P settings for those
  models, so the app now leaves both out and the model's own defaults apply. Your temperature and
  Top P settings still apply to older models.

### 2026-10-12

- **OpenAI GPT-6.1 Sol** is now also available as **(Global)**, next to **(US)**. Bedrock has added a
  global option for it, which is 10% cheaper than US: $2.00 per million tokens in and $10 per
  million out, against $2.20 and $11. The cost the app shows is based on the US rate for both, so
  for **(Global)** it runs about 10% high.
- **Claude Haiku 5.5** is now in the model dropdown, as **(Global)**, **(US)**, **(EU)** and
  **(JP)**. It is the fastest model in the Claude 5.5 family and the first Haiku that thinks
  adaptively: it decides for itself how much to think. It does better than Haiku 4.5 on coding and
  agent work, takes text and images, has a 1M-token context window, and can return up to 128,000
  tokens in one answer. At $0.10 per million tokens in and $0.50 per million out, it is a tenth of
  the price of Haiku 4.5. Prompts over 100,000 tokens bill at five times those rates, and the US,
  EU and JP options cost 10% more than Global, so in both cases the cost the app shows is a floor.
  It also shows up in the light-model list in Settings.
- **Retired and older models have been removed.** Claude Opus 4, Opus 4.1, Sonnet 4 and Sonnet 4.5
  and Amazon Nova Premier are gone from the model list. Bedrock has retired them or is about to, and
  the newer models that replace them are cheaper. Image generation drops Amazon Nova Canvas, both
  Titan Image Generator models, Stability SD3 Large, and the v1.1 versions of Stable Image Core and
  Ultra, which Bedrock no longer offers. That leaves SD3.5 Large (now the default), Stable Image Core
  and Stable Image Ultra, all in US West (Oregon) only, so image generation is now available only
  in that region. The Nova Canvas agent is also removed from the Agent Directory.
- If you had a removed model selected, the app switches you to its replacement the next time it
  starts: Opus 4 and 4.1 become Opus 5.5, Sonnet 4 and 4.5 become Sonnet 5.5, Nova Premier becomes
  Nova 2 Lite, Claude 3 Haiku becomes Haiku 5.5, and removed image models become SD3.5 Large. This
  covers the chat model, the light model, the image tools, your visible-models list and scheduled
  background agents. You keep the same US, EU, JP or Global option where the new model offers it.
  New background sessions with no model chosen now use Haiku 5.5 instead of the retired Claude 3
  Haiku.

### 2026-10-08

- Fixed follow-up messages failing with "reasoningContent needs reasoningText or redactedContent"
  on newer Claude models (Opus 4.8, Opus 5, Opus 5.5, Sonnet 5). These models return their
  thinking without the text, and the app was saving that thinking as an empty block that the next
  request rejected. Existing chats that hit the error work again without any action.
- The macOS install instructions now cover a step that was missing: macOS 15 and later won't let
  Terminal change anything in your Applications folder until you enable it under System Settings →
  Privacy & Security → App Management. Without that, the required signing command fails with
  "Operation not permitted" and the app can hang on launch with no window and no error. The
  instructions also now clear the download flag your browser leaves on the app, which is what causes
  that hang.

### 2026-10-01

- The app is now called **Bedrock Engineer** everywhere: the window title, the app in your
  Applications folder or Start menu, and the installer file names
  (`bedrock-engineer-rust-<version>-<arch>`). Your settings and chat history come with you: the first
  time the renamed app starts, it moves its data folder to
  `~/Library/Application Support/Bedrock Engineer/` (or the equivalent on Windows and Linux). The
  rename installs as a new app, so delete the app under its old name afterwards, and on macOS run
  the ad-hoc `codesign` step from the README again for the new app path. macOS will also ask once
  more for Screen Recording and Camera permission.
- The GitHub link in the app menu, the download badges and the links in the user guide now point at
  this fork's new home, [ibehren1/bedrock-engineer-rust](https://github.com/ibehren1/bedrock-engineer-rust).
- The app menu is back on macOS, Windows and Linux, with **View** (Reload, Force Reload, Toggle
  Developer Tools, Zoom In, Zoom Out, Reset Zoom, Full Screen) and **Help** (About Bedrock
  Engineer, GitHub Repository). `Cmd`/`Ctrl` + `+`, `-`, `0` zoom again and `Cmd`/`Ctrl` + `R`
  reloads. The About box shows the app, Tauri and web view versions, and Developer Tools work in
  installed builds as they did before. Right-clicking in the main window offers just Copy and Paste
  again. `Cmd` + `W` on macOS still closes the window and quits the app.
- MCP servers and commands now see your login shell's `PATH` on macOS and Linux, even when the app
  is started from the Dock, Finder or a desktop launcher, so servers started with `npx` or `uvx`
  find a `node` or `python` installed through nvm, volta, asdf or `~/.local/bin`.
- Pages shown inside the app (images in chats, the Website Generator preview, the Diagram Generator)
  now go through your proxy too, not only calls to AWS and the web tools. A proxy change reaches
  them after a restart. This needs macOS 14 or later on a Mac; on Windows a proxy that asks for a
  username and password does not work for this content.
- The main window opens at its old, larger size (fitted to your screen) and appears only once it has
  loaded, so there's no blank window flash at startup. The task history window does the same.
- If part of the app crashes, the error is now written to the log file, which makes a crash
  report much more useful.
- The license shown when you open the macOS disk image (and on the Windows installer's license
  page) now reads as normal paragraphs, without the broken short lines and the stray bold text.
- **The window reopens at the size and position you left it at**, maximized if it was maximized. A
  window saved off to the side of a display you no longer have is moved back into view, and one too
  large for the screen you have now is ignored in favour of the default size. The first launch, as
  before, opens at the default size, centered.
- **Linux builds now need Ubuntu 24.04 or a similarly recent distribution** (glibc 2.39 or later,
  such as Debian 13 or Fedora 40). Screen capture needs a newer PipeWire than Ubuntu 22.04
  provides, so the AppImage and `.deb` are now built on 24.04 and won't start on older systems.
- On Windows, an MCP server whose command isn't installed (for example `npx` without Node.js) now
  fails with "Command not found" and install hints, instead of a vague connection error.
- Security updates for several dependencies flagged by GitHub: the AWS connections no longer pull
  in an older TLS library with known certificate-checking and crash bugs, and the bundled HTML
  sanitizer and helper libraries are on patched versions.
- Exporting a chat to Word now leaves out images that aren't PNG, JPEG, GIF, WebP or BMP. The
  image reader built into the Word converter could hang forever on some malformed image types, so
  those images are skipped instead of freezing the export.

### 2026-09-30

- **OpenAI GPT-6.1 Sol** is now in the model dropdown, as **(US)** only — Bedrock does not offer a
  global or single-region option for it yet, so it runs in US East (N. Virginia, Ohio) and US West
  (Oregon). OpenAI positions it as coming close to GPT-6 Astra on demanding work — investigating
  codebases, reading complex documents, multi-step agent and computer-use tasks — at Sol-tier prices:
  $2.20 per million tokens in and $11 per million out, a fifth of what Astra costs in. It takes text
  and images, has a 1M-token context window, can return up to 131,072 tokens in one answer, and
  thinks by effort level, so **Deeper** asks it to think as hard as it can. Prompt content is cached
  automatically and there are no cache settings to set. Prompts longer than 272,000 tokens bill at
  double the input rate for the whole request, so the cost shown in the app is a floor on very long
  conversations. GPT-6 Sol and the rest of the GPT-6 family stay in the list.
- **Voice Chat (Nova Sonic) has been removed.** It had not worked reliably, and it is not coming
  along to the new Rust-based desktop app the rest of the app is moving to. Its sidebar entry, its
  page, and its region status in Settings are gone; everything else works as before.
- **Tool calls from models that can't stream tool use now receive their inputs.** For those models
  the app falls back to a non-streaming request, and that path dropped each tool call's input, so
  tools ran with nothing filled in and usually failed or did the wrong thing. They now get the
  input the model actually sent.
- **HTML previews in chat are now isolated from the app.** Scripts in a model-written HTML block
  still run in its preview, but they can no longer reach the app itself — before, a page the model
  wrote (or was tricked into writing by content it read) could run tools on your machine without the
  usual approval prompt. Diagram labels and the JSON viewer also no longer turn model or tool output
  into live page elements.
- **The code editor in agent and tool settings now works offline.** It used to download itself from
  a public CDN each time; it now ships with the app.
- **The app is now a native desktop app built in Rust (Tauri) instead of Electron.** It looks and
  works the same, but the download is much smaller and it starts faster, because it no longer
  carries its own copy of Chrome and Node.js. Your settings, agents and chat history carry over
  automatically on first launch; there is nothing to export or import. On macOS the system counts
  it as a new app, so it asks once more for Screen Recording and Camera permission the first time a
  tool needs them. On Windows it installs with a standard setup program (the file ending in
  `-setup.exe`).

### 2026-09-29

- **xAI Grok 4.7** is now in the model dropdown, as **(Global)** and **(US)**; as with Grok 4.6 there
  is no single-region option. It is xAI's newest frontier model, built on Grok 4.6 with a focus on
  long-running agents and more ambitious interactive work, and it keeps the 500K-token context
  window, image input, and always-on reasoning where the thinking control sets how hard it thinks
  rather than whether it thinks at all. Prompt content is cached automatically, so no cache settings
  are involved. It costs $2 per million tokens in and $6 per million out on Global routing, the rate
  the app uses for its cost estimates; US routing bills about 10% more than that. Grok 4.6 stays in
  the list. Grok 4.7 also reaches Jakarta and Melbourne, which Grok 4.6 does not.

### 2026-09-28

- **Claude Sonnet 5.5** is now in the model dropdown, as **(Global)**, **(US)**, **(EU)** and
  **(JP)**. It is Anthropic's newest Sonnet — better at coding, agentic work and following
  instructions than Sonnet 5 — with a 1M-token context window, output up to 128,000 tokens per
  response, images as input, adaptive thinking and prompt caching. It costs the same as Sonnet 5, $2
  per million tokens in and $10 per million out, so it is now the better all-round default of the
  two.

### 2026-09-24

- OpenAI's **GPT-6 Sol** and **GPT-6 Luna** are now in the model dropdown, as **(Global)** and
  **(US)**. They are the mid-priced and cheap tiers of the GPT-6 family that GPT-6 Astra tops: Sol
  for everyday reasoning and coding at $2.00 in and $10.00 per million tokens out, Luna for
  high-volume classification, summarization and routing at $0.10 in and $0.50 per million out. Both
  take tool use, output up to 128,000 tokens per response, and work in reasoning-effort levels rather
  than a thinking budget, so **Deeper** asks them for the highest effort they offer. Bedrock serves
  them only through cross-region inference, so there is no single-region variant. **The prices above
  are provisional** — they are OpenAI's own published rates, and the figures shown in the dropdown
  will be corrected once AWS publishes Bedrock pricing for these two models, which may differ.
- **Chats with a Docker sandbox now have a sandbox panel**, opened from the tab on the right-hand edge
  of the chat or from the whale menu. It shows what the container actually is — image, how long it has
  been up, CPU and memory against their limits — lists each service with its published ports as links
  that open in your browser, and keeps an **Activity** log of every command the agent ran in there:
  when it started, how long it took, and whether it finished, failed, stopped for input, was left
  running in the background, or outlived its timeout. Until now the only way to see any of this was to
  ask the agent or open a terminal of your own. The log lives in the sandbox's own folder, so it is
  still there after a restart, and rows are tagged so a command the agent ran is never confused with
  something you did.
- **A Compose tab** shows sandboxes that use Docker Compose as a diagram: every service with its
  image and whether it is up, the published ports that reach it from your browser, the folders mounted
  into it, and a note that services can reach each other by service name. The compose file itself is
  below the diagram, with buttons to copy it or open its folder. Single-container sandboxes say so
  instead.
- **The chat history now shows which chats have files or a sandbox**, as a paperclip and a Docker
  whale beside the title. Both are drawn in the row's own text colour rather than Docker blue: on a
  dense list they are metadata, not something to click.
- **Network and disk joined CPU and memory** in the panel's Overview, as throughput with running
  totals. The disk figure covers the container's own filesystem only: work in `/workspace` or `/data`
  is your own disk through a mount and Docker does not count it, which the panel now says. Hosts that
  report no block IO at all — Docker Desktop on macOS among them — say that instead of showing zero.
- **The stack diagram opens full window** when you click it, with zoom controls and Esc to close.
- **The sandbox panel now stops above the message box** instead of running to the bottom of the
  window, so the input keeps its full width and none of its icons are covered.
- **A terminal tab per container.** A compose stack gets a row of tabs in the Terminal view, one for
  each service, each with its own shell and scrollback. Switching between them is free — the shells
  live in the background and replay their recent output when you come back.
- **The panel also carries a real terminal inside the container.** Colours, `vim` and `htop`,
  tab completion, history, Ctrl-C, and it resizes with the panel — the quickest way to see what the
  agent left behind or fix something by hand. It is a genuine root shell with your project folder
  mounted at `/workspace`, so nothing you type is filtered or checked against the allowed-commands
  list; the app explains that once before the first time you open it. The model cannot reach this
  shell — there is no tool for it, so agents and background tasks have no way in. A shell you open
  keeps running while you look at other tabs or switch chats, and ends when the sandbox stops, the
  chat is deleted, or you quit. It needs Docker to be local: Docker Desktop, OrbStack, Colima and
  rootless installs all work, but a context pointing at a remote daemon disables the tab and says why.
- Sandbox state in the chat now updates the moment it changes rather than on a timer, so a sandbox
  appears as soon as the agent's first command creates one instead of up to fifteen seconds later.
- The whale menu is now just the actions — open panel, open folder, start, stop, remove — since the
  panel shows the state, services and ports it used to repeat.
- **The terminal is set in your chosen code font**, JetBrains Mono by default, and follows the Code
  font picker in Settings → Appearance while it is open. It was falling back to whatever plain
  monospace font the system offered.
- **Fixed the message box and its icons being hidden underneath the sandbox panel.** The input area
  now pulls in from the right while the panel is open, so the terminal can use the full height of the
  window without covering anything.
- **Fixed the terminal opening in a loop and never showing a prompt.** The terminal was being rebuilt
  as fast as it could be created, so it never lived long enough to draw anything, and the Activity log
  filled with "Interactive terminal opened" rows several times a second. Opening a terminal in a
  container that has stopped now says so and offers to reconnect, instead of retrying forever, and a
  terminal session is a single entry in the log that ends rather than two rows.

### 2026-09-22

- **Claude Opus 5.5** is now in the model dropdown, as **(Global)**, **(US)**, **(EU)** and
  **(JP)**. It is Anthropic's most capable Opus model — better at coding, knowledge work and
  long-running tasks than Opus 5 — with a 1M-token context window, output up to 128,000 tokens per
  response, images as input, and prompt caching. Adaptive thinking is always on for it and cannot be
  turned off. Bedrock serves it only through cross-region inference, so there is no single-region
  variant; Australian users can reach it through the **(Global)** entry. It costs $4 per million
  tokens in and $20 per million out — cheaper than Opus 5 at $5/$25.
- **Fixed pricing and model details being read from the wrong model** when one model's ID is the
  start of another's — Fable 5.1 was showing Fable 5's settings, and Opus 5.5 would have shown Opus
  5's.

### 2026-09-19

- Moonshot AI's **Kimi K3** is now in the model dropdown, as **(Global)** and **(US)**. It is
  Moonshot's most capable open-weight model, takes text and images, and has a 1M-token context
  window, which suits long coding sessions over a large repository. Bedrock serves it only through
  cross-region inference, so there is no single-region variant. Pricing shown in the dropdown is
  $3.30 in and $16.50 per million tokens out, the US rate; the **(Global)** variant bills about 10%
  less. Output is capped at 16,384 tokens per response, and thinking is left off for it: the model
  reasons internally, but Bedrock's Converse API errors out when reasoning from an earlier turn is
  sent back, so the app does not hold on to it between turns.
- **Fixed Kimi K3 failing on every request** with "This model doesn't support the temperature field".
  The model accepts neither Temperature nor Top P, so those two settings are now left out of its
  requests; both are ignored while Kimi K3 is selected, and Max Tokens still applies.

### 2026-09-09

- OpenAI's **GPT-6 Astra** is now in the model dropdown, as **(Global)** and **(US)**. It is OpenAI's
  most capable model — aimed at complex reasoning, coding, computer use, research and document
  creation — and takes a 1.05M-token context window with up to 128K tokens of output. Like the
  GPT-5.6 models it works in reasoning-effort levels rather than a thinking budget, so **Deeper**
  asks it for the highest effort it offers. The price shown in the dropdown, $11.00 in and $55.00 per
  million tokens out, is the rate for prompts under 272K tokens; the **(Global)** variant bills
  slightly less and prompts longer than that bill at roughly double, so treat the figure as a floor.
- **Max Tokens no longer has to be re-tuned for each model.** Every request now sends whichever is
  smaller, your Max Tokens setting or the output ceiling of the model you are using. Set it to
  128,000 for Claude Opus 5 and it stays there when you switch to Haiku 4.5, which quietly uses its
  own limit of 64,000 instead of refusing the request. The Background Agent task form follows the
  same rule: its Max Output Tokens field is now bounded by the model you picked for that task rather
  than a fixed 64,000, so models that can write more are no longer held back.
- Corrected the output limits the app had recorded for three models, which were lower than what the
  models actually allow: Amazon Nova 2 Lite now goes up to 64,000 tokens rather than 5,120, and both
  GPT-OSS 120B and GPT-OSS 20B up to 16,384 rather than 8,192. Every other model's limit was checked
  against its AWS model card and against the service itself, and was already right.
- **The app now has its own typeface, and you can choose it.** Text is set in Inter and code, file
  paths and identifiers in JetBrains Mono by default. Until now the app simply borrowed whatever font
  the operating system offered, which meant it looked meaningfully different on macOS, Windows and
  Linux. Inter is used with its optical-sizing axis switched on, so the many small labels around the
  interface get letterforms drawn for that size rather than shrunken-down large ones.
- **Settings → Appearance now has an Interface font and a Code font picker.** Both offer Inter or
  Geist for interface text, JetBrains Mono or Geist Mono for code, or your system font. They are
  deliberately two separate settings rather than one: JetBrains Mono makes 1, l and I — and 0 and O —
  much easier to tell apart, which matters when you are reading a tool-use ID, an ARN or a file path,
  so preferring Geist for the interface does not force it on code as well. All four typefaces are
  bundled with the app, so every combination works offline and looks the same on every platform. Your
  choice applies to all appearances and is remembered between sessions.
- Every text size now carries a line height and letter spacing chosen for it, instead of leaving both
  to browser defaults. Body text has more room to breathe, larger text is set slightly tighter, and
  numbers are fixed-width throughout — so a running token count or dollar cost no longer shifts
  sideways as the digits change.
- **Chat answers have been retypeset.** The styling for everything the model writes was largely
  inherited from browser defaults, and has been rebuilt: text is more readable at the same size
  because it finally has a line height, headings follow a real six-step scale rather than jumping
  from oversized to tiny, and inline `code` is now visibly set apart instead of looking like ordinary
  text. Code blocks scroll sideways rather than wrapping, so indented code keeps its shape, and their
  harsh black outline is now a soft rule that follows your chosen appearance. Tables size their
  columns to fit the content — previously every cell was locked to the same width, which cut off
  wider headings and wasted space on short ones — and numeric columns line up on their digits. Images
  render at their natural size instead of always being squeezed to half the width.
- Fixed: code blocks and table headings in chat followed your **operating system's** light or dark
  setting rather than the appearance chosen in the app. If you used the Dim appearance on a dark
  desktop, code blocks appeared as pale grey panels in otherwise dark surroundings.
- **Fixed: several parts of the app never went dark at all.** The Token Usage Analytics charts, the
  Mermaid diagrams rendered in chat, and the code interpreter's syntax highlighting were all checking
  for dark mode in a way that could never be true, so they stayed light no matter which appearance you
  chose. The side-by-side diff viewer had the opposite problem — it was pinned to a dark theme, so it
  showed a dark panel even in the Light appearance. All of them now follow your chosen appearance.
- Fixed: the to-do panel had no height limit, so a long list could run off the bottom of the window.
  The code interpreter's code panel had the same problem.
- Fixed: numbers in the JSON viewer were meant to be colour-highlighted and never were.
- Smaller corrections with visible effect: text areas that were meant to resize only vertically were
  resizing in both directions, and the chat input box sat a few pixels lower than intended because two
  conflicting positions were applied to it.
- **New appearance: Newspaper.** A flat, monochrome view in the spirit of newsprint or an e-ink reader
  — square corners, hairline rules and no shadows anywhere. The page is a toned grey rather than white,
  so it does not glare, with the faintest warm cast that newsprint has; it stops well short of sepia.
  Status colour is the one exception: success and error stay green and red, which makes them the only
  colour on screen and easier to spot than in any other appearance. Pick it under
  Settings → Appearance.
- **New appearance: Charcoal.** A warm grey rather than the blue-black of the existing Dark view,
  with amber as its accent — used for links, the active sidebar item, focus outlines and selected
  states. One thing to be aware of: because amber is the accent here, it no longer signals a warning,
  so warnings in this appearance are shown in a different colour and always carry an icon and wording
  rather than relying on colour alone.
- There are now five appearances, listed from lightest to darkest — Light, Newspaper, Dim, Charcoal
  and Dark — and an unrecognised saved value falls back to Dim instead of leaving the window unstyled.
- **Every appearance now reaches the whole window.** Previously an appearance only changed the parts
  of the interface that had been converted to use it; the rest fell back to a fixed grey palette, which
  is why Charcoal looked like the Dark view with patches and why Newspaper was the only appearance
  whose corners actually squared off. Colour, corner radius and shadow are now defined in one place per
  appearance and every surface reads from it — including dialogs, dropdowns, toggles, tooltips, tables,
  the command palette, notifications and the guided tour, all of which previously ignored your choice.
- **The interface is considerably tighter.** Interface text is 12.5px, the size used by editors and
  other dense professional tools, with 11px labels; corners are 3px on controls and 5px on panels
  instead of 6 and 8; icons are 16px rather than 20–24px; table rows are about 26px tall instead of
  roughly 56; the sidebar is 40px wide rather than 56; and vertical spacing throughout is about half
  what it was. Considerably more fits on screen, and the result reads as an instrument rather than a
  consumer app. Text the model writes is deliberately left at its previous, larger size — that is
  reading material, not interface.
- Empty screens, code editors and diagram panes are deliberately left roomy; shrinking those makes an
  intentionally empty screen look broken.
- **Tool calls in the transcript take up far less room.** Each collapsed tool row was carrying 20px of
  padding above and below and a 24px chevron around 12px of text, so a turn with twenty tool calls
  spent roughly 800 pixels on framing before any content appeared. Rows are now about a third of their
  previous height, which means much more of a long agent run fits on screen at once.
- Dialogs, tooltips, buttons, inputs, dropdowns and tables now share one set of corner radii, paddings
  and text sizes. Previously each was using the UI library's stock styling, which was never configured
  for this app, so spacing and rounding varied noticeably from one dialog to the next.
- **The interface is less decorated and more consistent.** Emoji that were standing in for interface
  icons have been replaced with real icons at sensible sizes — including a 48-pixel emoji as the
  tool-settings placeholder, a spinning hourglass emoji used as a loading indicator, and an emoji
  serving as an error icon. The waving hand has gone from the welcome message. The "Thinking" and
  "Reasoning" labels no longer use animated colour-shifting gradient text, which also stops seven
  continuously repainting animations; they are now simply drawn in the accent colour. Gradient fills on
  the sidebar's selected item and on several buttons are replaced by solid accent colour. Page and
  dialog titles were set at 30 or 24 pixels in heavy bold over 14-pixel body text and are now a
  calmer 20 pixels. Tool results show a small status dot and a quiet badge instead of a large
  saturated tick on every single call, and the token breakdown no longer tints four adjacent figures
  blue, teal, yellow and orange. The one emoji that stays is the one you choose for your own avatar.
- Fixed: the guided tour's highlight colour was a hardcoded orange that matched nothing else in the
  app; it now uses the current appearance's accent colour.
- **Buttons, text fields and form labels are consistent everywhere.** There were two different primary
  button designs in use — differing in colour, padding, corner radius and whether they showed a focus
  outline at all — plus three text-field designs and two label designs. All of them now come from one
  shared set, so every text field takes the same focus outline and every button is keyboard-focusable
  with a visible ring. Corners follow one rule too: 6px on controls like buttons and inputs, 8px on
  containers like cards and dialogs, and fully round reserved for avatars and count badges. Previously
  four different corner radii were used interchangeably across roughly 850 places.
- **The app now respects "Reduce motion".** If you have that turned on in your operating system's
  accessibility settings, spinners, pulses and transitions no longer animate. Previously the setting
  was ignored entirely.
- The tile behind an agent's icon is now a neutral shade rather than a tint of the accent colour,
  everywhere it appears — the My Agents list, the agent editor's Name & Icon row, and the Agent
  Directory's cards and detail panel. In the Charcoal appearance that tint was a
  wash of amber, which sat awkwardly behind icons in whatever colour the agent uses. Built-in and your
  own agents are still told apart by the tile, just by light and dark rather than by hue.
- Buttons and chips with a tinted background had a hover state that changed nothing — 45 of them, where
  the fill and the hover had ended up as the same colour. They now visibly respond again.
- **Mermaid diagrams are now greyscale and follow your appearance.** They previously used the diagram
  library's own blue-purple-pink palette, which matched none of the five appearances — and looked
  especially out of place in Newspaper. Nodes, subgraphs, sequence boxes, Gantt bands and pie slices are
  now drawn in shades of the current appearance's own grey, so a diagram reads as part of the app rather
  than pasted into it. They are shades rather than one flat tone, so the different parts of a diagram
  are still told apart, and each label's colour is picked for contrast against the shade behind it.
  Diagrams also redraw when you switch appearance, instead of keeping the colours they were first drawn
  with, and exported Word and PDF copies use the same greyscale.
- **Toggle switches were hard to read in the dark appearances.** The moving part of the switch was
  drawn in the panel colour, which on Charcoal is darker than the switch itself — so the knob
  effectively disappeared, and the switch had an outline that looked correct on light appearances and
  wrong on dark ones. Knobs are now always light, the outline is gone, and the knob travels evenly
  between the two ends instead of stopping short. The switches are also smaller, in line with the rest
  of the interface.
- **Fixed: generated chat titles could come back as Markdown.** Titles were arriving with asterisks,
  backticks, quotes or a "Title:" prefix, and occasionally as a whole sentence or several lines. The
  instructions given to the model are now explicit that a title is short plain text, and — since an
  instruction is only a request — anything that still comes back formatted is stripped before the title
  is saved. Titles are capped at 60 characters, cut on a word boundary. Existing titles are left as they
  are; use Generate Title on a chat to replace one.
- **Fixed: the assistant's replies were not being saved to chat history.** Since the 3 September build,
  reopening a past conversation showed your own messages and the tool results, but none of the answers
  — they were dropped on the way to disk. Prompts, tool results and errors were all saved correctly,
  which is why the sessions looked superficially intact. This is now fixed, and the code is arranged so
  the same mistake cannot happen again silently.
  **Conversations recorded between 3 September and this build are missing their replies permanently**;
  the text was never written, so there is nothing to recover. Older conversations are unaffected.

### 2026-09-08

- Agents can now be moved between machines as files. Every agent's ⋮ menu has a **Download YAML** option that saves its configuration wherever you choose — the Downloads folder and the agent's name are filled in for you. The file is written to be portable: the internal id and the flags recording where this particular copy came from are left out, so the same file can be handed to anyone.
- New **Import Agent** button on the My Agents page, next to Add New Agent. Pick a YAML or JSON agent file and it becomes one of your own agents: fully editable, not read-only like an agent shared through a project folder. It gets a fresh id, so importing the same file twice gives you two separate agents rather than overwriting anything, and a name that already exists is numbered instead of being duplicated silently. A file missing the pieces an agent cannot work without — name, description or system prompt — is refused with a message naming what is absent, rather than being added half-broken.
- Sharing an agent is now reversible. Once an agent has been saved to a project's `.bedrock-engineer/agents/` folder, its ⋮ menu offers **Delete Shared File**, which removes that file after showing you its full path to confirm. Your own copy of the agent is untouched — this only stops the agent appearing for everyone who opens the project. Agents shared to an organization's S3 bucket are unaffected and do not show the option.

### 2026-09-04

- On Windows, a Docker sandbox that mounts a folder from your project no longer writes that path with backslashes into its compose file. Docker Compose expects forward slashes there, so the mount could fail to resolve; sandbox paths are now always written in the form Compose understands.
- Attachments now work like sandboxes: they are files on disk in your project directory instead of text pasted into the message box. Dropping, pasting or picking a file writes it into `attachments/<chat-title>-<id>/` right away, and images are files too, so the thumbnail strip above the input is gone. A paperclip button sits next to the export buttons — always available, with a badge for how many files this chat has — and its menu lists each file with its size, deletes one on the spot, adds more through a file picker, and opens the folder in Finder, Explorer or your Linux file manager. The folder is named after the chat and is renamed automatically when the chat title changes, including when a title is generated for you.
- What the agent sees is rebuilt from the folder on every message. Edit an attached file in an editor, or remove it from the paperclip menu, and the next thing you send reflects that — no re-attaching and no rewriting of the conversation. Text, PDF and Word contents are extracted inline and images are sent as images; anything else, spreadsheets included, is listed by name and path so the agent can open it with its file tools. Very long files are trimmed with a note telling the agent to read the rest itself. Because the contents are assembled per message rather than stored, they no longer bloat the saved conversation or the Markdown, Word and PDF exports.
- Deleting a chat now deletes its attachments folder along with it, and deleting all chats clears the whole `attachments/` folder — including chats that never got as far as sending a message. The old shared `.bedrock-engineer/attachments` folder is no longer used and can be deleted by hand; files already in it are left alone.
- Add a full **User Guide** covering every page, every tool and every setting in plain language, written for someone comfortable with a computer but not deeply technical. It explains what each of the twenty-eight tools does and when to reach for it, walks through building an agent and writing a system prompt, covers attachments, Docker sandboxes, MCP servers, scheduled background tasks and the generators, and explains how to choose a model and keep costs down. It ends with a "how do I…" index of about fifty common questions, a glossary, and a troubleshooting section for the errors people actually hit. The agents in the app can read the guide themselves, so you can ask the chat how a feature works instead of going and looking it up.
- New **Help** button in the bottom-left corner, just above the GitHub link. It opens a chat called "Bedrock Engineer Help" with the user guide already attached, so you can ask how something works in plain language and get an answer taken from the guide rather than guessed at. The guide now ships inside the app, so it always matches the build you are running — no internet connection needed. The help chat runs on whichever model you picked as the **Light Processing Model** in Settings, which keeps it cheap; if you haven't picked one it uses your main conversation model. It has no tools at all, so it can only answer from the guide, and it will say so plainly when the guide does not cover what you asked. Clicking Help again returns to the same conversation instead of starting over, and the chat appears in your history like any other. If you have not set a project directory yet the guide cannot be saved as an attachment, so it is handed to the help agent directly instead and a note tells you why no attachment is shown.
- The User Guide now covers setting up AWS from nothing. A rewritten "Setting up access to Amazon Bedrock" section walks through enabling model access in the Bedrock console (including why access is per region and why cross-region models need it granted in several), attaching the IAM policy the app asks for, creating access keys in the console or with the API, installing the AWS CLI on macOS, Windows and Linux, and filling in the `~/.aws/credentials` and `~/.aws/config` files by hand or with `aws configure` — with named profiles, session tokens and IAM Identity Center covered, and a short set of commands for checking your setup works before blaming the app. Each failure those commands can produce is matched to the step that fixes it, the troubleshooting section gained entries for invalid and expired credentials, and the README points at the new section from the top, from Getting Started, from the install steps and from the documentation list.

### 2026-09-03

- Add xAI's Grok 4.6 to the model list. It is built for coding, agentic work and long-running tasks, with a 500K token context window, and its reasoning is always on — the thinking control sets how hard it thinks rather than whether it thinks at all. Bedrock offers it through two routing options, Global and US, both of which appear in the model dropdown; there is no single-region option. Grok also caches repeated prompt content automatically, so no cache settings are involved. Grok 4.3 is not offered, because Bedrock does not serve it on the API this app uses.
- The "Deeper" thinking setting now asks for the highest reasoning effort the model offers, one step up from before. This affects Grok 4.6 and the GPT-5.6 models; Claude models are unchanged.
- Switching to another chat no longer cancels the one you were in. An agent keeps working — including running its tools — while you read or write in a different chat, and the answer is there waiting when you come back. Chats still working are marked "Still responding" in the history list, and the stop button only ever stops the chat you are looking at. This also holds when you leave the Chat page entirely, so you can check settings mid-answer; reloading the window still cancels everything.
- Starting a new chat while one is answering leaves that answer running instead of throwing it away.
- Open the chat history panel by default when you go to Chat, instead of having to expand it each time.
- New **Docker Sandbox** tool. Turn it on for an agent and each chat gets its own long-lived container based on `ubuntu:26.04`, created the moment the agent first runs a command. Commands go into that container by default, so the agent can `apt-get install` whatever it needs and make a mess without any of it reaching your machine — and because it cannot reach your machine, the command allowlist no longer has to hold it back inside the container. Your project directory is mounted read-write at `/workspace`, so files move in and out freely, and the agent can publish ports if you want to open what it built in a browser. Long-running processes can be started in the background and their output read back later. If the agent genuinely needs your own machine it has to ask for it explicitly, and you get a dialog with the exact command before anything runs; you can allow it once or for the rest of that chat. A Docker whale appears next to the export buttons whenever the current chat has a container, with options to stop, start or remove it. Containers survive switching chats and are stopped when you quit the app, then come back with their installed packages intact. Deleting a chat removes its container, and the delete dialog now offers to delete the sandbox's data folder too — unchecked by default, so anything the agent wrote is kept unless you say otherwise. Compose and data files live under `docker-sandboxes/` in your project directory, using ordinary mapped folders instead of named volumes so you can just look at them. Needs Docker installed; if it is missing the agent tells you how to install it for your platform. Voice chat does not support sandboxes and keeps running host commands under the allowlist as before.
- Docker sandbox folders are now named after the chat instead of a raw timestamp, so `docker-sandboxes/fix-the-auth-bug-a3f21c/` replaces `docker-sandboxes/session_1756900000000/`. A sandbox usually gets created before the chat has a real title, so the folder is renamed automatically whenever the chat title changes — including when a title is generated for you. Renaming only moves the folder: containers keep running and installed packages survive, because Docker still tracks them by a project id that never moves.
- The Docker whale menu now shows the sandbox's folder name and has an **Open sandbox folder** option that reveals it in Finder, Explorer or your Linux file manager, so the compose file and the mapped data folders are easy to inspect. If the folder was deleted outside the app, or the system has no file manager to open it with, the reason is reported instead of failing silently.
- Give wide model logos room to be seen. Most model logos are square glyphs, but xAI's is a wordmark about two and a half times wider than it is tall, and fitting that into a square slot shrank it to a smudge. Wide logos now get the full width available in the chat avatar, the model dropdown and the model button, and they keep their proportions in the Word and PDF exports instead of being squeezed into a square. The icon column in the model dropdown is now a fixed width, so every row's name lines up regardless of which logo it has.

### 2026-09-02

- Keep Mermaid diagrams as `mermaid` code blocks in the Markdown export instead of turning them into
  PNG images. GitHub, VS Code and Obsidian render them from the source, and the diagram stays
  editable and readable as text. DrawIO diagrams and images pasted into the conversation are still
  written as PNGs into the `images/` folder, which is now only created when there is something to put
  in it. The Word and PDF exports are unchanged and still render every diagram as an image.
- Update the app's dependencies. The visible part is a newer Chromium: the app now runs on Electron 44, two major versions on from before, which brings current web platform and security fixes to everything rendered in the window. The AWS SDK also moves to its current release, so newly launched Bedrock models and regions are recognised without waiting for another build.
- Fix ten of the twelve security advisories reported against the app's dependency tree, covering URL parsing, query-string parsing and stylesheet processing. Two remain open with no fix published anywhere: they are denial-of-service bugs in the ICNS, JXL and HEIF decoders of an image library used by the Word exporter, which only ever hands it PNGs, so those code paths are never reached.
- Swap the icon offered for "Azure" when choosing an agent icon, because the icon set removed the Microsoft Azure mark. Agents already set to it keep working and pick up the new glyph.

### 2026-09-01

- Add Claude Fable 5.1 to the model list. It is Anthropic's most capable model for demanding reasoning and long-horizon agentic work, with a 1M token context window, up to 128K output tokens, and thinking always on. Bedrock currently offers it on the global endpoint and, for single-region routing, in US East (N. Virginia) only.

### 2026-08-31

- Call the action on built-in agents "Hide" instead of "Remove", and "Unhide" instead of "Restore default agents". Nothing is deleted — a hidden built-in agent is only taken out of the lists, and unhiding brings back its original configuration (any edits you made to it before hiding are not kept). Agents you created yourself are still deleted outright, and still say "Delete".
- Unhide agents one at a time. The button that brought every hidden agent back at once is now a dropdown that lists each hidden agent by name and icon, with "Unhide all" still available at the bottom.
- Allow hiding the Diagram Generator agent, which previously offered no way to remove it. The Diagram Generator page keeps working after you hide it, web search included, because it now falls back to the built-in agent configuration.
- Cut idle power use so the app is easier on a laptop battery. The largest saving: a hidden window was created at every launch to keep the background agent Task History screen warm, which meant a second full app process ran for the whole session even if you never used background agents. Task History now opens when you ask for it and is discarded when you close it, and scheduled background agent tasks are started directly instead of relying on that hidden window, so they still run after a restart without opening the page.
- Stop polling for TODO list changes when nothing can change them. The check now runs while a response is being generated, or while the TODO panel is open, instead of every two seconds for any conversation that has messages.
- Release the microphone when you leave voice chat. Previously the mic stream and audio processing kept running after navigating away mid-recording, so the system microphone indicator stayed on.
- Remove animations that ran forever with nothing happening: the gradient on the Act/Plan button in the message bar, and the rotating rings on the voice chat icon while it is idle. Loading and "Thinking" animations are unchanged.
- Check Docker availability once when the Code Interpreter settings are opened rather than every 30 seconds; the "re-check" button covers starting Docker afterwards.
- Reorganize Settings into five tabs — General, AWS, Models, Chat and Workspace — with a sidebar down the left, instead of one long column of sixteen unrelated sections. Everything that was there is still there, grouped with what it relates to: region now sits next to the credentials that use it and next to region failover, and model choice sits with the inference parameters and the light processing model. Each tab has its own address, so links into settings land on the right tab — the "Open Settings" prompt shown when voice chat isn't available in your region now opens the AWS tab directly.
- Translate the AWS and Language settings into Japanese. Those labels had no translation at all and showed in English regardless of the selected language.

### 2026-08-28

- Add a "My Agents" page with its own sidebar button (renamed from "Custom Agents"). It opens in the main window instead of an overlay, and is where you create, edit, duplicate, share, and remove agents.
- Replace the agent display button with a real agent dropdown, moved into the message entry area to the left of the model selector. The dropdown includes an "Edit agents" entry that opens the My Agents page.
- Label the message entry controls: Agent, Model, and Thinking.
- Rearrange agents by drag and drop on the My Agents page (card or table view). The arrangement is saved and is also used by the agent dropdown and `@` mentions. Dragging is disabled while a table column sort is active.
- Allow removing default agents you don't use. Removals persist across restarts (they are no longer re-seeded at startup) and can be undone with "Restore default agents" on the My Agents page. Default agents that back other pages (Website Generator, Diagram Generator) stay in place.
- Remove the agent settings overlay; the page and the dropdown replace it everywhere (chat, voice chat, and the background agent task form).
- Point the sidebar GitHub icon and the Help > GitHub Repository menu item at this fork (`ibehren1/bedrock-engineer-public`). Agent Directory contribution links still point upstream, since agent contributions go there.
- Rework the README: add a "What's different in this fork" section with current screenshots, point the download badges at this fork's releases, and correct the settings path (`Behrens AI`) and the stale agent screenshots.
- Move this dated update summary out of the README into `CHANGELOG.md`, and build release notes from that file instead of scraping the README. Each release now shows only the newest dated section plus a link to the full changelog.
- Document in `CLAUDE.md` that every commit changing user-visible behavior has to update this changelog in the same commit, so releases always carry accurate notes.
- Fix structured output (MCP server suggestions, website recommendations) failing with "Internal Server Error" whenever thinking mode was on: Bedrock rejects extended thinking together with a forced tool choice, so those requests now run without thinking. Failures also report the underlying reason instead of a bare "Internal Server Error".
- Fix the My Agents list looking like the first agent was stuck selected: the blue ring marked the agent active in chat, not the one you clicked, so it never moved. The ring is gone and the "Active" badge (with a tooltip) is the only marker. Clicking a built-in or shared agent now explains why it can't be edited instead of doing nothing.
- Make "Suggest for this agent" specific: it now reads the agent's system prompt, scenarios, allowed shell commands and additional instruction (not just its name and description), every search term has to be grounded in a phrase from that configuration, and the phrase is shown next to the term. Generic words like "automation" or "devops" are rejected, and if nothing in the agent names a system to connect to, the panel says so instead of guessing. Results are capped at 20 so the list stays scannable. Also adds an MCP Registry link next to the MCP Market one.
- Add a "Find MCP servers" panel to an agent's MCP Servers tab, backed by the official MCP Registry (registry.modelcontextprotocol.io). Search it directly, or press "Suggest for this agent" to have the model derive search terms from the agent's description, system prompt and enabled tools. Results carry real names, versions, package identifiers, required environment variables and hosted endpoints; "Load config" fills the JSON editor with a version-pinned command (or remote URL) so you review it before adding. The model only produces search terms, never package names. MCP Market is linked for browsing by category — it publishes no API and challenges automated requests, so the app doesn't read it.
- Choose an agent icon from about 38,000 icons instead of the previous 250. The icon picker keeps the curated, categorised list as its default view and adds ten icon collections you can browse or search by name: Tabler, Lucide, Phosphor, Material, Heroicons, Bootstrap, Font Awesome (plus brands), Simple Icons and Game Icons. Pick "All libraries" to search across every collection at once. Icon data loads only when you open a collection, so startup is unaffected, and existing agent icons keep working.

### 2026-08-27

- Add ability for user to specify that the agent use another agent profile (via @<profile name>) to accomplish a task. Results come back to the current agent. i.e. use @email to find email from Kevin and compose a reply. This allows the current agent to use the results of another agent's task (tools) without needing to switch back and forth.

### 2026-08-24

- Add an "Export chat to PDF" button. Same content as the Markdown export in a single self-contained PDF (Letter, 1" margins), typeset like the Word export.
- Write each avatar image once instead of once per turn (`user-avatar.png`, `assistant-avatar-<model ID>.png`).
- Group consecutive turns from the same party under a single heading.
- Word: Calibri, 10pt body, 18/16/14pt Heading 1/2/3, 1" margins, and half the space above each heading.
- Word: code blocks in 8pt Courier New, with line breaks and indentation preserved.
- Word: tighter table spacing — a one-line row is about half as tall as before.
- Diagrams (Mermaid/DrawIO) render at half size; Word also centers them.

### 2026-08-20

- Add username along side the avatar.
- Add a floating toolbar to copy just the highlighted text of a chat message as markdown or rich text.
- Make the stop-generation button red while inference is running.
- Make the new-chat button green.
- Add an "Export chat to Word (.docx)" button. Exports rich text (excluding ToolUse/ToolResult like the Markdown export), labels each turn as "Assistant – <model ID>" / "User – <username>", and embeds the user/assistant avatars as images.
- Markdown export now also embeds the avatar images and uses the "Assistant – <model ID>" / "User – <username>" headings.
- Publish the GitHub release even when the build job fails or produces no installers (notes-only release; any binaries that did build are still attached).

### 2026-08-19

- Serve OpenAI GPT-5.6 (Sol/Terra/Luna) models through the standard Bedrock Converse API and remove the OpenAI Responses API translation layer.
- Remove the GPT-5.5 and GPT-5.4 models, which are not available through Converse.
- Update the release workflow to keep build artifacts only for the most current release.

### 2026-08-14

- Update assistant icon in chat interface to match the model's icon.
- Hovering over the model icon in the chat interface now displays the specific model's name that produced the output.
- Add options to copy message to clipboard in either markdown or rich text format.

### 2026-08-13

- Update Claude Sonnet 5 pricing to $2/$10 per million input/output tokens.
- Update to address open security vulnerabilities.

### 2026-08-04

- Update OpenAI model pricing.
- Address new Dependabot security alerts.

### 2026-07-30

- Fix timezone issues for scheduled tasks.
- Create select/delete function for chat history.
- Address Dependabot security alerts.

### 2026-07-28

- Add support for OpenAI API / GPT-5.6 models..

### 2026-07-27

- Visual updates to add Dim and Dark themes.

### 2026-07-24

- Added support for Opus 5 model.

### 2026-07-23

- Remove gloss/glare from app icon.
- Add option in settings to allowlist models.
  - This to shrink the list of models shown in the model selection dropdown in the chat interface.
  - Simplfication for less technical users.
- Updated pricing and display costs of models in the chat interface dropdown.
- Allow users to pick an avatar from a picker for display in the chat interface.
- Removed prompt routers from model selection since they do not support modern models.
- Added support for Kimi 2.5 model.

### 2026-07-22

- Add setting to hide various route shortcuts from the sidebar.
- Update to name the app from productName in package.json.
- Update icon for the app.
- Update the agent chat assistant icon to the Bedrock logo.

### 2026-07-21

- Add drag-and-drop support for file attachments.
- Update versioning scheme to use date-based versioning from the last git commit.

### 2026-07-13

- Add conversation cost to the top right of the display.
- Update the ToDo list icon to flash as items change status.

### 2026-07-08

- Fix non-working Nova Sonic voice chat.
- Force chat naming at second user prompt.
- Update clear chat icon to a new-conversation icon.

### 2026-06-30

- Add Sonnet 5 model support.

### 2026-06-29

- Replace the unconditional auto-scroll-to-bottom with a hook that follows streaming output through the tool-use phase, then stops once the first line of the main response reaches the top of the message area. Respects manual scroll-up (pauses until the user returns to the bottom).
- Fix issue with title generation for chats.
- Add function to export chat history as a markdown file.
- Update to Electron 42.x.x.

### 2026-06-10

- Add support for Anthropic Fable 5 model.

### 2026-06-01

- Added Makefile targets for easy local building, installing, signing, and cleaning the project.
  - `make build-mac`: Build the project for macOS.
  - `make install`: Install the project.
  - `make sign`: Sign the project.
- Add Claude Opus 4.8 to model registry with global/JP/US inference profiles
- Add adaptive thinking mode for newer models (Sonnet 4.6, Opus 4.6/4.7/4.8) that use type: 'adaptive' without budget_tokens, while older models keep type: 'enabled' with budget_tokens
- Auto-translate thinking type in converse service based on model capabilities to prevent 400 errors when switching between model generations
- Fix Opus 4.7 supportsThinking (was incorrectly set to false)
- Remove duplicate Amazon Nova Premier entry from model registry
- Add date-based versioning via Makefile (YYYY.MMDD.N format)
- Add CLAUDE.md for codebase documentation
