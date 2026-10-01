# Update summary

Dated list of changes made in this fork of [aws-samples/bedrock-engineer](https://github.com/aws-samples/bedrock-engineer).
Newest first. The topmost dated section is published as the release notes for each build, so add a
dated section here for anything user-visible.

See the [README](./README.md#whats-different-in-this-fork) for a feature-by-feature
overview of the fork with screenshots.

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
- The macOS install instructions now cover a step that was missing: macOS 15 and later won't let
  Terminal change anything in your Applications folder until you enable it under System Settings →
  Privacy & Security → App Management. Without that, the required signing command fails with
  "Operation not permitted" and the app can hang on launch with no window and no error. The
  instructions also now clear the download flag your browser leaves on the app, which is what causes
  that hang.

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
