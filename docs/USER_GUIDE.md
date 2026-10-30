# User Guide

A complete, plain-language guide to this app: what every screen does, what every tool does, what
every setting means, and what to do when something goes wrong.

**Who this is for:** anyone comfortable using a computer. You do not need to be a programmer. Where
a technical word is unavoidable, it is explained the first time it appears, and again in the
[Glossary](#21-glossary) at the end.

**How to find things fast:** use the [Table of contents](#table-of-contents) for topics, or the
[Question index](#20-question-index-how-do-i) if you have a specific "how do I…" question.

**Or just ask.** The **Help** button in the bottom-left corner of the app opens this guide inside
the app, with search, and **Ask a question** there lets you ask in your own words instead of hunting
for the section that answers it. See [Section 23](#23-getting-help-inside-the-app).

> **Note for the AI assistant in this app**
>
> This file is written to be read by you as well as by people. If the user asks how the app works,
> answer from this file instead of guessing — read it with `readFiles` if you have that tool, or use
> the sections already in your context (the Help chat receives the ones that match each question).
> Every section has a stable heading and is written to stand on its own, so you can quote or
> summarize a single section without needing the rest. Section 20 maps common questions to the
> section that answers them. If the user's question is about something this guide does not cover, say
> so plainly rather than inventing an answer.

---

## Table of contents

1. [What this app is](#1-what-this-app-is)
2. [Quick start](#2-quick-start)
3. [The main window](#3-the-main-window)
4. [Setting up access to Amazon Bedrock](#4-setting-up-access-to-amazon-bedrock)
5. [Settings, tab by tab](#5-settings-tab-by-tab)
6. [The Chat page](#6-the-chat-page)
7. [Attachments: giving the agent your files](#7-attachments-giving-the-agent-your-files)
8. [Agents: what they are and how to build them](#8-agents-what-they-are-and-how-to-build-them)
9. [The complete tool reference](#9-the-complete-tool-reference)
10. [The Docker sandbox](#10-the-docker-sandbox)
11. [MCP servers: adding tools from outside the app](#11-mcp-servers-adding-tools-from-outside-the-app)
12. [Handing work to another agent with `@`](#12-handing-work-to-another-agent-with-)
13. [Agent Directory](#13-agent-directory)
14. [Background Agent: work on a schedule](#14-background-agent-work-on-a-schedule)
15. [Website Generator](#15-website-generator)
16. [Diagram Generator](#16-diagram-generator)
17. [Step Functions Generator](#17-step-functions-generator)
18. [Choosing a model](#18-choosing-a-model)
19. [Where the app keeps your files](#19-where-the-app-keeps-your-files)
20. [Question index: "How do I…"](#20-question-index-how-do-i)
21. [Glossary](#21-glossary)
22. [Troubleshooting](#22-troubleshooting)
23. [Getting help inside the app](#23-getting-help-inside-the-app)
24. [Local knowledge bases: letting an agent search your documents](#24-local-knowledge-bases-letting-an-agent-search-your-documents)
25. [Memory: rules and facts every chat knows](#25-memory-rules-and-facts-every-chat-knows)
26. [Accounting: what Bedrock is costing you](#26-accounting-what-bedrock-is-costing-you)

---

## 1. What this app is

This is a desktop app that lets you talk to an AI **agent**.

A regular AI chatbot can only write text back to you. An **agent** can also _do_ things: read and
write files on your computer, search the web, run commands, make pictures and videos, draw diagrams,
and look things up in your company's own documents. You tell it what you want in ordinary language,
and it decides which of its abilities — its **tools** — to use.

The AI models come from **Amazon Bedrock**, which is a service from Amazon Web Services (AWS) that
gives you access to AI models from Anthropic (Claude), Amazon (Nova), OpenAI, xAI (Grok) and others.
The app itself runs on your own computer. Your files stay on your computer unless a tool
deliberately sends something out — see [Section 19.4](#194-what-leaves-your-computer).

```mermaid
flowchart LR
  You([You]) -->|"plain language request"| App[This app on your computer]
  App -->|"your question + the tools it can use"| Bedrock[Amazon Bedrock<br/>AI models]
  Bedrock -->|"an answer, or a request<br/>to use a tool"| App
  App -->|"runs the tool"| Local[Your files, your terminal,<br/>Docker, the web]
  Local -->|"the result"| App
  App -->|"the finished answer"| You
```

The important idea: **the model never touches your computer directly.** It asks the app to run a
tool, and the app runs it. That is why you get to decide, per agent, which tools exist at all.

---

## 2. Quick start

Five steps to a working first conversation.

### Step 1 — Install the app

Download the installer from the
[releases page](https://github.com/ibehren1/bedrock-engineer-rust/releases/latest) and run it.

On a Mac, there are two one-time steps in the Terminal app before you open the app for the first
time, because the app is self-signed rather than signed by Apple:

1. **Let Terminal manage apps.** On macOS 15 and later, open **System Settings → Privacy & Security →
   App Management**, turn on **Terminal**, then quit and reopen Terminal. Without this, the next step
   fails with "Operation not permitted" and the app can hang on launch with no window and no error.
2. **Sign the app on your Mac**, and clear the download flag your browser put on it:

   ```bash
   sudo codesign --force --deep --sign - "/Applications/Bedrock Engineer.app"
   sudo xattr -dr com.apple.quarantine "/Applications/Bedrock Engineer.app"
   ```

   Each copy has to be signed on the Mac it runs on. Without it the app may not open at all, and
   permission pop-ups (screen recording, camera) may appear twice or behave strangely.

Then open the app. If macOS says the app was blocked, go to **System Settings → Privacy & Security**,
scroll down, find the blocked app, and click **Open Anyway**.

Run the two commands again only if you install a new version by hand from a downloaded file. Updates
the app installs itself ([Section 5.8](#58-updates-tab)) do not need them.

On Windows, if SmartScreen says "Windows protected your PC", click **More info**, then **Run anyway**.

On Linux, make the AppImage executable (`chmod +x bedrock-engineer-rust-*.AppImage`) and run it, or
install the `.deb` with `sudo apt install ./<file>.deb`. Both need a recent distribution (glibc 2.39 or
later), such as Ubuntu 24.04, Debian 13 or Fedora 40.

### Step 2 — Give the app your AWS keys

Open **Settings** (the gear at the bottom of the left sidebar), go to the **AWS** tab, and enter
either your access key and secret key, or the name of an AWS profile you already have set up. Pick
your region.

If you do not have AWS keys yet, or Bedrock has never been used in your AWS account,
[Section 4](#4-setting-up-access-to-amazon-bedrock) is the full walkthrough: enabling model access in
the Bedrock console, attaching the IAM policy, creating access keys, installing the AWS CLI, and
filling in the `~/.aws/credentials` and `~/.aws/config` files.

### Step 3 — Turn on the models you want to use

Still in Settings, go to the **Models** tab. Choose your main model. If the list is overwhelming, use
**Visible Models** to trim it down to the two or three you actually want.

Not sure which to pick? Start with a Claude Sonnet model. It is the best balance of speed, cost and
capability for everyday work. See [Section 18](#18-choosing-a-model).

### Step 4 — Pick a project folder

Go to the **Workspace** tab and choose a **Project Directory**. This is the folder the agent is
allowed to work in — where it reads files, writes files, and stores attachments and sandboxes. Make a
new empty folder for this if you are just trying things out.

### Step 5 — Chat

Click **Chat** in the sidebar. At the bottom of the screen you will see three controls: **Agent**,
**Model** and **Thinking**. Leave them at their defaults, type what you want, and press Enter.

Try: _"List the files in my project folder and tell me what this project seems to be."_

---

## 3. The main window

### 3.1 The sidebar

Every screen in the app is reachable from the icon strip down the left edge. The icons have no text
labels; hover over one to see its name and its keyboard shortcut.

| Icon           | Page                         | What it is for                                                                                                                                      |
| -------------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| House          | **Home**                     | The starting screen.                                                                                                                                |
| Speech bubbles | **Chat**                     | The main workspace. Talk to an agent, which uses tools to get things done.                                                                          |
| Robot          | **My Agents**                | Create, edit, arrange and hide your agents.                                                                                                         |
| Brain          | **Memory**                   | Rules and facts every chat can use. See [Section 25](#25-memory-rules-and-facts-every-chat-knows).                                                  |
| Book           | **Agent Directory**          | Browse ready-made agents other people have shared, and add them to your own list.                                                                   |
| Pulse          | **Background Agent**         | Set an agent to run automatically on a schedule.                                                                                                    |
| Feather        | **Website Generator**        | Describe a web page and watch it get built, with a live preview.                                                                                    |
| Merging arrows | **Step Functions Generator** | Build an AWS Step Functions workflow from a description.                                                                                            |
| Layout         | **Diagram Generator**        | Turn a description into an AWS architecture diagram.                                                                                                |
| Dollar sign    | **Accounting**               | Estimated Bedrock cost by month, day, model, feature and agent, and a monthly budget. See [Section 26](#26-accounting-what-bedrock-is-costing-you). |
| Gear           | **Settings**                 | Everything configurable.                                                                                                                            |

You can hide any icon you never use: **Settings → General → Sidebar Settings**. Hiding an icon does
not disable the feature; the page still exists at its address, and its keyboard shortcut still works.
Settings itself cannot be hidden.

Two more buttons sit at the **bottom** of the strip, below the gap. These are not pages, and they
cannot be hidden:

| Icon          | What it does                                                                                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Question mark | **Help** — opens this guide, with search, and a chat that answers questions about this app. Works offline. See [Section 23](#23-getting-help-inside-the-app). |
| GitHub cat    | Opens the project's page on GitHub in your web browser.                                                                                                       |

### 3.2 Keyboard shortcuts

| Shortcut                          | What it does                                                                                                                       |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `Cmd`/`Ctrl` + `K`                | Open the command palette — a search box for jumping to any page.                                                                   |
| `Cmd`/`Ctrl` + `,`                | Open Settings. Also in the macOS app menu as **Bedrock Engineer → Settings…**, which brings the window forward first.              |
| `Cmd`/`Ctrl` + `1` … `9`          | Jump straight to the 1st through 9th page in the sidebar list in [3.1](#31-the-sidebar) (Home is 1, Diagram Generator is 9).       |
| `Cmd`/`Ctrl` + `Shift` + `A`      | On the Chat page, switch between **Plan** and **Act** mode.                                                                        |
| `Enter` or `Cmd`/`Ctrl` + `Enter` | Send your message. Which one sends is your choice — see [Section 5.4](#54-chat-tab).                                               |
| `@`                               | In the message box, pick another agent to hand part of the job to. See [Section 12](#12-handing-work-to-another-agent-with-).      |
| `Cmd`/`Ctrl` + `A`                | In a file preview on the Files tab (after clicking in it), select just that file's content.                                        |
| `Cmd`/`Ctrl` + `S`                | Save the file you are editing on the Files tab.                                                                                    |
| `Cmd`/`Ctrl` + `+` / `-` / `0`    | Make everything bigger, smaller, or back to normal size.                                                                           |
| `Cmd`/`Ctrl` + `R`                | Reload the window, if it ever looks stuck. Your settings and chats are kept, but a reply that is still being written is cancelled. |
| `Cmd` + `W` (macOS)               | Close the window, which quits the app.                                                                                             |

`Cmd` is for macOS and `Ctrl` for Windows and Linux; hovering over a sidebar icon shows its shortcut
with the right key.

The **View** menu also has Force Reload, Toggle Developer Tools and Full Screen. Right-clicking in the
window offers Copy and Paste.

---

## 4. Setting up access to Amazon Bedrock

This app has no AI of its own. Every answer, image and diagram it produces comes from a model running
in **Amazon Bedrock**, a service inside your own AWS account, and AWS bills you directly for what you
use. So before the app can do anything useful, three things have to be true:

1. The models you want are **switched on** in your AWS account, in the region you plan to use.
2. Your AWS identity has **permission** to call Bedrock.
3. The app has **credentials** for that identity — either typed into its settings, or read from the
   files the AWS command line tools use.

Sections 4.1 to 4.7 walk through all three from nothing. If someone else administers your AWS
account, you can hand them [4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console) and
[4.3](#43-step-2--give-your-identity-permission-to-call-bedrock) and skip to
[4.6](#46-step-5--put-your-keys-in-the-aws-credentials-and-config-files).

> **Costs.** Bedrock charges per token (roughly, per word) for text and per image or video generated.
> There is no monthly fee and nothing to cancel, but there is also no free tier for most models, so a
> long conversation with an expensive model costs real money. [Section 18](#18-choosing-a-model)
> explains how to keep that small, and the app shows a running cost estimate for each conversation.

### 4.1 What you need before you start

- **An AWS account.** If you do not have one, create it at
  [aws.amazon.com](https://portal.aws.amazon.com/billing/signup). It needs a credit card and a phone
  number, and takes a few minutes to become active.
- **A way to sign in to the AWS console.** Either the root account (only use it for the initial
  setup), an IAM user, or a company single sign-on portal.
- **About fifteen minutes**, mostly waiting for model access to be granted.

### 4.2 Step 1 — Turn on the models you want in the Bedrock console

New AWS accounts cannot call most models until you ask for them. This is per-region, so if you plan
to use `us-east-1`, do this while `us-east-1` is the selected region.

1. Sign in to the [AWS console](https://console.aws.amazon.com/) and search for **Bedrock**.
2. Check the region name in the top-right corner. Change it if it is not the region you want.
3. In the left sidebar, scroll to the bottom and open **Model access** (under _Bedrock
   configurations_).
4. Click **Modify model access** (or **Enable specific models** on a fresh account).
5. Tick the models you want. At a minimum, tick the Anthropic **Claude** models — those are the ones
   this app is built around. Ticking Amazon **Nova Reel** (used by the video tool) and the
   **Stability AI** image models (used by the image tool) is worth it too. If you plan to give agents
   [local knowledge bases](#24-local-knowledge-bases-letting-an-agent-search-your-documents) with
   Bedrock embeddings, tick **Titan Text Embeddings V2** (the default) or the **Cohere Embed** model
   you intend to use.
6. Some model families — Anthropic's in particular — ask for a short **use case description**. A
   sentence or two about what you intend to do with it is enough ("internal engineering
   productivity assistant for my own use").
7. Submit. Most models are granted instantly and show **Access granted**. A few take minutes to
   hours. You cannot use a model whose status is still _Available to request_.

Two things that trip people up here:

- **Access is per region.** Granting Claude in `us-east-1` does nothing for `eu-west-1`.
- **Cross-region models need access in more than one region.** Many newer models are only offered
  through a _cross-region inference profile_ — their ID starts with a prefix like `us.`, `eu.`,
  `apac.` or `global.` — which forwards your request to whichever region has capacity. Grant access
  in every region in that group (for `us.`, that means at least `us-east-1`, `us-east-2` and
  `us-west-2`), or requests can fail intermittently even though your own region looks fine.

### 4.3 Step 2 — Give your identity permission to call Bedrock

Being allowed to sign in to AWS does not imply being allowed to call Bedrock. The identity whose keys
you give the app needs an **IAM policy** attached.

The app will write the policy out for you: **Settings → AWS → View required IAM policies**. It offers
two.

| Policy                            | Use it when                                                                                                                                                                   |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Recommended Policy (Complete)** | You want everything to work: chat, images, video, translation, knowledge bases, Bedrock Agents and Flows, guardrails, inference profiles, and reading and writing S3 objects. |
| **Basic Policy (LLM Only)**       | You only want to chat. It permits invoking models and listing them, and nothing else.                                                                                         |

Both policies allow `bedrock:InvokeModel` on every model, which also covers the embedding models used
by [local knowledge bases](#24-local-knowledge-bases-letting-an-agent-search-your-documents) (Titan
Text Embeddings V2 and Cohere Embed). If your administrator writes a narrower policy that lists
specific models, it has to include the embedding model too, or adding documents to a knowledge base
fails with an access error. (The "knowledge bases" in the Recommended policy's row above are Amazon
Bedrock Knowledge Bases in AWS, a different feature; local knowledge bases need no extra
permission beyond `InvokeModel`, and none at all if they use the on-device model.)

Copy the JSON from that dialog (**Copy**), then in the AWS console:

1. Go to **IAM → Policies → Create policy**, switch the editor to **JSON**, paste it in, and give it
   a name such as `BedrockEngineerAccess`.
2. Go to **IAM → Users**, pick the user you will use (or **Create user** — a dedicated user such as
   `bedrock-engineer` is tidier than reusing an admin), then **Add permissions → Attach policies
   directly** and attach `BedrockEngineerAccess`.

Or do the same from the command line, once the AWS CLI is installed ([4.5](#45-step-4--install-the-aws-cli)):

```bash
# save the JSON from the app as bedrock-engineer-policy.json first
aws iam create-user --user-name bedrock-engineer
aws iam put-user-policy \
  --user-name bedrock-engineer \
  --policy-name BedrockEngineerAccess \
  --policy-document file://bedrock-engineer-policy.json
```

If you would rather not manage the JSON, the AWS-managed policy **AmazonBedrockFullAccess** covers
the Bedrock half of it. It grants more than this app needs and does not include the `translate:` or
`s3:` permissions, so the translation tool and anything reading from S3 will still fail.

### 4.4 Step 3 — Create AWS access keys

An **access key** is a two-part password for the AWS API: an **access key ID** (starts with `AKIA`,
about twenty characters, not secret) and a **secret access key** (about forty characters, very much
secret).

**In the console:**

1. **IAM → Users →** click your user **→ Security credentials** tab.
2. Under _Access keys_, click **Create access key**.
3. Choose **Local code** or **Other** as the use case. AWS will suggest IAM Identity Center instead —
   that is genuinely better if your organization uses it (see
   [4.6](#46-step-5--put-your-keys-in-the-aws-credentials-and-config-files)), but a plain access key
   is fine for a personal account. Tick the confirmation and continue.
4. Optionally tag it with a description like `bedrock-engineer laptop`.
5. Click **Create access key**, then **Download .csv file** or copy both values now.

> **The secret is shown exactly once.** Close that page without copying it and there is no way to
> recover it — you have to delete the key and create another. Nothing is lost by doing that.

**From the CLI or API**, if you already have working admin credentials:

```bash
aws iam create-access-key --user-name bedrock-engineer
```

That prints JSON containing `AccessKeyId` and `SecretAccessKey`. The same call exists in every AWS
SDK as `CreateAccessKey`. An IAM user can hold at most two access keys at a time, which is what makes
rotation possible: create the second, switch over to it, then delete the first.

**Treat the secret like a password.**

- Never paste it into a chat, an email, a ticket, or a file you might commit to git.
- Never put it in a public repository. AWS scans for leaked keys and will quarantine your account,
  but by then someone may already have run up a bill.
- Delete keys you have stopped using: **IAM → Users → Security credentials → Actions → Delete**.
- If you think a key leaked, deactivate it immediately and create a new one.

### 4.5 Step 4 — Install the AWS CLI

The app does not require the AWS CLI. You can skip this section and simply paste your two keys into
the app's settings ([4.7, Option A](#47-step-6--point-the-app-at-your-credentials)).

Install it anyway if you can. It gives you `aws configure`, which writes the credentials files
correctly so the app can read them instead of storing a copy of your secret; it gives you a way to
test that your keys and model access actually work before blaming the app; and it is what every AWS
tutorial assumes you have.

**macOS**

```bash
# Option 1 — the official installer
curl "https://awscli.amazonaws.com/AWSCLIV2.pkg" -o "AWSCLIV2.pkg"
sudo installer -pkg AWSCLIV2.pkg -target /

# Option 2 — Homebrew, if you use it
brew install awscli
```

**Windows** — download and run
[the 64-bit MSI installer](https://awscli.amazonaws.com/AWSCLIV2.msi), or from PowerShell:

```powershell
winget install -e --id Amazon.AWSCLI
```

**Linux (x86_64)**

```bash
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install
```

On an ARM machine, replace `x86_64` with `aarch64`. Then open a **new** terminal window and check it:

```bash
aws --version
# aws-cli/2.x.x Python/3.x.x ...
```

If that says "command not found", the installer finished but your shell has not picked up the new
`PATH` yet — close the terminal and open a fresh one.

### 4.6 Step 5 — Put your keys in the AWS credentials and config files

The AWS CLI and every AWS SDK — including this app, when you use the profile option — read
credentials from two plain text files in a hidden `.aws` folder in your home directory:

| File          | macOS / Linux        | Windows                          | Holds                               |
| ------------- | -------------------- | -------------------------------- | ----------------------------------- |
| Credentials   | `~/.aws/credentials` | `%USERPROFILE%\.aws\credentials` | Access keys, per profile            |
| Configuration | `~/.aws/config`      | `%USERPROFILE%\.aws\config`      | Region, output format, SSO settings |

**The easy way — let the CLI write them.** Run:

```bash
aws configure
```

It asks four questions. Paste the access key ID, paste the secret access key, type your region
(`us-east-1` for example), and press Enter to skip the output format. That creates both files with
the right permissions, under a profile named `default`.

To keep this app's credentials separate from whatever else you do with AWS, give the profile a name:

```bash
aws configure --profile bedrock
```

**The manual way — edit the files yourself.** Create the folder and files if they do not exist. This
is also how you fix a typo, or add a second profile by hand.

`~/.aws/credentials`:

```ini
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY

[bedrock]
aws_access_key_id = AKIAI44QH8DHBEXAMPLE
aws_secret_access_key = je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
```

`~/.aws/config` — note that named profiles are written `[profile name]` here, but plain `[name]` in
the credentials file. That asymmetry is a genuine and long-standing oddity of the format:

```ini
[default]
region = us-east-1
output = json

[profile bedrock]
region = us-west-2
output = json
```

On macOS and Linux, lock the files down so other users on the machine cannot read them:

```bash
chmod 600 ~/.aws/credentials ~/.aws/config
```

**If your credentials are temporary**, as they are with `sts assume-role`, a company SSO portal or
Amazon internal tooling, there is a third value and it goes in the credentials file with the others:

```ini
[bedrock]
aws_access_key_id = ASIAI44QH8DHBEXAMPLE
aws_secret_access_key = je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
aws_session_token = IQoJb3JpZ2luX2VjEK...very long...==
```

Temporary keys start with `ASIA` rather than `AKIA` and expire — typically after one to twelve hours.
When they do, the app starts failing with a credentials or token error and you have to refresh them
and, if you typed them into the app rather than using a profile, paste the new ones in. This is the
strongest argument for using a profile: refresh the file and the app picks it up.

**If your organization uses IAM Identity Center (AWS SSO)**, do not create access keys at all:

```bash
aws configure sso            # once, to set the profile up
aws sso login --profile bedrock   # each time the session expires
```

Then point the app at that profile. It works for as long as the cached SSO session is valid; when the
session expires, requests fail until you run `aws sso login` again.

**Environment variables** (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_PROFILE`) are the other
place the SDKs look, and they are handy in a terminal. They are unreliable for this app, because a
desktop app launched from the Dock or Start menu does not inherit the environment of your shell. Use
the files or the app's own settings instead.

### 4.7 Step 6 — Point the app at your credentials

Open **Settings → AWS** (the gear at the bottom of the sidebar, then the **AWS** tab). There are two
ways to do this and you only need one.

**Option A — type the keys into the app**

Fill in:

- **AWS Access Key ID** — looks like `AKIA...`
- **AWS Secret Access Key** — the longer random string
- **AWS Session Token (optional)** — only if your keys are temporary (see above). Leave it blank otherwise.
- **AWS Region** — the region you enabled models in, such as `us-east-1`.

The app stores these on your own machine in its settings file. Nothing is sent anywhere except to
AWS.

**Option B — use an AWS profile (better if you have one)**

Turn on **Use AWS Profile** and type the name into **AWS Profile Name** — `default`, or `bedrock` if
you followed the naming above. The app then reads `~/.aws/credentials` every time it makes a request, so your secret
never gets copied into the app's own storage, and rotating or refreshing the key in that file takes
effect without touching the app.

Set the **AWS Region** in the app either way. The app uses the region you pick here, not the one in
your profile's config file. The region list puts the regions that offer Bedrock first, under
**Bedrock Supported Regions**, and everything else under **Other Regions**.

### 4.8 Check that it works

Fastest check: go to **Settings → Models** and see whether the model list is populated. That list
comes from a live call to Bedrock, so if models appear, your credentials and permissions are good.
Then go to Chat and ask the agent anything.

If that fails and you installed the CLI, work through it from the bottom up:

```bash
# 1. Are the credentials valid at all? Prints your account and identity.
aws sts get-caller-identity --profile bedrock

# 2. Can this identity see Bedrock in this region?
aws bedrock list-foundation-models --region us-east-1 --profile bedrock \
  --query 'modelSummaries[?contains(modelId, `claude`)].modelId'

# 3. Can it actually invoke a model? (Adjust the model ID to one you enabled.)
aws bedrock-runtime converse \
  --model-id us.anthropic.claude-sonnet-5-5 \
  --messages '[{"role":"user","content":[{"text":"Say hi"}]}]' \
  --region us-east-1 --profile bedrock
```

Reading the failures:

- Step 1 fails with `InvalidClientTokenId` or `SignatureDoesNotMatch` — the keys are wrong,
  truncated, or from a deleted user. Create a new access key.
- Step 1 fails with `ExpiredToken` — temporary credentials have expired; refresh them.
- Step 2 or 3 fails with `AccessDeniedException` — the credentials are fine but the IAM policy is
  missing or too narrow. Go back to [4.3](#43-step-2--give-your-identity-permission-to-call-bedrock).
- Step 3 fails with `ValidationException` mentioning model access, or "You don't have access to the
  model" — the key and policy are fine, but the model itself is not enabled in this region. Go back
  to [4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console).
- Step 3 fails with `ThrottlingException` — permission is fine, capacity is not. Retry, or switch to
  a cross-region (`us.`, `eu.`, `global.`) version of the model.

[Section 22](#22-troubleshooting) covers the same errors as they appear inside the app.

### 4.9 Which region should I pick?

The region decides which models you can reach and how far your requests travel. `us-east-1` and
`us-west-2` get new models first and have the widest selection. If your organization requires data to
stay in a particular country, use the region for that country.

The model list in Settings changes based on your region — a model that is not offered in your region
will not appear.

### 4.10 What permissions does my AWS user need?

Click **View required IAM policies** in the AWS settings tab. The app shows you the exact permission
policy to hand to whoever administers your AWS account. In short, it needs permission to call Bedrock
for text, images, video and embeddings, plus a few read-only permissions to list what is available.
[Section 4.3](#43-step-2--give-your-identity-permission-to-call-bedrock) walks through attaching it.

### 4.11 If your office uses a proxy

**Settings → AWS → Proxy Settings**. Turn on **Enable Proxy** and fill in the host, port and protocol
(HTTP or HTTPS). Username and password are optional. Calls to AWS and the web tools use a changed
proxy straight away; pages shown inside the app (images in chats, the Website Generator preview,
the Diagram Generator) pick it up the next time the app starts. For that in-app content: on macOS it
needs macOS 14 or later, and on Windows a proxy that asks for a username and password is not
supported.

---

## 5. Settings, tab by tab

![settings-sidebar](../assets/settings-sidebar.png)

Settings are split into eight tabs listed down the left: **General**, **AWS**, **Models**, **Chat**,
**Workspace**, **Sandbox**, **Knowledge Base** and **Updates**. Each tab has its own web-style
address, so a link can point at one directly (for example `#/setting/aws`). Open Settings with the
gear at the bottom of the sidebar's page list, or `Cmd` + `,` on a Mac.

Settings save as you change them; there is no Save button.

Some settings are not on these tabs, because they belong to one tool or one agent: the image models,
the screen and camera capture options and similar per-tool choices are in the **Tools** button below
the message box ([Section 6.10](#610-the-folder-tools-and-ignore-buttons)), and the ignore patterns
are in its **.ignore** button.

### 5.1 General tab

| Setting                         | What it does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Language**                    | Switches the app's own text between English and Japanese (日本語). It does not change what language the AI replies in — for that, just ask the agent, or say so in its system prompt.                                                                                                                                                                                                                                                                                                                 |
| **Appearance → Theme**          | **Light**, **Newspaper**, **Dim**, **Charcoal**, **Dark**, or **System** (follow your operating system) — listed from lightest to darkest. Dim is the default: dark, but softer than full black. **Newspaper** is flat black-on-white like a printed page or an e-ink reader — square corners, no shadows, and colour used only for success and error. **Charcoal** is a warm grey with an amber accent; because amber is the accent there, warnings use a different colour and always carry an icon. |
| **Appearance → Interface font** | The typeface used for labels, menus and body text: **Inter** (default), **Geist**, or your **system font**. All are bundled with the app, so any choice works offline and looks the same on macOS, Windows and Linux.                                                                                                                                                                                                                                                                                 |
| **Appearance → Code font**      | The typeface used for code, JSON, file paths and identifiers: **JetBrains Mono** (default), **Geist Mono**, or your **system font**. This is deliberately separate from the interface font — JetBrains Mono makes `1`, `l` and `I`, and `0` and `O`, much easier to tell apart, which matters when you're reading a tool-use ID or an ARN. Your font choices apply to every appearance.                                                                                                               |
| **User Avatar / User Name**     | Pick an emoji and a display name for yourself. These appear on your messages in chat and in exported documents. **Reset to default** goes back to the plain user icon.                                                                                                                                                                                                                                                                                                                                |
| **Sidebar Settings**            | Show or hide each navigation icon. Settings is always shown.                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| **Notification Settings**       | **Enable desktop notifications** (on by default) gives you a pop-up when a long answer finishes or a scheduled background task completes. Useful if you switch to other work while the agent runs.                                                                                                                                                                                                                                                                                                    |

Update controls used to be on this tab; they now have their own [Updates tab](#58-updates-tab).

### 5.2 AWS tab

Covered in full in [Section 4](#4-setting-up-access-to-amazon-bedrock): credentials or a profile,
region, the **View required IAM policies** dialog, and **Proxy Settings**.

### 5.3 Models tab

| Setting                                           | What it does                                                                                                                                                                                                                                                                                                                           |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **LLM (Large Language Model)**                    | Your default model for new conversations. You can still change it per conversation from the Chat page.                                                                                                                                                                                                                                 |
| **Visible Models**                                | An allowlist. Add models here (**Add a visible model**) to trim the chat's model dropdown down to only the ones you want to see. Leave it empty to see everything your region offers. The model you are using always stays visible.                                                                                                    |
| **Enable Application Inference Profiles**         | On by default. For cost tracking: lets you route requests through a tagged profile so your AWS bill can be split by project or team. See [the inference profile guide](./inference-profile/INFERENCE_PROFILE.md).                                                                                                                      |
| **Enable Region Failover on ThrottlingException** | Off by default. If AWS is busy and rejects your request ("throttling"), automatically retry in another region you list under **Failover Regions**. Turn this on if you get "too many requests" errors. Turning it off clears the list.                                                                                                 |
| **Inference Parameters**                          | Fine controls over how the model writes, and the default thinking budget. See below.                                                                                                                                                                                                                                                   |
| **Light Processing Model**                        | A model used for small background jobs like naming your chats and suggesting follow-up questions, and for the [Help chat](#23-getting-help-inside-the-app). The default, **Use Main Conversation Model**, uses your main model; pick a cheap Haiku or Nova model to save money. It does not affect the quality of your actual answers. |
| **Amazon Bedrock Guardrails**                     | Connect an Amazon Bedrock Guardrail to filter content. See below.                                                                                                                                                                                                                                                                      |

**Inference parameters, in plain terms:**

- **Max Tokens** (default 4,096) — the longest reply the model is allowed to produce. A "token" is
  roughly ¾ of a word. Raise it if answers get cut off mid-sentence; lower it to keep replies short
  and cheap. Each model has its own ceiling — 128,000 tokens on Claude Opus 5, 64,000 on Haiku 4.5,
  5,120 on the first-generation Nova models — and requests are capped at whichever is lower, this
  setting or the ceiling of the model you are using. So you can leave it high without a smaller model
  refusing the request; that model simply uses as much of it as it can.
- **Temperature** (default 0.5) — how much randomness. `0` is focused and repetitive; higher values
  are more creative and more unpredictable. For code and factual work, keep it low.
- **topP** (default 0.9) — another randomness dial that limits the model to its most likely word
  choices. Most people should leave this alone and adjust only Temperature.
- **Thinking Mode / Thinking Budget** — the default for the **Thinking** control on the Chat page
  ([Section 6.2](#62-the-three-main-controls)).

Some newer models (Claude Opus 4.7 and later, Sonnet 5 and later, Haiku 5.5, and a few others) no
longer accept Temperature and topP, so for those the app leaves both out and the model's own defaults
apply.

**Guardrails** are an AWS feature that blocks or masks content you do not want — profanity, personal
information, particular topics. You create the guardrail in the AWS console, then turn on **Enable
Guardrails** and enter its **Guardrail Identifier** and **Guardrail Version** (default `DRAFT`).
**Trace** logs why something was blocked, which is useful while you are tuning it.

### 5.4 Chat tab

| Setting                                    | What it does                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Tavily Search API Key**                  | Required for the web search tool. Get a free key from [tavily.com](https://tavily.com/). Without it, `tavilySearch` is left out of the agent's tools.                                                                                                                                                           |
| **Context Length**                         | How many past messages get sent along with each new one. Higher means the agent remembers more of the conversation, but every message costs money to send, so long context costs more. The default is 60; lower it if long chats get expensive, raise it if the agent keeps forgetting things you said earlier. |
| **Request Timeout (minutes)**              | How long to wait before giving up on a reply (default 15). Raise it if you ask for long, tool-heavy jobs that legitimately take a while.                                                                                                                                                                        |
| **Enable Prompt Cache**                    | Lets AWS remember the unchanging front part of your conversation so it does not have to be re-processed every turn. This makes replies faster and cheaper. On by default; leave it on.                                                                                                                          |
| **Advanced Setting → Enter key behaviour** | "When writing a message, press [Enter] to" **Send the message** (the default; `Shift+Enter` makes a new line), or **Start a new line** (then `Cmd`/`Ctrl` + `Enter` sends). Pick whichever matches the other apps you use.                                                                                      |

### 5.5 Workspace tab

| Setting              | What it does                                                                                                                                                                                                                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Project Setting**  | The project directory: the folder the agent works in. Click the path or **Select Project Directory** to choose it. All file tools are anchored here, and attachments and Docker sandboxes are stored inside it. Choose a real project folder, or a scratch folder if you are experimenting. |
| **Config Directory** | Where the app keeps its own settings and chat history. Shown, read-only, so you can find it; see [Section 19](#19-where-the-app-keeps-your-files).                                                                                                                                          |

You can also switch the project folder for the current chat from the folder button below the message
box ([Section 6.10](#610-the-folder-tools-and-ignore-buttons)).

### 5.6 Sandbox tab

Settings for the [Docker sandbox](#10-the-docker-sandbox), the private Linux container a chat can work
in.

| Setting                                        | What it does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Docker → Environment**                       | Whether Docker and Docker Compose are installed and running on this computer, with **Re-check** and when it was last checked. Compose is optional; without it, sandboxes are single containers.                                                                                                                                                                                                                                                                                                                                                       |
| **Default container image → Operating system** | The Linux distribution new sandboxes are built on: **Debian 13** (the default — the quickest to download and to install packages on), **Ubuntu 26.04**, **Amazon Linux 2023**, **Amazon Linux 2027**, **AlmaLinux 9**, **Rocky Linux 9** or **Red Hat UBI 9** (real RHEL, limited to Red Hat's freely available packages). Debian and Ubuntu install software with `apt-get`, the others with `dnf`. A chat that already has a sandbox keeps its OS until the sandbox is removed, and the agent can still ask for a different OS when it creates one. |
| **Container limits → Memory limit**            | 256 MB to 8 GB. Default 2 GB.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| **Container limits → CPU limit**               | 0.5 to 4 CPUs. Default 2.0 CPUs.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| **Container limits → Command timeout**         | 60 seconds to 30 minutes. Default 5 minutes. How long the agent waits for a command before getting control back. The command itself keeps running past this, so a long build is not killed.                                                                                                                                                                                                                                                                                                                                                           |

The limits apply to every service in every sandbox. Existing sandboxes pick up new limits when they are
recreated. The **Docker Sandbox** entry in the chat's **Tools** button shows a one-line summary of these
settings with an **Open sandbox settings** button that brings you here.

### 5.7 Knowledge Base tab

Settings for [local knowledge bases](#24-local-knowledge-bases-letting-an-agent-search-your-documents),
the documents and websites an agent can search. The tab has five sections:

- **Overview** — every knowledge base on this computer, with totals, health, **Optimize all** and a way
  to delete knowledge bases whose agent no longer exists.
- **Embedding model** — which model turns text into searchable form for new knowledge bases: Amazon
  Titan Text Embeddings V2 or Cohere Embed through Bedrock, or the on-device multilingual-e5-small,
  with its download button.
- **Chunking and retrieval** — passage size, results per search, and the balance between meaning and
  keyword matches.
- **Automatic re-sync** — whether file and folder sources pick up changes on their own, and how often.
- **Website crawler** — defaults for new website sources.

Each section is explained in [Section 24.6](#246-the-knowledge-base-settings-tab).

### 5.8 Updates tab

The app keeps itself up to date from this fork's GitHub releases.

| Item                                | What it does                                                                                                              |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| **Version**                         | The version you are running.                                                                                              |
| **Check for updates automatically** | On by default. The app checks shortly after it starts and then every hour, and downloads a new version in the background. |
| **Check for updates**               | Checks right now. Once a new version has downloaded, the button becomes **Restart now**.                                  |
| **What's new in …**                 | Appears while a new version is downloading or ready, with its release notes, so you can read them before restarting.      |
| **Changes in version …**            | What changed in the version you are running. **Show earlier changes** lists the versions before it.                       |

When an update is ready, a banner says so with **Restart now** and **Later**. Later hides the banner,
and the new version is used the next time you quit and reopen the app. Every download is checked
against the app's update signing key before it is installed.

On a Mac, an update the app installs itself needs none of the Terminal steps from
[Step 1](#step-1--install-the-app). macOS 15 and later may ask you to let **Bedrock Engineer** manage
apps the first time it updates itself (**System Settings → Privacy & Security → App Management**); if an
update fails with a permission error, turn it on there and check again. Because the app is
self-signed, macOS also asks for Screen Recording and Camera permission again after each update.

The Linux `.deb` package does not update itself; the AppImage does.

---

## 6. The Chat page

This is where most work happens.

![chat-conversation](../assets/chat-conversation.png)

### 6.1 The layout

```
┌───────────┬────────────────────────────────────────────────────────────────┐
│  Chat     │ [Chat] [Files] [Sandbox] [Terminal] [Todo] [Cost] [System prompt]│  chat tabs
│  history  ├────────────────────────────────────────────────────────────────┤
│  list     │                                                                │
│           │   Your messages and the agent's replies                        │
│  📎 🐳    │   (tool calls shown inline, collapsible)                       │
│           │                                                                │
│           ├────────────────────────────────────────────────────────────────┤
│           │ [Tools] [folder] [.ignore]   [brain] [terminal] [paperclip]    │  toolbar
│           │                [stop] [whale] [Files] [MD] [Word] [PDF] [new]  │
│           ├────────────────────────────────────────────────────────────────┤
│           │ Type message or attach files…                                  │  input box
│           │ [Agent ▾] [Model ▾] [Thinking ▾] [Interleave]   [Plan|Act] [➤] │  controls
└───────────┴────────────────────────────────────────────────────────────────┘
```

The chat history is on the left ([6.9](#69-chat-history)). The tabs across the top switch what the
main area shows ([6.6](#66-the-chat-tabs)); the message box stays at the bottom on every tab. The
buttons above the message box only appear when they have something to do — for example, the export
buttons appear once the chat has messages, and the whale only when the chat has a Docker sandbox.

Each row in the chat history carries a **paperclip** when that chat has attached files and a **whale**
when it has a Docker sandbox, so you can tell which conversations have something on disk behind them
without opening each one. Both are drawn in the same colour as the rest of the row — they are labels,
not buttons.

### 6.2 The three main controls

![agent-dropdown](../assets/agent-dropdown.png)

**Agent** — which agent you are talking to. Each agent has its own personality, instructions and set
of tools. The dropdown also has an **Edit agents** entry that jumps to the My Agents page.

**Model** — which AI model answers. The dropdown shows the price per million tokens in and out, so
you can see what you are spending before you spend it.

**Thinking** — how much the model is allowed to reason privately before it answers. More thinking
gives better answers on hard problems, but costs more and takes longer. It appears only for models
that can think.

| Thinking setting | When to use it                                                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **None**         | Simple questions, quick lookups, chatting.                                                                                                                                            |
| **Adaptive**     | Let the model decide how hard to think. Offered instead of the budgets below on newer models that support it.                                                                         |
| **Quick** (1K)   | A little planning.                                                                                                                                                                    |
| **Normal** (4K)  | Everyday multi-step work.                                                                                                                                                             |
| **Deep** (16K)   | Hard debugging, tricky analysis, long plans.                                                                                                                                          |
| **Deeper** (32K) | The hardest problems. On models that use effort levels instead of a thinking budget (Grok 4.7, Grok 4.6, GPT-6.1, GPT-6, GPT-5.6), this asks for the highest effort the model offers. |

Models that think adaptively offer just **None** and **Adaptive**; the others offer None and the four
budgets. Some models always think and cannot be turned down; the control will tell you when that is
the case.

On some Claude models, while thinking is on, there is also an **Interleave** toggle. When on, you see
the model's reasoning woven into its answer as it works, instead of only the final result. Handy for
understanding _why_ it did something.

### 6.3 Plan mode and Act mode

The **Plan / Act** toggle in the bottom-right of the input box (or `Cmd`/`Ctrl` + `Shift` + `A`)
controls whether the agent is allowed to change anything.

- **Plan mode** — only read-only tools work: `readFiles`, `listFiles`, `tavilySearch`,
  `fetchWebsite`, `recognizeImage`, `retrieve`, `invokeBedrockAgent`, `think`, and the memory tools
  (memory items are app settings, not project files). The agent can look at your files, search the web
  and think out loud, but it cannot write, move or delete anything, and it cannot run commands. MCP
  tools, including the [knowledge base tools](#910-local-knowledge-base-tools), are also off in Plan
  mode.
- **Act mode** — every tool the agent has is available.

Use Plan mode when you want advice, a review, or a proposal before anything happens. Switch to Act
mode when you are happy with the plan.

### 6.4 Sending a message

Type in the box and press `Enter` (or `Cmd`/`Ctrl`+`Enter`, depending on your
[Advanced setting](#54-chat-tab)). You can paste or drop files into the box too
([Section 7](#7-attachments-giving-the-agent-your-files)). Sending a message always switches back to
the **Chat** tab.

While the agent works you will see tool calls appear in the conversation — a line for each tool it
used, which you can expand to see exactly what it did and what came back. This is the single most
useful thing to read when an answer surprises you.

The **stop** button is red while a reply is being generated. It stops only the conversation you are
looking at.

When the agent saves, changes or removes a [memory item](#25-memory-rules-and-facts-every-chat-knows),
the reply shows a small note — **Memory saved**, **Memory updated** or **Memory removed** — with
**View** (opens the Memory page) and, for saved and removed items, **Undo**.

### 6.5 Answers keep running when you look away

If you switch to a different chat, start a new chat, or leave the Chat page entirely, the agent keeps
working. It carries on calling tools, and the finished reply is waiting when you come back.

Chats still working show **"Still responding"** in the history list on the left.

The one thing that does cancel everything is reloading or closing the window.

### 6.6 The chat tabs

The row of tabs across the top of the chat shows the conversation and everything around it. Each tab
uses the full width of the chat. A tab only appears when it has something to show, and each chat
remembers the tab you left it on until the app restarts (new chats open on **Chat**).

| Tab               | Appears                                     | What it shows                                                                                                                                                                                                                                                                                                                                                  |
| ----------------- | ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Chat**          | Always                                      | The conversation.                                                                                                                                                                                                                                                                                                                                              |
| **Files**         | When a project folder is set                | Your project folder, with previews and editing. A fuchsia count shows how many files the agent created or changed in this chat; it pulses briefly and stays until you look. See [6.11](#611-the-files-tab).                                                                                                                                                    |
| **Sandbox**       | When the chat has a Docker sandbox          | The container's state, services, terminal and activity log. Marked with a fuchsia dot until you first look. See [10.8](#108-the-sandbox-tab).                                                                                                                                                                                                                  |
| **Terminal**      | When you have opened a terminal in the chat | Shells on your own computer. See [6.12](#612-the-terminal-tab).                                                                                                                                                                                                                                                                                                |
| **Todo**          | Once the agent has made a todo list         | The agent's checklist for a long job. The tab shows how many items are done, for example `3/7`. If you want the agent to keep one, give it the `todo` tool ([9.6](#96-thinking-tools)).                                                                                                                                                                        |
| **Cost**          | Once the chat has messages                  | The tab label is the running cost in dollars. Inside: **Token Usage Analytics** — total tokens, total cost, model and message count, then a **Summary** (token and cost breakdown, including how much the prompt cache saved) and a **Time Series Analysis** (tokens and cost per turn). Open this if a conversation feels expensive and you want to know why. |
| **System prompt** | Always                                      | The full system prompt the model actually receives, with placeholders filled in and everything the app adds: tool rules, [memory items](#25-memory-rules-and-facts-every-chat-knows) when the brain button is on, and the [list of local knowledge bases](#244-how-the-agent-uses-a-knowledge-base). Use this when an agent behaves unexpectedly.              |

The token and cost figures are estimates from published prices; your AWS bill is the final word.

### 6.7 Exporting a conversation

Three buttons above the input box export the current conversation. They appear once the chat has
messages. A chat that still has its placeholder title gets a generated one first, which is used as
the file name.

| Format               | Best for                                 | Notes                                                                                                                                                                                                 |
| -------------------- | ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Markdown** (`.md`) | Keeping notes, putting in a repo, GitHub | Mermaid diagrams stay as editable `mermaid` code blocks, so GitHub, VS Code and Obsidian draw them for you. Draw.io diagrams and pasted images are written into an `images/` folder next to the file. |
| **Word** (`.docx`)   | Sharing with colleagues, editing further | Every diagram is rendered as a picture, in the same greyscale used on screen. Images that are not PNG, JPEG, GIF, WebP or BMP are left out.                                                           |
| **PDF**              | Sending something that will not change   | Every diagram is rendered as a picture.                                                                                                                                                               |

All three exports leave out tool calls and tool results, so you get the readable conversation rather
than the machinery. Each turn is labelled — `Assistant – <model name>` and `User – <your name>` — and
your avatar and the model's logo are embedded.

### 6.8 Copying part of a message

Select any text inside a message and a small floating toolbar appears. It copies just what you
selected, either as **Markdown** (plain text with `**bold**` style markers) or as **rich text**
(formatting preserved, for pasting into Word or email). You can also copy a whole message either way.

### 6.9 Chat history

The panel on the left lists your past conversations, newest first, and is open whenever you go to
the Chat page. Click one to reopen it — everything is still there. Each row shows the date, the
number of messages and, once the chat has cost anything, its estimated cost so far
([Section 26](#26-accounting-what-bedrock-is-costing-you)).

**Hide chat history** collapses the panel to a narrow strip instead of hiding it completely. The strip
has **Show chat history**, a green **New chat** button, and your most recent chats as one- or
two-letter initials — hover to see a title, click to switch to it.

Clicking the **Chat History** heading opens a menu with **Generate All Titles**, **Select** and
**Delete All**. **Select** lets you tick several chats (or **Select all**) and delete them together.
Deleting a chat also deletes that chat's
[attachments folder](#7-attachments-giving-the-agent-your-files), its
[Docker sandbox container](#10-the-docker-sandbox) and any [terminals](#612-the-terminal-tab) open in
it.

Chat titles are generated for you from the first thing you say, using the
[Light Processing Model](#53-models-tab). A title is always short plain text — a handful of words, no
formatting — and is trimmed to fit the sidebar. Each chat's **⋯** menu offers **Edit title** to type
your own (Enter saves, Esc cancels), **Generate title** to ask for a new one, and **Delete**.
**Generate All Titles** redoes every chat that still has a placeholder "Chat…" title.

### 6.10 The folder, Tools and .ignore buttons

Three buttons sit on the left above the message box:

- **Tools** (wrench) — the tools of the current agent, with a switch for each, and the settings of
  tools that have them: the Tavily key for web search, the models used to recognize and generate
  images, video settings, the code interpreter, the Docker sandbox summary, and which windows and
  cameras the capture tools may use.
- **The folder path** — your current project directory. Click it to switch project folders without
  going into Settings.
- **.ignore** (only when a project folder is set) — **File Exclusion Settings**: the files and folders
  the file tools skip, with **Global Settings** and **Project-specific Settings** tabs. See
  [Section 9.2](#92-file-system-tools).

The other buttons above the message box, left to right:

| Button                    | What it does                                                                                                                                                                 |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Brain**                 | Memory items on or off for this chat. Highlighted, with the number of items, while on. On for every new chat. See [Section 25](#25-memory-rules-and-facts-every-chat-knows). |
| **Terminal**              | Opens a shell on your own computer in a **Terminal** tab ([6.12](#612-the-terminal-tab)). Available once the chat has started.                                               |
| **Paperclip**             | Attachments: the count of attached files, and a menu to add, remove or reveal them ([Section 7](#7-attachments-giving-the-agent-your-files)).                                |
| **Stop**                  | Stops the reply being written in this chat.                                                                                                                                  |
| **Whale**                 | Only when the chat has a Docker sandbox: show the Sandbox tab, open its folder, stop, start or remove it ([10.4](#104-managing-sandboxes)).                                  |
| **Files** (folder)        | Opens or closes the Files tab.                                                                                                                                               |
| **Markdown / Word / PDF** | Export the chat ([6.7](#67-exporting-a-conversation)).                                                                                                                       |
| **New chat**              | Starts a new conversation, after asking you to confirm.                                                                                                                      |

### 6.11 The Files tab

The **Files** tab shows your project folder, so the agent's output is one click away instead of
somewhere in Finder or Explorer. Open it from the tab strip or the **Files** button above the message
box.

**The file list**

- **Project** shows the whole folder as a tree. **This chat** narrows it to the files the agent
  created or changed in the current chat, grouped by folder.
- Files the agent created or changed in this chat carry a **fuchsia dot** in the full tree too, and the
  folders that hold them open on their own. A folder you collapse stays collapsed.
- **Filter files…** narrows the list by name. The list updates by itself as files change.
- The header has **Refresh** and **Show in Finder** (**Show in Explorer** on Windows, **Show in folder**
  on Linux) for the whole project folder.
- **Resizing.** Drag the divider between the file list and the preview to make the list wider (to read
  long names) or narrower (to give the preview more room). With the divider focused, the arrow keys
  move it (hold `Shift` for bigger steps); a double-click puts it back to the default width. The width
  you choose is remembered.

**The preview**

Click a file to preview it:

- Text and code, with syntax highlighting and line numbers. Copying from it copies only the code, not
  the line numbers.
- Markdown and HTML open rendered; **Source** switches to the highlighted source and **Rendered** back.
- Images (PNG, JPEG, GIF, WebP, SVG, BMP, ICO, AVIF).
- PDF, page by page, with page buttons and zoom.
- Word documents (`.docx`).
- Files that cannot be previewed (archives, media, older Office formats, spreadsheets and the like) say
  so, with an **Open in default app** button.

After clicking in a preview, `Cmd` + `A` (`Ctrl` + `A` on Windows and Linux) selects just that file's
content, ready to copy, rather than everything in the window.

The preview's buttons:

- **Edit** — edit a text file in place. `Cmd`/`Ctrl` + `S` or **Save** saves it; an amber dot marks
  unsaved changes. If the file changed on disk meanwhile, you are told and can **Reload**, or on saving
  choose **Overwrite** or **Keep mine**. Leaving with unsaved edits asks first.
- **Pop out** — open the file in a window of its own, for reading or editing next to the chat. Each
  file gets one window, and popping out the same file again brings its window to the front. The
  window updates when the file changes on disk, says so if the file is deleted, and asks before closing
  if there are unsaved edits. Select-all works there too.
- **Open** — open the file in its default app.
- **Reveal** — show the file in its folder in Finder or Explorer.
- **Copy path**.

File paths in the agent's replies are links: click one to open that file on the Files tab.

### 6.12 The Terminal tab

The **Terminal** button above the message box opens a shell on **your own computer** — your usual
shell, started in the project folder (or your home folder if none is set) — in a **Terminal** tab.

- A chat can have several shells, shown as sub-tabs. **+** adds one and **×** closes one.
- Shells keep running while you switch tabs or chats, and end when you close them, delete the chat, or
  quit the app. A shell that exits stays listed, crossed out, with its exit code.
- Prompt icons drawn by tools like Starship and Powerlevel10k display correctly; the app ships the
  symbol font they need.

**The agent has no access to these shells.** There is no tool for them; they are only for you. For a
shell inside the chat's Docker container instead, use the Sandbox tab's terminal
([10.9](#109-the-interactive-terminal)).

### 6.13 Diagrams and web pages in replies

When a reply contains a Mermaid diagram, a draw.io diagram or a piece of HTML, the app draws it in
the chat:

- **Preview** and **Source** switch between the drawing and its highlighted source. Copying from the
  Source view copies only the code, and Mermaid's Source view has a copy button.
- Mermaid and draw.io diagrams have **Download as PNG** and zoom buttons.
- A Mermaid flowchart that comes out much wider than tall is turned to run top-to-bottom when that fits
  the chat column better. **As written** switches back to the direction in the source; PNG downloads
  and exports use whichever layout you see.
- HTML is shown in a locked-down preview that cannot reach the rest of the app.

---

## 7. Attachments: giving the agent your files

Attachments in this app are **real files in a real folder**, not text pasted into a message. That
one design choice explains everything else about how they behave.

### 7.1 Adding files

Three ways, all equivalent:

- **Drag and drop** a file onto the chat.
- **Paste** a file or an image from your clipboard.
- Click the **paperclip** button above the message box and choose **Add files…**. The paperclip is
  there even before the first message, so you can attach first and then ask.

The moment you do, the file is copied into your project folder at:

```
<your project folder>/attachments/<chat-title>-<id>/
```

The folder is named after the chat, and gets renamed automatically when the chat title changes —
including when a title is generated for you.

### 7.2 Managing files

The paperclip button always shows a badge with how many files this chat has. Its menu lists each
file with its size, and lets you:

- **Remove** a single file on the spot (the trash icon that appears when you hover over it)
- **Add files…** through a file picker
- **Open attachments folder** in Finder, File Explorer, or your Linux file manager

Chats that have files also show a small paperclip next to their title in the chat history, so you can
spot them without opening each one.

### 7.3 How the agent sees them

Here is the useful part: the agent's view of your attachments is **rebuilt from the folder every time
you send a message**. So:

- Edit an attached file in your normal editor, send another message, and the agent sees the new
  version. No re-attaching.
- Delete a file from the paperclip menu and the agent stops seeing it, starting with your next
  message.
- Drop a new file in with Finder or Explorer and the agent picks it up.

What the agent receives depends on the file type:

| File type                              | What the agent gets                                                  |
| -------------------------------------- | -------------------------------------------------------------------- |
| Text, code, Markdown, CSV              | The contents, inline                                                 |
| PDF                                    | The text, extracted                                                  |
| Word (`.docx`)                         | The text, extracted                                                  |
| Images (PNG, JPEG, …)                  | The actual image, so a vision-capable model can look at it           |
| Everything else, spreadsheets included | The file name and path, so the agent can open it with its file tools |

Very long files are trimmed — each file to its first 120,000 characters, and all attachments together
to 480,000 per message — with a note telling the agent to read the rest itself using `readFiles`.

Attachments are for files you want the agent to see in this one chat. For a library of documents an
agent should be able to search in every chat, use a
[local knowledge base](#24-local-knowledge-bases-letting-an-agent-search-your-documents) instead.

Because contents are assembled fresh for each message rather than stored in the conversation, they do
not bloat your saved chat history or your Markdown, Word and PDF exports.

### 7.4 Cleaning up

Deleting a chat deletes its attachments folder. Deleting all chats clears the whole `attachments/`
folder, including chats that never got as far as sending a message.

---

## 8. Agents: what they are and how to build them

### 8.1 What is an agent?

An agent is a saved bundle of four things:

```mermaid
flowchart TD
  A[An agent] --> B["<b>Identity</b><br/>name, icon, description"]
  A --> C["<b>System prompt</b><br/>standing instructions:<br/>who it is, how it works,<br/>what it must never do"]
  A --> D["<b>Tools</b><br/>which abilities it has<br/>at all"]
  A --> E["<b>Resources</b><br/>knowledge bases, allowed<br/>commands, MCP servers,<br/>flows, example scenarios"]
```

The same model behaves completely differently depending on the agent wrapped around it. A
"Documentation Writer" agent with no command-running tools and a prompt about clear writing is a
different assistant from a "DevOps Helper" with shell access — even though both are, underneath, the
same Claude model.

### 8.2 The My Agents page

![my-agents](../assets/my-agents.png)

Open **My Agents** from the sidebar. This is home base for agents.

- **Create** a new agent with **Add New Agent**, or **Search agents...** to find one.
- **Card view or table view** — toggle with the buttons at the top. Table view is easier for sorting
  and scanning when you have a lot of agents.

![my-agents-table](../assets/my-agents-table.png)

- **Rearrange by dragging.** The order you set is saved and reused everywhere agents are listed —
  the Agent dropdown in chat and `@` mentions. Dragging is switched off while a table column sort is
  active, because two orderings at once makes no sense.
- **Hide agents you do not use.** The app ships with several built-in agents. You cannot delete or
  edit those, because they get restored at every launch, but **Hide** takes them out of every list and
  the setting sticks across restarts. The **Unhide** dropdown lists everything you have hidden, so
  you can bring back one agent, or all of them (**Unhide all**). To change a built-in agent,
  **Duplicate** it and edit the copy.

Each agent's **⋮** menu has, depending on the kind of agent:

| Item                      | What it does                                                                                                                                     |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Edit**                  | Opens the agent editor ([8.3](#83-the-agent-editor)). For agents you made.                                                                       |
| **Duplicate**             | Makes your own editable copy, including copies of its [local knowledge bases](#24-local-knowledge-bases-letting-an-agent-search-your-documents). |
| **Download YAML**         | Saves the agent to a file you choose ([8.5](#85-sharing-an-agent)).                                                                              |
| **Convert to Strands**    | Exports the agent as a starter project for the Strands Agents framework, so it can run as your own Python program outside this app.              |
| **Save as File**          | Shares the agent with everyone who opens your project folder ([8.5](#85-sharing-an-agent)).                                                      |
| **Share to Organization** | Uploads the agent to your organization's shared agent collection ([13.1](#131-sharing-inside-your-organization)).                                |
| **Delete Shared File**    | On an agent that came from a project file: removes that file ([8.5](#85-sharing-an-agent)).                                                      |
| **Delete** / **Hide**     | Agents **you** created are deleted for real, together with their local knowledge bases. Built-in agents can only be hidden.                      |

**Import Agent**, at the top next to Add New Agent, reads an agent file someone sent you. See
[8.6](#86-importing-an-agent).

### 8.3 The agent editor

![agent-editor](../assets/agent-editor.png)

Clicking **Edit** (or **Add New Agent**) opens the editor, which has four tabs down the side: **Basic
Settings**, **MCP Servers**, **Knowledge Base** and **Tools**.

#### Tab 1 — Basic Settings

**Name & Icon.** The icon button opens a picker. It starts with a curated, categorised list, and adds
ten full icon collections you can browse or search: Tabler, Lucide, Phosphor, Material, Heroicons,
Bootstrap, Font Awesome (including brand logos), Simple Icons and Game Icons — roughly 38,000 icons
in total. Choose **All libraries** to search all of them at once. Long result lists are capped, so if
you do not see what you want, search a more specific word. Collections load the first time you open
one, so this does not slow the app's startup. You can also pick an **Icon Color**, or **Auto Select**
one.

**Description.** A sentence about what this agent is for. This is shown in lists, and is also used
when suggesting MCP servers.

**System Prompt.** The most important field. See [Section 8.4](#84-writing-a-good-system-prompt).

**Scenarios.** Example prompts shown as clickable starters when you open a new chat with this agent.
They can be generated for you from the system prompt. Good scenarios make an agent much easier for
someone else to pick up.

**Tags.** Labels for finding this agent later.

#### Tab 2 — MCP Servers

Connect external tool servers. See [Section 11](#11-mcp-servers-adding-tools-from-outside-the-app).

When the agent has local knowledge bases, a read-only **local-knowledgebase** card marked **Built-in**
sits above your own servers. It is added automatically, lists the knowledge base tools with their
descriptions, and has a **Test connection** button that should report "Connected: 3 tools available".
You cannot edit or remove it here; it goes away when the agent has no knowledge bases left.

#### Tab 3 — Knowledge Base

The agent's own searchable library of documents and websites, stored on your computer. Create a
knowledge base, add files, a folder or a website to it, and the agent gets a tool to search it. This tab is
explained step by step in [Section 24.3](#243-the-knowledge-base-tab-in-the-agent-editor).

#### Tab 4 — Tools

![select-tools](../assets/select-tools.png)

The Tools tab has two parts.

**Available Tools** — tick the tools this agent may use, grouped into categories: File System, Web &
Search, AI Services, System, Thinking, Memory, Agent Delegation, and MCP. A **Preset Category** menu
fills in a starting set ([9.11](#911-default-tool-sets)). MCP tools — those of your MCP servers and the
built-in knowledge base tools — are listed in the MCP category and are always on while the server is
configured; they cannot be switched off one by one. Every tool is documented in
[Section 9](#9-the-complete-tool-reference).

**Tool Detail Settings** — settings for the enabled tools that need them:

- **Knowledge Base Settings** (for `retrieve`) — the IDs of **Amazon Bedrock Knowledge Bases** in your
  AWS account that the agent may search. These are a different thing from the local knowledge bases
  on Tab 3.
- **Command Settings** (for `executeCommand`) — the **Allowed Commands** list.
- **Bedrock Agent Settings** (for `invokeBedrockAgent`) — Amazon Bedrock Agents (agent ID and alias ID)
  this agent may call.
- **Flow Settings** (for `invokeFlow`) — Amazon Bedrock Flows this agent may run.

**Allowed Commands** are a safety list for the `executeCommand` tool. Only commands matching a pattern
here can be run on your computer. `*` is a wildcard, so `npm *` allows every `npm` command and nothing
else. If this list is empty, the agent cannot run anything on your machine at all.

> **Safety note:** every pattern you add here is a command the AI can run on your computer without
> asking you first. Add specific patterns (`git status`, `npm test`) rather than broad ones. Never
> add a bare `*`. If you want the agent to be able to install packages and run anything it likes,
> use the [Docker sandbox](#10-the-docker-sandbox) instead — that keeps it away from your machine
> entirely.

The rule of thumb for tools: **give an agent the fewest tools it needs.** Fewer tools means less to
go wrong, lower cost per message (the tool list is sent with every request), and a model that stays
focused.

If you cancel a new agent before saving it, any knowledge bases you created for it are deleted.

### 8.4 Writing a good system prompt

The system prompt is standing instructions the agent reads before every single message. A good one
covers:

1. **Who it is** — "You are a careful technical editor."
2. **What it does** — "You review documents for clarity and factual accuracy."
3. **How it should work** — "Always read the whole file before suggesting changes. Quote the original
   line next to your suggestion."
4. **What it must not do** — "Never rewrite a document without showing the changes first. Never
   delete files."
5. **What 'done' looks like** — "Finish with a bulleted summary of what you changed and why."

**Placeholders.** You can drop these into the prompt and the app fills them in automatically:

| Placeholder           | Replaced with                                                    |
| --------------------- | ---------------------------------------------------------------- |
| `{{projectPath}}`     | Your current project folder                                      |
| `{{date}}`            | Today's date                                                     |
| `{{allowedCommands}}` | The commands this agent may run                                  |
| `{{knowledgeBases}}`  | The Amazon Bedrock Knowledge Bases it can search with `retrieve` |
| `{{bedrockAgents}}`   | The Bedrock agents it can call                                   |
| `{{flows}}`           | The flows it can run                                             |
| `{{allowedWindows}}`  | Windows it may screenshot                                        |
| `{{allowedCameras}}`  | Cameras it may use                                               |

You do not need a placeholder for [local knowledge bases](#24-local-knowledge-bases-letting-an-agent-search-your-documents):
the app adds their names and descriptions to the prompt by itself.

**Helpers in the editor:**

- **Generate system prompt** — describe what you want in a sentence and let the model draft the
  prompt for you. A great starting point that you then edit.
- **Show Preview** — see the prompt with all placeholders filled in and all automatic additions
  included, exactly as the model will receive it. Use this when an agent behaves unexpectedly.
- **Environment Context Settings** — two toggles for instructions the app can add to the prompt.
  The project path and the date are always included.
- **Project Rule** — a toggle that adds instructions telling the agent to load project-specific
  rules from a `.bedrock-engineer/rules` folder in your project. Turn it on when a project has its
  own coding standards or conventions you want followed automatically.
- **Visual Expression Rules** — adds instructions encouraging the agent to answer with diagrams and
  images where they help.

### 8.5 Sharing an agent

Open the **⋮** menu on any agent on the My Agents page. Two ways to send an agent elsewhere:

- **Save as File** writes the agent to `.bedrock-engineer/agents/` inside your project folder.
  Anyone who opens that project in this app gets the agent automatically. This is the way to put an
  agent under version control alongside the code it works on.
- **Download YAML** saves the agent to a file you choose, so you can email it, put it in a Git
  repository, or keep a backup. The save box opens on your Downloads folder with the agent's name
  filled in. The file is deliberately plain: it holds the agent's configuration but not its internal
  id or any record of where your copy came from, so it can be given to anyone.

Neither way includes the agent's
[local knowledge bases](#24-local-knowledge-bases-letting-an-agent-search-your-documents). They stay on
your computer; the person you share with has to build their own. **Share to Organization** leaves them
out too.

A shared agent loaded from a project file is edited by editing that file, and cannot have local
knowledge bases of its own. If you want to change it
just for yourself, **duplicate** it first, or use **Import Agent** on the copy you downloaded.

**Un-sharing.** On an agent that came from a project file, the **⋮** menu also has **Delete Shared
File**. It shows you the full path and asks before deleting anything. This removes only the file, so
the agent stops appearing for everyone who opens that project — your own copy in My Agents is left
exactly as it was. Agents shared to an organization's S3 bucket live in the cloud rather than in a
file, so they do not offer this.

### 8.6 Importing an agent

**Import Agent**, next to **Add New Agent** at the top of the My Agents page, reads an agent file
someone sent you. It accepts `.yaml`, `.yml` and `.json`.

What you get is **your own agent**: editable, deletable, and listed alongside the ones you built —
not the read-only kind you get from a project's shared folder. Details worth knowing:

- The imported agent gets a new id, so importing the same file twice gives you two independent
  agents and never overwrites one you already have.
- If the name is already taken, a number is added, so you can tell the two apart.
- A file that is missing a name, a description or a system prompt is refused, and the message tells
  you which of the three is absent. Nothing half-built lands in your list.
- MCP servers listed in the file come across as configuration. The tools each server provides are
  discovered when the agent runs, so they are not stored in the file.
- An imported agent starts with no local knowledge bases; agent files never contain them.

To hand an agent to someone else, **Download YAML** and send them the file.

---

## 9. The complete tool reference

Tools are the agent's abilities. This section covers every built-in one: what it does, when it is the
right choice, what it needs configured, and what to watch out for.

### 9.1 How to think about tools

```mermaid
flowchart TD
  Q{"What does the<br/>agent need to do?"}
  Q -->|"look at or change<br/>files on my computer"| FS["<b>File System</b><br/>readFiles, writeToFile,<br/>listFiles, applyDiffEdit,<br/>createFolder, moveFile, copyFile"]
  Q -->|"find things out<br/>from the internet"| W["<b>Web &amp; Search</b><br/>tavilySearch, fetchWebsite"]
  Q -->|"make a picture,<br/>video, or read an image"| AI["<b>AI Services</b><br/>generateImage, generateVideo,<br/>recognizeImage"]
  Q -->|"search documents and<br/>websites I gave it"| LKB["<b>Local knowledge base</b><br/>searchKnowledgeBase"]
  Q -->|"search an Amazon Bedrock<br/>Knowledge Base"| KB["<b>AI Services</b><br/>retrieve"]
  Q -->|"run a program,<br/>script, or command"| SYS["<b>System</b><br/>executeCommand, codeInterpreter,<br/>dockerSandbox"]
  Q -->|"see my screen<br/>or camera"| CAP["<b>System</b><br/>screenCapture, cameraCapture"]
  Q -->|"stop and reason,<br/>or track a long job"| TH["<b>Thinking</b><br/>think, todo"]
  Q -->|"remember a rule<br/>or a fact"| MEM["<b>Memory</b><br/>addMemory, updateMemory,<br/>removeMemory"]
  Q -->|"hand a step to a<br/>different agent"| DEL["<b>Agent Delegation</b><br/>invokeAgent"]
  Q -->|"use a tool this app<br/>does not have"| MCP["<b>MCP</b><br/>any MCP server"]
```

### 9.2 File System tools

These all operate inside your **project folder**.

| Tool            | What it does                                                                                                            | When it is the right choice                                                      |
| --------------- | ----------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `listFiles`     | Shows the folder structure as an indented tree, including subfolders. Respects your ignore patterns.                    | Getting oriented in an unfamiliar project. Almost always the agent's first move. |
| `readFiles`     | Reads several files at once. Handles text files, and converts Excel files (`.xlsx`, `.xls`) to CSV so it can read them. | Understanding existing content before changing it.                               |
| `writeToFile`   | Writes content to a file, creating it if it does not exist and replacing it if it does.                                 | Creating new files. Rewriting a file completely.                                 |
| `applyDiffEdit` | Changes one specific part of an existing file, leaving the rest untouched.                                              | Editing a large file. Safer and much cheaper than rewriting the whole thing.     |
| `createFolder`  | Makes a new folder.                                                                                                     | Setting up a project structure.                                                  |
| `moveFile`      | Moves or renames a file.                                                                                                | Reorganizing.                                                                    |
| `copyFile`      | Duplicates a file elsewhere.                                                                                            | Making a backup before a risky change, or templating from an existing file.      |

**Ignore patterns.** You can tell the app which files and folders to skip — build output, dependency
folders, secrets. There are two lists: a **global** one that applies everywhere, and a **project**
one saved in `.bedrock-engineer/.ignore` inside the project. Both use the same style of patterns as
`.gitignore`. This keeps `listFiles` readable and stops the agent wasting time and money reading
thousands of irrelevant files.

**Safety.** `writeToFile`, `applyDiffEdit`, `moveFile` and `createFolder` change your files without
asking. They are disabled in [Plan mode](#63-plan-mode-and-act-mode). If you are working on something
you care about, keep it in version control so you can always see and undo what the agent did.

### 9.3 Web & Search tools

| Tool           | What it does                                                                                                                                                                                                                                                              | Needs                                                                |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `tavilySearch` | Searches the web and returns summarized results.                                                                                                                                                                                                                          | A free [Tavily](https://tavily.com/) API key in **Settings → Chat**. |
| `fetchWebsite` | Fetches a specific URL. Long pages are split into numbered chunks: the first call gives an overview, then the agent asks for the chunks it needs. Supports GET, POST, PUT, DELETE, PATCH, HEAD and OPTIONS with custom headers and body, so it can also talk to web APIs. | Nothing.                                                             |

Use `tavilySearch` when the agent needs to _find_ something ("what is the current version of X"), and
`fetchWebsite` when it already knows _where_ to look.

Both send data out of your computer: your search words go to Tavily, and your request goes to
whichever site you name.

### 9.4 AI Services tools

These call Amazon Bedrock services and cost money per use, separate from the conversation itself.

| Tool                 | What it does                                                                                                                                                                                                                                                                                                                         | Notes                                                                                                                                                                                                                   |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `generateImage`      | Makes an image from a text description. Defaults to Stability SD 3.5 Large, and also supports Stability Stable Image Core and Stable Image Ultra. Choose the aspect ratio; output as PNG, JPEG or WebP. Supports a **negative prompt** (things to leave out) and a **seed** (the same seed and prompt gives the same picture again). | Only offered in `us-west-2`, the region where Bedrock offers these image models.                                                                                                                                        |
| `recognizeImage`     | Looks at an image file and describes it: objects, text in the picture, the scene, a caption.                                                                                                                                                                                                                                         | Good for pulling text out of screenshots, describing images for accessibility, or tagging a folder of photos.                                                                                                           |
| `generateVideo`      | Makes a video with Amazon Nova Reel, from text or from a starting image. Modes: `TEXT_VIDEO` (6 seconds), `MULTI_SHOT_AUTOMATED` (12–120 seconds), `MULTI_SHOT_MANUAL`.                                                                                                                                                              | Video takes minutes, so this returns a job reference immediately rather than waiting. Requires an S3 bucket configured.                                                                                                 |
| `checkVideoStatus`   | Checks whether a video job has finished.                                                                                                                                                                                                                                                                                             | Use the job reference from `generateVideo`.                                                                                                                                                                             |
| `downloadVideo`      | Downloads a finished video from S3 to your project folder.                                                                                                                                                                                                                                                                           | Only works once `checkVideoStatus` reports "Completed".                                                                                                                                                                 |
| `retrieve`           | Searches an Amazon Bedrock Knowledge Base — a collection of your own documents that AWS has indexed in your account.                                                                                                                                                                                                                 | Needs a knowledge base ID under **Tool Detail Settings** on the agent. For documents kept on your own computer, use a [local knowledge base](#24-local-knowledge-bases-letting-an-agent-search-your-documents) instead. |
| `invokeBedrockAgent` | Calls an Amazon Bedrock Agent you built in the AWS console, and returns its answer. Keeps a session ID so a conversation can continue across calls.                                                                                                                                                                                  | Needs the agent ID and alias ID on the agent config.                                                                                                                                                                    |
| `invokeFlow`         | Runs an Amazon Bedrock Flow — a data-processing pipeline you designed in AWS. Accepts text, numbers, true/false, objects and lists as input.                                                                                                                                                                                         | Needs the flow configured on the agent. Useful for multi-step processing you have already built elsewhere and do not want to rebuild in a prompt.                                                                       |

The image models used by `generateImage` (**Image Generation Model**) and `recognizeImage`
(**Recognition Model**), and the S3 bucket for video, are chosen in the **Tools** button below the
message box ([Section 6.10](#610-the-folder-tools-and-ignore-buttons)).

### 9.5 System tools

These are the powerful ones. Read the safety notes.

#### `executeCommand`

Runs a command on your computer, or sends typed input to a command that is already running.

**Safety:** only commands matching a pattern in the agent's **Allowed Commands** list can run.
Anything else is refused. This is your protection, and it is only as good as the patterns you write.
See the safety note in [Section 8.3](#83-the-agent-editor).

If the agent has the **Docker sandbox** tool as well, commands go into the sandbox container by
default instead of onto your machine, and the allowlist does not restrict them there — because
nothing in the container can reach your computer. Running on your actual machine then requires an
explicit request that pops up a dialog for you to approve. See
[Section 10](#10-the-docker-sandbox).

**When to use it:** running your project's tests, checking `git status`, starting a development
server, invoking a command-line tool you already trust. You can extend an agent enormously just by
allowing a command that queries a database or calls an internal API.

#### `codeInterpreter`

Runs Python code in a locked-down Docker container with data science libraries already installed.

- **No internet access** from inside, on purpose.
- Two environments: **basic** (numpy, pandas, matplotlib, requests) and **datascience** (the full
  stack — scikit-learn, scipy, seaborn and more). Defaults to datascience.
- Your input files can be mounted read-only at `/data/` for the code to analyze.
- Files the code produces — charts, CSVs — are detected automatically and reported back.

**When to use it:** analyzing a spreadsheet, making a chart, doing statistics, trying out a
calculation. This is the safest way to let an agent run code, because the code cannot reach the
internet or your files except the ones you hand it.

**Needs:** Docker installed and running.

#### `dockerSandbox`

Gives the chat its own long-lived Linux container to work in. It also brings the Sandbox tab, which
shows the container's state, services and published ports, logs every command the agent runs there, and
carries an interactive shell you can use yourself. Full explanation in
[Section 10](#10-the-docker-sandbox).

#### `screenCapture`

Takes a screenshot and saves it as a PNG. Can optionally have a vision model look at the screenshot
and describe it — extract the text, identify the buttons and fields, describe what is on screen.

**When to use it:** showing the agent an error dialog, having it read a chart you cannot copy text
from, documenting a user interface.

**Needs:** on macOS, Screen Recording permission in **System Settings → Privacy & Security**. The
agent config can also list specific windows it is allowed to capture.

#### `cameraCapture`

Takes a photo with your computer's camera and saves it, optionally having a model describe it.
Quality settings low/medium/high, formats JPG or PNG.

**Needs:** camera permission. The agent config can list which cameras are allowed.

### 9.6 Thinking tools

| Tool                                      | What it does                                                                                                                           | When it helps                                                                                                                                                         |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `think`                                   | Gives the model a scratchpad to reason in the middle of a task, without changing any data or fetching anything.                        | Multi-step problems, decisions with trade-offs, checking its own work against rules it was given. It costs a little and often saves a lot by preventing a wrong turn. |
| `todo` (with `todoInit` and `todoUpdate`) | Lets the agent write down a checklist for a long job and tick items off as it goes. You can watch the list on the chat's **Todo** tab. | Jobs with many steps, where you want to see progress and be sure nothing was skipped.                                                                                 |

These are different from the **Thinking** dropdown. The dropdown controls private reasoning before
the model starts answering; the `think` tool is something it can reach for partway through.

### 9.7 Memory tools

| Tool           | What it does                                                                                         |
| -------------- | ---------------------------------------------------------------------------------------------------- |
| `addMemory`    | Saves a new item to the shared memory list, as a **rule** (an instruction) or a **memory** (a fact). |
| `updateMemory` | Changes an item's text or kind, or switches it on or off.                                            |
| `removeMemory` | Deletes an item.                                                                                     |

These are what make "remember…", "add a rule…" and "forget…" work in a chat. They change app settings,
not your files, so they also work in Plan mode. Each use shows a note in the reply with **View** and,
where it makes sense, **Undo**. Full explanation in
[Section 25](#25-memory-rules-and-facts-every-chat-knows).

### 9.8 Agent Delegation

| Tool          | What it does                                                                                                                                                                                        |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `invokeAgent` | Hands one step of the work to a different agent of yours, which runs with its own system prompt and its own tools, and returns its result. This is also what an `@mention` in the message box uses. |

Full explanation in [Section 12](#12-handing-work-to-another-agent-with-).

### 9.9 MCP tools

Any tool provided by an MCP server you have connected shows up in the MCP category, named after the
server's own tool names. MCP tools are always on while their server is configured on the agent, and
are off in Plan mode. See [Section 11](#11-mcp-servers-adding-tools-from-outside-the-app).

### 9.10 Local knowledge base tools

An agent with at least one
[local knowledge base](#24-local-knowledge-bases-letting-an-agent-search-your-documents) gets these
three tools automatically, from a built-in MCP server called **local-knowledgebase**. There is nothing
to tick: they appear in the MCP category of the Tools tab as soon as the agent has a knowledge base,
and go away when it has none.

| Tool                       | What it does                                                                                                                                                                                                                                                                                                                                                                                                                            |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `searchKnowledgeBase`      | Searches the agent's knowledge bases and returns the most relevant passages, ranked by both meaning and keyword matches, each with its title, section heading, the knowledge base it came from and its source (file path or page URL). It takes a question or key terms, and optionally one knowledge base (by name) and how many passages to return (1–20; the default comes from **Settings → Knowledge Base → Results per search**). |
| `listKnowledgeBases`       | Lists the agent's knowledge bases with their descriptions and sizes.                                                                                                                                                                                                                                                                                                                                                                    |
| `listKnowledgeBaseSources` | Lists the files, folders and websites in the agent's knowledge bases, or in one of them.                                                                                                                                                                                                                                                                                                                                                |

**How to tell they are being used.** Each search shows up in the chat as a `searchKnowledgeBase` tool
call. Expand it to see the query the agent used and the passages it got back. The agent is asked to
cite the file or URL of the passages it uses, so a well-grounded answer names its sources. The
**System prompt** tab shows a `<local_knowledge_bases>` block listing each knowledge base by name and
description, with the instruction to search them before answering questions they may cover.

The same tools are available to the agent when it runs as a background task or as a sub-agent
through `@` or `invokeAgent`. They are not available in Plan mode, because MCP tools are off there.

### 9.11 Default tool sets

When you create an agent you can start from a preset (**Preset Category** on the Tools tab) rather
than ticking boxes one at a time. Every preset except Custom also turns on the three memory tools.

| Preset                      | Tools it turns on                                                                                                                          |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| **General Purpose**         | `readFiles`, `listFiles`, `tavilySearch`, `fetchWebsite`, `think`                                                                          |
| **Software Development**    | All seven file tools, plus `tavilySearch`, `executeCommand`, `invokeFlow`, `think`                                                         |
| **Design & Creative**       | `readFiles`, `listFiles`, `fetchWebsite`, `tavilySearch`, `generateImage`, `think`                                                         |
| **Data Analysis**           | `readFiles`, `listFiles`, `tavilySearch`, `fetchWebsite`, `retrieve`, `executeCommand`, `think`                                            |
| **Business & Productivity** | `readFiles`, `listFiles`, `tavilySearch`, `fetchWebsite`, `retrieve`, `think`                                                              |
| **All**                     | The file tools, `tavilySearch`, `fetchWebsite`, `retrieve`, `generateImage`, `invokeBedrockAgent`, `executeCommand`, `invokeFlow`, `think` |
| **Custom Configuration**    | Nothing — you choose                                                                                                                       |

**All** is not literally every tool: video, `recognizeImage`, `codeInterpreter`, `dockerSandbox`, the
capture tools, `todo` and `invokeAgent` are left for you to add. `generateImage` is only turned on by a
preset when your region is `us-west-2`, where the image models are.

---

## 10. The Docker sandbox

### 10.1 The problem it solves

An agent that can run commands is enormously more useful — it can install a library, run your tests,
try an idea. But an agent that can run commands _on your computer_ is also a risk, which is why the
allowed-commands list exists, and why that list keeps the agent boxed in.

The Docker sandbox removes the trade-off. It gives the chat its own Linux computer to make a mess in.

### 10.2 How it works

Turn on the **Docker Sandbox** tool for an agent. Then, the first time that agent runs a command in
a chat, the app creates a container for that chat on the operating system chosen in **Settings →
Sandbox**
([Section 5.6](#56-sandbox-tab)), and from then on commands go there by default. The choices are Debian 13 (the default),
Ubuntu 26.04, Amazon Linux 2023 and 2027, AlmaLinux 9, Rocky Linux 9 and Red Hat UBI 9 (real RHEL,
limited to Red Hat's freely available packages). Changing it affects new sandboxes; a chat that
already has one keeps its OS until you remove the sandbox. The agent can also ask for a different OS
when it creates a sandbox.

```mermaid
flowchart LR
  subgraph Your["Your computer"]
    P["Project folder"]
    D["docker-sandboxes/&lt;chat&gt;/"]
    H["Everything else on<br/>your machine"]
  end
  subgraph C["The chat's container (Debian 13 by default)"]
    WS["/workspace<br/>(your project, read+write)"]
    DATA["/data<br/>(saved on your machine)"]
    PKG["Anything the agent<br/>installs — apt/dnf, pip, npm"]
  end
  P <--> WS
  D <--> DATA
  H -.->|"blocked — needs your<br/>explicit approval"| C
```

- Your project folder is mounted read-write at `/workspace`, so files move in and out freely.
- Data written to `/data` is kept on your machine, so it survives the container.
- The agent can install anything it wants. Because the container cannot reach your computer, the
  allowed-commands list does not restrict it inside there.
- The agent can publish ports, so if it builds a web page or starts a server you can open it in your
  browser.
- Long-running processes can be started in the background and their output read back later.

### 10.3 Running something on your actual computer

The agent can still ask to run a command on your machine, but it has to say so explicitly. When it
does, **you get a dialog showing the exact command before anything runs.** You can allow it once, or
allow that pattern for the rest of the chat.

### 10.4 Managing sandboxes

A **whale icon** appears above the message box whenever the current chat has a container. Its menu
shows the sandbox's state and can:

- **Show the Sandbox tab** (see [10.8](#108-the-sandbox-tab))
- **Open sandbox folder** in Finder, File Explorer, or your Linux file manager
- **Stop containers** or **Start containers**
- **Remove sandbox (keep data)**, or **Remove sandbox and delete data**

Chats that have a sandbox also show a small whale next to their title in the chat history.

Containers survive switching chats. They are stopped when you quit the app and come back with their
installed packages intact when you return.

### 10.5 Where the files live

```
<your project folder>/docker-sandboxes/<chat-title>-<id>/
```

Ordinary mapped folders and a plain Docker Compose file, not hidden Docker volumes, so you can just
look at them. The folder is named after the chat and is renamed automatically when the chat title
changes — which happens often, because a sandbox usually exists before the chat has a real title.
Renaming only moves the folder; containers keep running, because Docker tracks them by an internal ID
that never moves.

### 10.6 Cleaning up

Deleting a chat removes its container. The delete dialog also offers to delete the sandbox's data
folder — **unchecked by default**, so anything the agent wrote is kept unless you deliberately say
otherwise.

### 10.7 Requirements and limits

- **Docker must be installed and running.** If it is missing, the agent tells you how to install it
  for your platform. **Settings → Sandbox** shows whether the app can see Docker.
- **Each container is limited** to the memory and CPU set in **Settings → Sandbox** (2 GB and 2 CPUs
  by default), and the agent waits up to the command timeout (5 minutes by default) for each command
  before moving on. New limits apply when a sandbox is recreated.
- **Docker Compose** is used when available, and is required for stacks with more than one service.
- **The terminal needs Docker to be local.** Docker Desktop, OrbStack, Colima, Rancher Desktop and
  rootless installs all work. If your Docker context points at a remote daemon, the Terminal tab is
  disabled and explains why; everything else on the Sandbox tab still works.

### 10.8 The Sandbox tab

The **Sandbox** tab appears in the chat's tab strip ([6.6](#66-the-chat-tabs)) for chats that have a
container, marked with a fuchsia dot until you first look. Open it from the tab strip or from the
whale menu's **Show the Sandbox tab**. Its header shows the project and a state: **running**, **partly
running**, **stopped** or **missing**.

It has up to four tabs of its own:

**Overview** — what the container is and what it is doing: its name and image, when it started, the
workspace and folder, and CPU, memory, network and disk against the limits from
[Settings → Sandbox](#56-sandbox-tab). Each service is listed with its state, and each published port
is a button that opens `localhost:<port>` in your browser. The usual actions — open folder, start,
stop, restart, remove (keeping or deleting the data) — are at the bottom.

CPU is shown the way `docker stats` shows it: 100% means one core fully used, so a container working
across two cores reads 200%. It says _measuring…_ for the first few seconds after you open the tab,
because a percentage needs two readings to compare.

**Compose** — only for sandboxes that use Docker Compose. A diagram of the stack: which services
exist, what image each runs, whether it is up, which published ports reach it from your browser
(solid arrows) and which folders are mounted into it (dashed). Services in one stack can reach each
other by service name, which the diagram notes, because that is the detail people most often miss.
Click the diagram to open it full window, with zoom controls and Esc to close — worth it for a stack
of several services. Below the diagram is the compose file
itself, exactly as written to disk, with buttons to copy it or open its folder.

**Terminal** — a real shell in the container. See [10.9](#109-the-interactive-terminal).

**Activity** — every command the agent has run in this sandbox, newest first, with the time it
started, how long it took, and how it ended:

| Outcome                     | Meaning                                                                                                        |
| --------------------------- | -------------------------------------------------------------------------------------------------------------- |
| **exit 0** / **exit 1** …   | The command finished, with that exit code.                                                                     |
| **waiting for input**       | It stopped at a prompt. The agent can answer it; the row shows how many bytes were sent, never what they were. |
| **detached**                | It looked like a server that had come up, so the agent stopped waiting. It is still running.                   |
| **timed out, left running** | It outlived the per-command timeout. The agent moved on and the process was deliberately **not** killed.       |

Rows are tagged **agent** or **you**, so a command the model ran is never confused with one you typed.
Opening and closing a terminal shows up as a **you** row; what you type inside it does not, because
reading commands back out of a live shell is not something that can be done reliably.

The log is kept in the sandbox's own folder, so it survives quitting the app, and the newest 500
entries are retained. Removing the sandbox removes the log with it.

### 10.9 The interactive terminal

The **Terminal** tab gives you a shell inside the chat's container — the same container the agent has
been working in. A stack with more than one service gets a row of tabs, one per container, each with
its own shell and its own scrollback; a dot shows which are running. It is a proper terminal: colours, `vim` and `htop`,
tab completion, command history, Ctrl-C, and it resizes with the window.

This is the quickest way to see what the agent actually left behind, or to fix something yourself
without describing it to the model first.

**Read this once, then it will stop telling you:**

- `/workspace` is your real project folder, mounted read-write. A `rm -rf` typed here deletes real
  files. There is no undo and no confirmation.
- Nothing you type is checked against the allowed-commands list. That list exists to protect your
  machine from the model; this shell is you, in a container, on purpose.
- The shell runs as **root**, so files it creates may end up owned by root on Linux.

The app asks you to acknowledge that once (**I understand — open the terminal**), before the first
time you open a terminal, and then remembers it.

**The model cannot use this.** There is no tool for it, so agents, sub-agents and background tasks
have no way to open a shell or type into yours. It is reachable only from this tab.

A shell you open keeps running while you look at other tabs or switch chats — so a dev server you
started by hand does not die when you look away. Switching between service tabs is free for the same
reason: the shell lives outside the tab, and its recent output is replayed when you come back. It ends when the sandbox is stopped
or removed, when the chat is deleted, or when you quit the app.

---

## 11. MCP servers: adding tools from outside the app

### 11.1 What MCP is

**MCP** stands for Model Context Protocol. It is an open standard for programs that expose tools to
an AI. Someone writes an MCP server for, say, Jira or Postgres or Google Drive, and any app that
speaks MCP — including this one — can use those tools.

This is how you give an agent an ability the app does not ship with, without anyone changing the
app's code.

```mermaid
flowchart LR
  Agent["Your agent"] --> App["This app"]
  App -->|"MCP"| S1["MCP server:<br/>your ticket system"]
  App -->|"MCP"| S2["MCP server:<br/>your database"]
  App -->|"MCP"| S3["MCP server:<br/>anything else"]
  S1 --> T1["its tools appear<br/>in the Tools tab"]
  S2 --> T1
  S3 --> T1
```

MCP servers are configured **per agent**, on the **MCP Servers** tab of the agent editor. Two kinds
of connection are supported: a **command** the app starts on your computer (called stdio), or a
**URL** for a server running elsewhere (SSE or streamable HTTP).

For the exact configuration format, see the
[MCP Server Configuration Guide](./mcp-server/MCP_SERVER_CONFIGURATION.md).

One MCP server is built into the app: **local-knowledgebase**, which an agent gets automatically when
it has [local knowledge bases](#24-local-knowledge-bases-letting-an-agent-search-your-documents). It
runs inside the app, needs no configuration, and appears on the MCP Servers tab as a read-only card
marked **Built-in**.

### 11.2 Finding servers

![mcp-market](../assets/mcp-market.png)

The MCP Servers tab searches the [official MCP Registry](https://registry.modelcontextprotocol.io).
Type a term, or press **Suggest for this agent** to have the model work out the search terms from
this agent's own description, system prompt, scenarios, allowed commands and enabled tools.

Everything listed comes from the real registry, so names, versions, package identifiers, required
environment variables and hosted endpoints are real. **Load config** fills the JSON editor with a
version-pinned command (or the remote URL) for you to review before you add it. The model only ever
produces the _search terms_ — never a package name — so it cannot invent a package that does not
exist.

The suggested terms have to be grounded in the agent's own configuration, and each one is shown next
to the phrase it came from. A support agent whose prompt mentions Zendesk, Stripe, Snowflake, Linear
and Slack gets exactly those five searches — not vague words like "automation" or "devops". Generic
category words are rejected, and if nothing in the configuration names a real system, it says so
instead of guessing.

[MCP Market](https://mcpmarket.com) is also linked, for browsing by category. That is a link only —
the app does not read from it, because the site publishes no API and blocks automated requests.

### 11.3 Testing a connection

The MCP Servers tab has a connection test. Run it after adding a server. If it fails, the usual
causes are a missing environment variable (an API key the server needs), a package that could not be
downloaded, or a URL that requires authentication.

Once a server connects, switch to the **Tools** tab and its tools are listed in the MCP category.
MCP tools are always enabled while the server is configured; to stop an agent using a server's tools,
remove the server.

The built-in **local-knowledgebase** card has its own **Test connection** button, which should report
three tools.

### 11.4 Using the app's AWS settings in a server

Many AWS MCP servers need AWS credentials. Instead of typing a profile name or keys into the
server's `env`, use these two placeholders. The app fills them in from **Settings → AWS** each time
the server starts:

| Value in `env`   | Becomes                                                                  |
| ---------------- | ------------------------------------------------------------------------ |
| `{{awsRegion}}`  | The region set in the app                                                |
| `{{awsProfile}}` | The profile name, when the app signs in with a named profile (see below) |

```json
"env": {
  "AWS_PROFILE": "{{awsProfile}}",
  "AWS_REGION": "{{awsRegion}}"
}
```

If the app signs in with an access key instead of a profile, the `AWS_PROFILE` entry is left out
and the server gets the access key, secret key and session token in its place. Values the
placeholders produce take priority over any AWS variables already in the app's environment.
Changing your AWS settings reconnects the servers that use them the next time you send a message.

Only servers whose `env` uses a placeholder get your credentials. No other MCP server is given them.

### 11.5 The AWS Solutions Architect agent

The **AWS Solutions Architect** agent in the [Agent Directory](#13-agent-directory) comes with four
AWS MCP servers already set up. Add it to My Agents to use it, and change its prompt, tools or
servers like any of your own agents. It answers from the official AWS documentation and links each
point to the page it came from:

| Server              | What it provides                                                                                         |
| ------------------- | -------------------------------------------------------------------------------------------------------- |
| `aws-documentation` | Search, read and get related pages from the AWS documentation                                            |
| `aws-knowledge`     | AWS's hosted knowledge server: documentation, which Regions offer a service or feature, AWS agent skills |
| `aws-iac`           | Validate CloudFormation templates, check them for compliance, troubleshoot failed deployments, CDK docs  |
| `aws-pricing`       | Prices from the AWS Pricing API, for cost estimates                                                      |

The `aws-iac` and `aws-pricing` servers use your AWS settings through the placeholders in
[Section 11.4](#114-using-the-apps-aws-settings-in-a-server). `aws-iac` only reads and validates, and
it never deploys anything. The pricing server needs read access to the AWS Pricing API, and the
IaC server's deployment troubleshooting reads CloudFormation and CloudTrail.

Three of the servers are started with `uvx`, so install [uv](https://docs.astral.sh/uv/) first. The
first start downloads each server and can take a minute. If a server's tools are missing after
that, run the connection test on the agent's **MCP Servers** tab.

---

## 12. Handing work to another agent with `@`

![agent-mention](../assets/agent-mention.png)

Type `@` followed by an agent's name in the message box and pick from the list. That agent will
handle that part of the task, using **its own** system prompt and **its own** tools, and hand the
result back to the agent you are talking to.

Example:

> use @email to find the email from Kevin, then draft a reply for me to review

You stay in one conversation. There is no switching back and forth, and no copying results between
chats.

**Why this is useful:** it lets you build small, focused, well-behaved agents instead of one giant
agent that has every tool and a system prompt trying to cover every situation. A researcher agent, a
writer agent and an email agent, each good at one thing, work better together than one agent with all
their tools bolted on.

This is the same mechanism as the `invokeAgent` tool, so an agent can also delegate on its own
initiative if you give it that tool and mention in its system prompt when it should.

---

## 13. Agent Directory

![agent-directory](../assets/agent-directory.png)

A collection of ready-made agents you can add to your own list with one click.

- **Browse or search** the collection, or filter by tag.
- **View details** — the full system prompt, the tools it uses, its example scenarios, and who wrote
  it. Read the system prompt before adding an agent: it is the whole personality, and it is worth
  understanding what you are installing.
- **Add to My Agents** copies it into your own list, where you can edit it freely.

### 13.1 Sharing inside your organization

You can host a private collection of agents for your team in an AWS S3 bucket. Team members point the
app at that bucket and see your agents alongside the public ones. Setup instructions:
[Organization Sharing Guide](./agent-directory-organization/README.md).

### 13.2 Contributing

To share an agent publicly: add your GitHub username as the author, use **Download YAML** from the
agent's **⋮** menu to get the file, and submit that file as a pull request or issue to the upstream
project.

---

## 14. Background Agent: work on a schedule

![background-agent](../assets/background-agent.png)

Set an agent to run by itself, on a repeating schedule, without you being there.

Good uses: a morning summary of overnight changes, a nightly check that a build still works, a weekly
report pulled together from several sources.

### 14.1 Creating a task

Open **Background Agent** in the sidebar and click **Create Task**.

| Field                           | What to put in it                                                                                                                                                             |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Task Name**                   | Something you will recognize in the list.                                                                                                                                     |
| **Schedule**                    | When to run, as a cron expression. Presets are provided.                                                                                                                      |
| **Agent**                       | Which agent runs. It uses that agent's tools, including its MCP servers and local knowledge bases.                                                                            |
| **Project Directory**           | Optional. Which folder it works in.                                                                                                                                           |
| **Model**                       | Which model it uses. Scheduled tasks are a good place to use a cheaper model.                                                                                                 |
| **Max Output Tokens**           | The longest reply allowed.                                                                                                                                                    |
| **Wake Word (Prompt)**          | The message sent to the agent when the task runs. This is the actual instruction — for example _"Summarize commits from the last 24 hours and write it to daily-summary.md"_. |
| **Continue Session**            | Whether each run continues the same conversation or starts fresh.                                                                                                             |
| **Session Continuation Prompt** | Shown when Continue Session is on: the message used on continuing runs. Leave it empty to reuse the wake word.                                                                |
| **Enable task immediately**     | Start the schedule as soon as the task is saved. You can enable or disable it later without deleting it.                                                                      |

Name, schedule, agent, model and wake word are required.

### 14.2 Cron expressions, briefly

A cron expression is five values: `minute hour day month day-of-week`. `*` means "every".

| Expression    | Means                             |
| ------------- | --------------------------------- |
| `0 9 * * *`   | Every day at 9:00 am              |
| `0 9 * * 1-5` | Weekdays at 9:00 am               |
| `0 9 * * 1`   | Mondays at 9:00 am                |
| `0 * * * *`   | Every hour, on the hour           |
| `*/5 * * * *` | Every 5 minutes                   |
| `0 9 1 * *`   | The 1st of every month at 9:00 am |

The app provides presets — every minute, every 5 minutes, every hour, daily at 9 AM, weekdays at
9 AM, weekly on Monday at 9 AM, monthly on the 1st at 9 AM — so you rarely need to write one by hand.

### 14.3 Watching and running tasks

Each task in the **Scheduled Tasks** list shows its wake word, last and next run, and the last error if
there was one. Its buttons:

- **Test Execution** runs the task immediately, without waiting for the schedule. Use this to test.
- **Enable task** / **Disable task**, **Edit Task** and **Delete Task**.
- **Execution History** opens a separate window with every past run, what it did, how long it
  took and its estimated Bedrock cost. You can filter it by outcome (all, success only, failure only) and by date, and **Continue
  Conversation** sends a follow-up instruction into that session.
- **Show system prompt** shows exactly what the agent is told when the task runs.

**Notifications** (if **Enable desktop notifications** is on in Settings → General) tell you when a
scheduled run finishes or fails — but only while the app window is not in front, since otherwise you
can see it already. Clicking the notification opens that task's history.

Scheduled tasks only run while the app is open.

### 14.4 Continue Session, explained

- **Off** — every run starts with a blank memory. Best for independent jobs: "check X and report".
- **On** — runs build on each other, so the agent remembers what it found last time. Best for
  ongoing work: "keep working through the list". Be aware that a session that continues forever gets
  longer and more expensive each run.

---

## 15. Website Generator

![website-gen](../assets/website-generator.png)

Describe a web page in words and watch the code get written with a live preview beside it. Keep
giving instructions to refine it.

**Frameworks:** React (with TypeScript), Vue (with TypeScript), Svelte, or plain JavaScript.

**Styling:** inline styles, Tailwind CSS, or Material UI (React only).

### 15.1 Web search

Click **Search** at the bottom to let the generator look things up on the web while it works —
current library versions, design patterns, best practices. Turning this on generally produces better,
more current code.

### 15.2 Using your own design system

Click **Connect** at the bottom and enter an Amazon Bedrock Knowledge Base ID. The generator will
then reference whatever is in that knowledge base — your design system, your existing source code,
your house style.

To set this up, you need to have loaded that material into a knowledge base beforehand. Source code
works better if you flatten it into a single readable file first (tools like
[gpt-repository-loader](https://github.com/mpoon/gpt-repository-loader) do this). Figma designs can
be referenced by exporting them as HTML and CSS and adding that.

---

## 16. Diagram Generator

![diagram-generator](../assets/diagram-generator.png)

Describe a system in ordinary language and get an AWS architecture diagram back, drawn with the
official AWS icons.

- Diagrams are produced as **draw.io compatible XML**, so you can open and edit them in draw.io
  afterwards.
- **Web search** can be switched on so the diagram reflects current AWS services and patterns.
- **History** keeps your past diagrams so you can go back and iterate.
- The generator can also **suggest improvements** to a diagram you have made.

---

## 17. Step Functions Generator

![step-functions-generator](../assets/step-functions-generator.png)

Describe a workflow and get an AWS Step Functions state machine definition (ASL — Amazon States
Language) back, with a live visual preview of the flow as you refine it.

Useful when you know what the steps are but not the exact JSON syntax for expressing them.

---

## 18. Choosing a model

### 18.1 The short answer

| If you want…                                             | Use                                                          |
| -------------------------------------------------------- | ------------------------------------------------------------ |
| A good all-round default                                 | **Claude Sonnet 5.5**                                        |
| The strongest reasoning for hard problems                | **Claude Opus 5.5** or **Claude Fable 5.1**                  |
| Speed and low cost for simple work                       | **Claude Haiku 5.5** or **Amazon Nova Lite**                 |
| The cheapest option for background tasks and chat titles | **Amazon Nova Micro**, **Nova Lite** or **Claude Haiku 5.5** |
| A very large context window for huge documents           | **Claude Fable 5.1** (1M tokens) or **Grok 4.7** (500K)      |
| OpenAI's most capable model, cost no object              | **GPT-6 Astra** (1.05M tokens, $11/$55 per 1M)               |
| Near-Astra quality at a mid-tier OpenAI price            | **GPT-6.1 Sol** (1M tokens, $2.20/$11 per 1M)                |
| A strong, low-cost OpenAI model for agent work           | **GPT-6.1 Terra** (1.05M tokens, $1.10/$5.50 per 1M)         |
| A mid-priced OpenAI model for everyday work              | **GPT-6 Sol** ($2/$10 per 1M)                                |
| The cheapest OpenAI model for high-volume work           | **GPT-6 Luna** ($0.10/$0.50 per 1M)                          |

The model dropdown shows the price per million tokens in and out for each model, so you can compare
before you commit.

### 18.2 The full list

Which of these you see depends on your region and your **Visible Models** allowlist.

**Anthropic (Claude):** Haiku 4.5, Haiku 5.5, Sonnet 4.6, Sonnet 5, Sonnet 5.5, Opus 4.6, Opus 4.7,
Opus 4.8, Opus 5, Opus 5.5, Fable 5, Fable 5.1

**Amazon (Nova):** Nova Micro, Nova Lite, Nova 2 Lite, Nova Pro

**OpenAI:** GPT-6 Astra, GPT-6.1 Sol, GPT-6.1 Terra, GPT-6 Sol, GPT-6 Luna, GPT-5.6 Sol, GPT-5.6 Terra, GPT-5.6 Luna,
GPT-OSS 20B, GPT-OSS 120B

**Others:** DeepSeek R1, Kimi K3, Kimi K2.5, Grok 4.6, Grok 4.7

**Image models** (used by `generateImage`, `us-west-2` only): Stability SD3.5 Large, Stable Image
Core v1.0, Stable Image Ultra v1.0

### 18.3 Routing: why some models appear twice

Amazon Bedrock offers several ways to route a request, and the same model may appear more than once
in the dropdown with a different suffix:

| Routing            | What it means                                                                                                   |
| ------------------ | --------------------------------------------------------------------------------------------------------------- |
| **Global**         | AWS picks whichever commercial region has capacity. Usually the most reliable and least likely to be throttled. |
| **US / EU / APAC** | Stay within that geography, spreading across its regions. Use when you have a data residency requirement.       |
| **Japan**          | Stay within Japan (Tokyo and Osaka).                                                                            |
| _(no suffix)_      | Run in exactly the region you selected, and nowhere else.                                                       |

If you keep hitting capacity errors, try a Global option, or turn on
[Region Failover](#53-models-tab).

### 18.4 Prompt caching

Most conversations repeat a lot of unchanging text every turn — the system prompt, the tool list, the
earlier messages. Prompt caching tells AWS to remember that part so it does not have to be
re-processed each time. Replies come back faster and cost less.

It is on by default (**Settings → Chat → Enable Prompt Cache**) and there is rarely a reason to turn
it off. Some models handle caching automatically and ignore the setting entirely.

### 18.5 Managing cost

Four things actually move the needle:

1. **Set Context Length sensibly.** Every past message is re-sent and re-charged each turn.
2. **Set a cheap Light Processing Model.** By default it is your main model. Chat titles, suggestions
   and the Help chat do not need your best model.
3. **Give agents fewer tools.** The tool list is sent with every request.
4. **Turn thinking down for simple work.** Reasoning tokens are billed.

Watch the chat's **Cost** tab ([6.6](#66-the-chat-tabs)) to see where your money is going. Its label
is the running cost of the conversation.

For the whole app — every chat, generator, background agent, sub-agent and knowledge base, month by
month — open the **Accounting** page, and set a monthly budget there to be warned before a month
gets expensive ([Section 26](#26-accounting-what-bedrock-is-costing-you)).

Knowledge bases built with Bedrock embeddings also cost a little each time you add or re-sync a source,
and each search embeds the question. The on-device model costs nothing to run. See
[Section 24.8](#248-costs-permissions-and-privacy).

---

## 19. Where the app keeps your files

### 19.1 App settings and chat history

On a Mac:

```
~/Library/Application Support/Bedrock Engineer/
  config.json              ← all your settings
  chat-sessions/           ← your conversations
  chat-sessions-meta.json  ← the index of conversations
  window-state.json        ← the window's size and position, so it reopens where you left it
  memory.json              ← your memory items (rules and facts)
  usage/usage.db           ← the cost ledger behind the Accounting page (every Bedrock call)
  knowledgebases/
    <agent id>/<kb id>.db  ← one file per local knowledge base
    .models/               ← the on-device embedding model, once downloaded
```

On Windows the folder is `%APPDATA%\Bedrock Engineer\`, and on Linux `~/.config/Bedrock Engineer/`.
The exact path for your machine is shown in **Settings → Workspace → Config Directory**, and the
knowledge base folder and its total size at the top of **Settings → Knowledge Base**.

Note the folder is named after the app's display name (`Bedrock Engineer`), not
`bedrock-engineer`. If someone gives you a support instruction referring to a `bedrock-engineer`
folder, this is the folder they mean. Builds of this fork from before the rename used a different
folder name; the app moves that folder here the first time it starts.

### 19.2 Inside your project folder

```
<your project folder>/
  attachments/<chat>-<id>/       ← files you attached to a chat
  docker-sandboxes/<chat>-<id>/  ← compose file and mapped data for a chat's container
  .bedrock-engineer/
    agents/                      ← agents shared with this project
    rules/                       ← project-specific rules for agents to follow
    .ignore                      ← files and folders to skip in this project
```

Everything under `.bedrock-engineer/` is meant to be committed to version control alongside your
code, so a whole team gets the same agents and rules.

### 19.3 Logs

The app writes rotating daily log files. If you are reporting a problem, these are what to include.
Their location is alongside the config directory.

### 19.4 What leaves your computer

Worth being clear about:

| What                                                              | Goes where                                                                                                                                                                                                    |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Your messages, and the file contents the agent reads              | Amazon Bedrock, in your AWS account and chosen region                                                                                                                                                         |
| Web search terms                                                  | Tavily                                                                                                                                                                                                        |
| A URL you ask it to fetch                                         | That website                                                                                                                                                                                                  |
| Image, video and Bedrock Knowledge Base requests                  | Amazon Bedrock / your AWS account                                                                                                                                                                             |
| Local knowledge base text, when it uses a Bedrock embedding model | Amazon Bedrock, to be turned into vectors: every passage when you add or re-sync a source, and the question for each search. Nothing is stored there. With the on-device model, nothing leaves your computer. |
| Website sources you add to a knowledge base                       | The app's crawler fetches pages from that site                                                                                                                                                                |
| The on-device embedding model, if you download it                 | Downloaded once from Hugging Face                                                                                                                                                                             |
| Update checks and downloads                                       | GitHub, to see whether a newer release of this fork exists and to download it                                                                                                                                 |
| Anything an MCP server does                                       | Wherever that MCP server sends it — check before you install one                                                                                                                                              |
| Everything else                                                   | Nowhere. The app has no telemetry and no server of its own. Memory items, chat history and knowledge bases stay on your computer.                                                                             |

Your files are not uploaded wholesale. Only the parts an agent actually reads get sent, as part of
the conversation.

---

## 20. Question index: "How do I…"

**Getting started**

- _…ask a question about the app without reading this guide?_ → The Help button,
  [Section 23](#23-getting-help-inside-the-app)
- _…install the app?_ → [Step 1](#step-1--install-the-app)
- _…get past the macOS security warning?_ → [Step 1](#step-1--install-the-app)
- _…connect it to AWS?_ → [Section 4](#4-setting-up-access-to-amazon-bedrock)
- _…enable the models in my AWS account?_ →
  [Section 4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console)
- _…know what AWS permissions I need?_ → [Section 4.10](#410-what-permissions-does-my-aws-user-need)
- _…create an AWS access key?_ → [Section 4.4](#44-step-3--create-aws-access-keys)
- _…install the AWS CLI?_ → [Section 4.5](#45-step-4--install-the-aws-cli)
- _…set up the `~/.aws/credentials` and `~/.aws/config` files?_ →
  [Section 4.6](#46-step-5--put-your-keys-in-the-aws-credentials-and-config-files)
- _…use an AWS profile or single sign-on instead of pasting keys?_ →
  [Section 4.6](#46-step-5--put-your-keys-in-the-aws-credentials-and-config-files)
- _…check whether my AWS setup actually works?_ → [Section 4.8](#48-check-that-it-works)
- _…make it work behind my company's proxy?_ → [Section 4.11](#411-if-your-office-uses-a-proxy)

**Chatting**

- _…change which model answers?_ → [Section 6.2](#62-the-three-main-controls)
- _…stop the agent from changing my files?_ → Plan mode, [Section 6.3](#63-plan-mode-and-act-mode)
- _…make the agent think harder?_ → [Section 6.2](#62-the-three-main-controls)
- _…see what a tool actually did?_ → Expand the tool call in the conversation,
  [Section 6.4](#64-sending-a-message)
- _…keep an answer running while I do something else?_ → It already does,
  [Section 6.5](#65-answers-keep-running-when-you-look-away)
- _…see what this conversation is costing me?_ → the Cost tab, [Section 6.6](#66-the-chat-tabs)
- _…see the exact system prompt the model gets?_ → the System prompt tab, [Section 6.6](#66-the-chat-tabs)
- _…open a terminal on my own computer?_ → [Section 6.12](#612-the-terminal-tab)
- _…collapse the chat history?_ → [Section 6.9](#69-chat-history)
- _…save a conversation as a document?_ → [Section 6.7](#67-exporting-a-conversation)
- _…copy just one paragraph out of a reply?_ → [Section 6.8](#68-copying-part-of-a-message)
- _…delete a lot of old chats at once?_ → [Section 6.9](#69-chat-history)
- _…change whether Enter sends or makes a new line?_ → [Section 5.4](#54-chat-tab)

**Files**

- _…give the agent a file to look at?_ → [Section 7](#7-attachments-giving-the-agent-your-files)
- _…update an attached file without re-attaching it?_ → Just edit it,
  [Section 7.3](#73-how-the-agent-sees-them)
- _…stop the agent reading my `node_modules` folder?_ → Ignore patterns,
  [Section 9.2](#92-file-system-tools)
- _…see and open the files the agent made?_ → the Files tab, [Section 6.11](#611-the-files-tab)
- _…edit a file without leaving the app?_ → [Section 6.11](#611-the-files-tab)
- _…open a file in its own window?_ → Pop out, [Section 6.11](#611-the-files-tab)
- _…make the file list wider?_ → drag the divider, [Section 6.11](#611-the-files-tab)
- _…find where my chat history is stored?_ → [Section 19.1](#191-app-settings-and-chat-history)

**Agents**

- _…make my own agent?_ → [Section 8.2](#82-the-my-agents-page)
- _…write a good system prompt?_ → [Section 8.4](#84-writing-a-good-system-prompt)
- _…get rid of a built-in agent I never use?_ → Hide it, [Section 8.2](#82-the-my-agents-page)
- _…bring back an agent I hid?_ → The Unhide dropdown, [Section 8.2](#82-the-my-agents-page)
- _…change an agent's icon?_ → [Section 8.3](#83-the-agent-editor)
- _…share an agent with my team?_ → [Section 8.5](#85-sharing-an-agent)
- _…send an agent to someone as a file?_ → [Section 8.5](#85-sharing-an-agent)
- _…use an agent file someone sent me?_ → [Section 8.6](#86-importing-an-agent)
- _…stop sharing an agent I shared into a project?_ → [Section 8.5](#85-sharing-an-agent)
- _…reorder my agents?_ → Drag them, [Section 8.2](#82-the-my-agents-page)
- _…use someone else's agent?_ → [Section 13](#13-agent-directory)
- _…have one agent ask another for help?_ → [Section 12](#12-handing-work-to-another-agent-with-)
- _…let an agent search my own documents or a website?_ → a local knowledge base,
  [Section 24](#24-local-knowledge-bases-letting-an-agent-search-your-documents)
- _…check whether the agent actually searched its knowledge base?_ →
  [Section 24.4](#244-how-the-agent-uses-a-knowledge-base)
- _…make the assistant remember something in every chat?_ →
  [Section 25](#25-memory-rules-and-facts-every-chat-knows)

**Tools**

- _…let the agent search the web?_ → `tavilySearch` plus a Tavily key,
  [Section 9.3](#93-web--search-tools)
- _…let the agent run commands safely?_ → [Section 10](#10-the-docker-sandbox)
- _…see what the agent is doing inside its container?_ → the Sandbox tab,
  [Section 10.8](#108-the-sandbox-tab)
- _…get a shell inside the container myself?_ → [Section 10.9](#109-the-interactive-terminal)
- _…let the agent run commands on my actual machine?_ → Allowed Commands,
  [Section 8.3](#83-the-agent-editor) — and read the safety note
- _…choose the sandbox's operating system or limits?_ → [Section 5.6](#56-sandbox-tab)
- _…let the agent analyze a spreadsheet?_ → `codeInterpreter`, [Section 9.5](#95-system-tools)
- _…let the agent search my company's documents?_ → a local knowledge base
  ([Section 24](#24-local-knowledge-bases-letting-an-agent-search-your-documents)), or `retrieve`
  with an Amazon Bedrock Knowledge Base ([Section 9.4](#94-ai-services-tools))
- _…make an image?_ → `generateImage`, [Section 9.4](#94-ai-services-tools)
- _…make a video?_ → `generateVideo`, [Section 9.4](#94-ai-services-tools)
- _…show the agent my screen?_ → `screenCapture`, [Section 9.5](#95-system-tools)
- _…add a tool the app does not have?_ → MCP,
  [Section 11](#11-mcp-servers-adding-tools-from-outside-the-app)
- _…find an MCP server for a service I use?_ → [Section 11.2](#112-finding-servers)
- _…let an MCP server use my AWS credentials?_ → [Section 11.4](#114-using-the-apps-aws-settings-in-a-server)
- _…get answers from the AWS documentation with source links?_ → [Section 11.5](#115-the-aws-solutions-architect-agent)

**Automation**

- _…run something every morning?_ → [Section 14](#14-background-agent-work-on-a-schedule)
- _…write a cron schedule?_ → [Section 14.2](#142-cron-expressions-briefly)
- _…see whether a scheduled task worked?_ → [Section 14.3](#143-watching-and-running-tasks)

**Building things**

- _…build a web page?_ → [Section 15](#15-website-generator)
- _…draw an AWS architecture diagram?_ → [Section 16](#16-diagram-generator)
- _…build a Step Functions workflow?_ → [Section 17](#17-step-functions-generator)

**Updates**

- _…update the app?_ → it updates itself, [Section 5.8](#58-updates-tab)
- _…see what changed in this version?_ → [Section 5.8](#58-updates-tab)

**Cost and models**

- _…spend less money?_ → [Section 18.5](#185-managing-cost)
- _…see what I've spent this month, or in an earlier month?_ → the Accounting page,
  [Section 26](#26-accounting-what-bedrock-is-costing-you)
- _…get warned before I spend too much?_ → a monthly budget,
  [Section 26.4](#264-a-monthly-budget)
- _…see which model, agent or feature costs the most?_ →
  [Section 26.2](#262-reading-the-page)
- _…hide models I never use?_ → Visible Models, [Section 5.3](#53-models-tab)
- _…stop getting throttling errors?_ → Region failover or Global routing,
  [Section 18.3](#183-routing-why-some-models-appear-twice)
- _…know which model to pick?_ → [Section 18.1](#181-the-short-answer)

**Appearance**

- _…switch to dark mode?_ → [Section 5.1](#51-general-tab)
- _…get a flat, paper-like or e-ink look?_ → the **Newspaper** appearance, [Section 5.1](#51-general-tab)
- _…why are my diagrams grey?_ → Mermaid diagrams are drawn in shades of the current appearance's own
  grey so they match the app rather than the diagram library's default palette. They redraw when you
  switch appearance.
- _…change the font?_ → **Interface font** and **Code font**, [Section 5.1](#51-general-tab)
- _…hide sidebar icons?_ → [Section 5.1](#51-general-tab)
- _…set my name and avatar?_ → [Section 5.1](#51-general-tab)
- _…make the text bigger?_ → `Cmd`/`Ctrl` + `+`, [Section 3.2](#32-keyboard-shortcuts)
- _…stop the interface animating?_ → turn on "Reduce motion" in your operating system's accessibility
  settings; the app follows it.

---

## 21. Glossary

**Agent** — a saved bundle of instructions, tools and resources that shapes how the AI behaves. See
[Section 8](#8-agents-what-they-are-and-how-to-build-them).

**Access key** — a two-part password for the AWS API: an access key ID (`AKIA…`, or `ASIA…` if
temporary) and a secret access key. See [Section 4.4](#44-step-3--create-aws-access-keys).

**Amazon Bedrock** — the AWS service that provides the AI models. You pay AWS per use.

**ASL (Amazon States Language)** — the JSON format that describes an AWS Step Functions workflow.

**AWS CLI** — the AWS command line tools. Not required by this app, but `aws configure` is the easiest
way to write the credentials files it can read. See [Section 4.5](#45-step-4--install-the-aws-cli).

**AWS profile** — a named set of credentials in `~/.aws/credentials`. Pointing the app at a profile
means it reads your keys from that file instead of storing its own copy. See
[Section 4.6](#46-step-5--put-your-keys-in-the-aws-credentials-and-config-files).

**Container** — a lightweight, isolated Linux environment. Software inside a container cannot see
your computer unless you deliberately connect the two.

**Context** — everything sent to the model with your message: the system prompt, the tool list, and
the recent conversation. Larger context means more memory and more cost.

**Context window** — the maximum amount of text a model can consider at once, measured in tokens.

**Crawler** — the part of the app that visits a website page by page, following links, to add it to a
knowledge base.

**Cron expression** — five values describing a repeating schedule. See
[Section 14.2](#142-cron-expressions-briefly).

**Docker** — the program that runs containers. Needed for `codeInterpreter` and `dockerSandbox`.

**Guardrail** — an AWS feature that filters or blocks content according to rules you define.

**IAM** — AWS's permissions system. An IAM policy is a document saying what your account may do.

**Inference profile** — a Bedrock routing option that decides which regions can serve your request,
and can also carry tags for cost tracking.

**Embedding** — turning a passage of text into a list of numbers (a vector) that captures its meaning,
so passages about the same thing can be found even when they use different words. Local knowledge
bases use an embedding model for this. See [Section 24.2](#242-how-it-works).

**Knowledge base** — a searchable collection of documents. This app has two kinds. A **local knowledge
base** is built and stored on your computer from files, folders and websites you add, and searched with
`searchKnowledgeBase` ([Section 24](#24-local-knowledge-bases-letting-an-agent-search-your-documents)).
An **Amazon Bedrock Knowledge Base** is one AWS has indexed in your account, searched with `retrieve`
([Section 9.4](#94-ai-services-tools)).

**Memory item** — a rule or a fact saved for every chat. See
[Section 25](#25-memory-rules-and-facts-every-chat-knows).

**MCP (Model Context Protocol)** — an open standard for programs that supply tools to an AI. See
[Section 11](#11-mcp-servers-adding-tools-from-outside-the-app).

**Model** — the AI itself. Claude, Nova and Grok are models.

**Model access** — the per-region switch in the Bedrock console that decides which models your AWS
account may call at all. Separate from IAM permissions, and both have to be in place. See
[Section 4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console).

**Passage** — a short piece of a document (a few hundred words) that a knowledge base stores and
returns as a search result.

**Prompt cache** — remembering the unchanging part of a conversation so it does not have to be
re-processed every turn. Faster and cheaper.

**System prompt** — standing instructions the agent reads before every message. The single biggest
influence on how an agent behaves.

**Thinking / reasoning tokens** — text the model writes privately to work a problem out before
answering. Better answers on hard problems; costs more.

**Throttling** — AWS refusing a request because too many are arriving at once. Retrying, or using a
different region, fixes it.

**Token** — the unit AI models count in, roughly ¾ of an English word. Pricing is per million
tokens.

**Tool** — one specific ability an agent can use, like reading a file or searching the web. See
[Section 9](#9-the-complete-tool-reference).

---

## 22. Troubleshooting

### The app will not start, or shows a settings error

A settings file may have become corrupted. Find your config directory
([Section 19.1](#191-app-settings-and-chat-history)) and delete `config.json`, then restart. You will
have to re-enter your AWS details. If it still will not start after that, please open an issue.

### Nothing happens when I send a message

Check, in this order:

1. **Settings → AWS** — are your credentials filled in?
2. **Settings → Models** — is a model selected, and is it available in your region?
3. Is your internet connection working, and if you are behind a proxy, is
   [the proxy configured](#411-if-your-office-uses-a-proxy)?

### "AccessDeniedException" or a permissions error

Your AWS account has credentials but not permission to use Bedrock, or not permission for that
specific model. Two things to check: click **View required IAM policies** in **Settings → AWS** and
compare against what your account has
([Section 4.3](#43-step-2--give-your-identity-permission-to-call-bedrock)); and in the AWS console,
confirm your account has requested access to that model family
([Section 4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console)) — some models require
you to enable them first.

### "InvalidClientTokenId", "SignatureDoesNotMatch" or "credentials not found"

The keys themselves are the problem, not the permissions. Common causes: a character lost while
copying the secret, keys belonging to a user that has since been deleted, or a profile name in
**Settings → AWS** that does not exist in `~/.aws/credentials`. Test the keys outside the app with
`aws sts get-caller-identity` ([Section 4.8](#48-check-that-it-works)); if that fails too, create a
fresh access key ([Section 4.4](#44-step-3--create-aws-access-keys)).

### It worked yesterday and now says the token is expired

You are using temporary credentials, which expire after a few hours. Refresh them — `aws sso login`,
your company's credential tool, or a new `sts assume-role` — and if you typed them into the app rather
than using a profile, paste the new ones in.
[Section 4.6](#46-step-5--put-your-keys-in-the-aws-credentials-and-config-files) explains why the
profile option saves you this step.

### "ThrottlingException" or "too many requests"

AWS is busy. Fixes, easiest first:

1. Wait a moment and try again.
2. Switch to a **Global** routing option for your model
   ([Section 18.3](#183-routing-why-some-models-appear-twice)).
3. Turn on **Region Failover** and list a couple of backup regions
   ([Section 5.3](#53-models-tab)).

### The answer got cut off mid-sentence

Raise **Max Tokens** in **Settings → Models → Inference Parameters**.

If it is already at the maximum, you have hit the ceiling of the model itself rather than the
setting — every model has its own output limit and requests are capped at it. Switch to a model with
a higher ceiling (Claude Opus 5.5, Sonnet 5.5 and GPT-6 Astra all reach 128,000 tokens) or ask for the
answer in smaller pieces.

### The agent keeps forgetting what I told it earlier

Raise **Context Length** in **Settings → Chat** (the default is 60 messages). Be aware this makes each
message more expensive. For things it should know in every chat, save them as
[memory items](#25-memory-rules-and-facts-every-chat-knows) instead.

### The agent forgot a rule I gave it in the system prompt

Use **Show Preview** in the agent editor's system prompt section to see exactly what the model
receives, placeholders and all. Very long system prompts also get less reliable — if yours is pages
long, tighten it.

### The agent will not run a command

Either the command does not match anything in the agent's **Allowed Commands** list
([Section 8.3](#83-the-agent-editor)), or you are in **Plan mode**
([Section 6.3](#63-plan-mode-and-act-mode)), or the `executeCommand` tool is not enabled for this
agent at all.

### Docker sandbox or code interpreter will not work

Docker needs to be installed _and running_. Start Docker Desktop (or your Docker service) and try
again. For multi-service sandboxes, Docker Compose is also required.

### The sandbox terminal says it cannot reach Docker

The terminal talks to Docker directly rather than through the `docker` command, so it needs a local
Docker socket. Two cases:

- **"This Docker context points at a remote daemon."** Your Docker context is a `tcp://` or `ssh://`
  address. The terminal cannot attach to that, and neither can the rest of the sandbox in any useful
  way — a remote daemon cannot mount your project folder either. Switch contexts with
  `docker context use default` (or whichever local context you have) and reopen the Sandbox tab.
- **"No Docker socket at …"** or **"Permission denied …"** — Docker is not running, or your user
  cannot open its socket. Start Docker, and on Linux add yourself to the `docker` group
  (`sudo usermod -aG docker $USER`, then log out and back in).

Everything else on the Sandbox tab — state, services, ports and the activity log — keeps working either way.

### The agent does not use its knowledge base, or knowledge base errors

See [Section 24.9](#249-knowledge-base-troubleshooting) for the common causes: Plan mode, a model not
enabled in your region, an access error, the on-device model not downloaded, and websites that cannot
be crawled.

### An update fails or never arrives

Check **Settings → Updates**: **Check for updates automatically** must be on, or press **Check for
updates**. On macOS 15 and later, a permission error means the app needs **App Management** permission
in **System Settings → Privacy & Security**. The Linux `.deb` package does not update itself; install
the new `.deb` by hand or use the AppImage. See [Section 5.8](#58-updates-tab).

### Web search does not work

The `tavilySearch` tool needs an API key. Get a free one from [tavily.com](https://tavily.com/) and
put it in **Settings → Chat → Tavily Search API Key**.

### Image generation is not available

The image models the app supports (Stability SD3.5 Large, Stable Image Core and Stable Image Ultra)
are only offered in `us-west-2`. Switch to that region, or use a different approach.

### I get the screen recording or camera permission dialog twice

Run the ad-hoc code signing command:

```bash
sudo codesign --force --deep --sign - "/Applications/Bedrock Engineer.app"
```

### Screen capture produces a blank or black image

On macOS, grant **Screen Recording** permission in **System Settings → Privacy & Security**, then
restart the app.

### My proxy settings do not seem to work

Restart the app. Pages shown inside the app only read the proxy setting at startup; see
[Section 4.11](#411-if-your-office-uses-a-proxy) for what works on each system.

### The app is using a lot of money

Open the **Accounting** page ([Section 26](#26-accounting-what-bedrock-is-costing-you)) to see which
model, feature, agent or day it comes from, and the chat's **Cost** tab for a single conversation. Then
work through [Section 18.5](#185-managing-cost) — Context Length and the Light Processing Model are usually the
two biggest wins.

### An MCP server will not connect

Most often a missing environment variable — an API key or token the server needs, which has to be
listed in the server's configuration. Also check the package name and version are right (use **Load
config** from the registry search rather than typing it by hand), and that a URL-based server does
not need authentication you have not supplied. Use the connection test on the MCP Servers tab to see
the actual error.

On macOS and Linux the app reads your login shell's `PATH` when it starts, so servers launched with
`npx`, `uvx` and the like find the same `node` or `python` your terminal does (including ones
installed through nvm, volta, asdf or `~/.local/bin`). If you have just installed one, restart the
app.

### Something else

Please open an issue on
[the project's GitHub page](https://github.com/ibehren1/bedrock-engineer-rust/issues), and include
what you were doing, what you expected, what happened, and the relevant log file.

---

## 23. Getting help inside the app

You do not have to read this guide from start to finish to use it. The **Help** button — the
question mark at the very bottom of the sidebar, just above the GitHub link — opens it inside the
app, and from there you can search it or ask a question in your own words:

- "Where do I put my AWS keys?"
- "What's the difference between Plan and Act mode?"
- "Which tool lets an agent look something up on the web?"
- "Why is my chat costing so much?"

### 23.1 The guide page

**Help** opens this guide as a page of its own. It needs no AWS access, no internet connection and no
setup, so it works on a new install before anything is configured:

- **Contents** on the left lists every section; the one you are reading is highlighted. Click one to
  jump to it. Links inside the guide jump the same way.
- **Search the guide** at the top finds the sections that match what you type — try "budget",
  "proxy" or "plan mode". Pick a result to jump to it, or press Enter for the first one.
- **Ask a question** opens the help chat ([23.2](#232-the-help-chat)).

The guide is the version that shipped with your build, so it always matches the app you are running.

### 23.2 The help chat

**Ask a question** opens a chat with an agent called **Bedrock Engineer Help**. It is an ordinary
chat on the Chat page: it shows up in your chat history, and you can export, rename or delete it like
any other.

- **It answers from this guide.** With each question, the app looks up the sections of the guide
  that match it (and your previous question, so follow-ups work) and gives only those to the model,
  which names the section it used so you can read more. The look-up runs on your computer; only the
  question, the matching sections and the answer go to Amazon Bedrock.
- **It needs Amazon Bedrock.** The answer comes from a Bedrock model, so the chat needs working AWS
  access ([Section 4](#4-setting-up-access-to-amazon-bedrock)). Until then the guide page says so and
  links to the setup section; the guide itself still works.
- **It has no tools.** It cannot read your files, run commands, search the web or look at your
  settings. That is deliberate: a help button should not be able to touch your machine. When the
  guide does not cover your question, it tells you so instead of guessing — and if answering needs
  something checked on your computer, it tells you what to look at rather than looking itself.

### 23.3 How the guide is searched

The app ships with an index of this guide (a small knowledge base, built from the guide with the
on-device embedding model when the app is built), so there is nothing to download or build and no
cost to search it. Search works two ways:

- **By keyword**, always. This needs nothing beyond the app.
- **By meaning as well**, if you have downloaded the on-device model for local knowledge bases
  (**Settings → Knowledge Base**, [Section 24](#24-local-knowledge-bases-letting-an-agent-search-your-documents)).
  Then a question finds the right section even when it uses different words from the guide.

When the app is updated, the index is replaced along with the guide. If the index ever can't be
read, the help chat falls back to sending the whole guide with your question, and says so.

### 23.4 Which model it uses

The help chat runs on whichever model you selected as the **Light Processing Model** in
**Settings → Models** (see [Section 5.3](#53-models-tab)). Each question carries only a few sections
of the guide, so a small, cheap model like Haiku or Nova answers well and costs little. If you have
not chosen a Light Processing Model, the help chat uses your main conversation model instead.

The agent and model shown at the bottom of a help chat are displayed as plain text rather than
dropdowns, because the help chat pins both and the pickers would not be able to change them.

Clicking **Ask a question** again returns to the same help conversation, so you can carry on where you
left off. To start a fresh one, use the **New chat** button inside the help chat — that gives you a
normal chat — then ask again from the guide page.

### 23.5 When to use the help chat, and when not to

Use it for "how does this app work" questions. It is the fastest way to find a setting, understand
what a tool does, or work out which page you need. It is also a good way through first-time AWS
setup, because [Section 4](#4-setting-up-access-to-amazon-bedrock) walks that whole process through
step by step — the help chat can talk you along it and tell you which step a given error belongs to.
(While Bedrock isn't working yet, read Section 4 on the guide page instead.)

Do not use it for work — it has no tools, so it cannot write files, run anything or search the web.
And it only knows what is in this guide, so it cannot answer general questions about Amazon Bedrock,
AWS or the models beyond what Sections 4 and 19 cover, and it cannot look at your AWS account or run
the verification commands for you. For anything wider, ask a normal agent with the web search tool
enabled, or read the AWS documentation.

---

## 24. Local knowledge bases: letting an agent search your documents

### 24.1 What a local knowledge base is

A **local knowledge base** is a searchable library you give one agent: documents from your computer
and pages from websites. The agent gets a search tool for it, so it can answer from _your_ material —
a product manual, a policy folder, a documentation site — and say which file or page each answer came
from, instead of answering from general knowledge.

"Local" means the knowledge base is built and stored on your computer by the app itself. You do not
need to set anything up in AWS. (Amazon Bedrock Knowledge Bases, which live in your AWS account and
are searched with the `retrieve` tool, are a separate feature; see [Section 9.4](#94-ai-services-tools).)

Each agent has its own knowledge bases, and an agent can have several — say one for the product docs
and one for the support handbook. Knowledge bases are set up on the **Knowledge Base** tab of the agent
editor ([24.3](#243-the-knowledge-base-tab-in-the-agent-editor)), and the app-wide choices are on
**Settings → Knowledge Base** ([24.6](#246-the-knowledge-base-settings-tab)).

Use attachments ([Section 7](#7-attachments-giving-the-agent-your-files)) for a file you want the agent
to read in one chat. Use a knowledge base for a body of material the agent should be able to look
things up in, in every chat, without reading all of it each time.

### 24.2 How it works

```mermaid
flowchart LR
  F["Files and<br/>web pages"] -->|"text extracted,<br/>split into passages"| P["Passages"]
  P -->|"embedding model"| V["Passages + vectors<br/>in one file<br/>on your computer"]
  Q["The agent's<br/>question"] -->|"embedding model"| S{"Search by meaning<br/>and by keywords"}
  V --> S
  S -->|"best passages,<br/>with their sources"| A["The agent"]
```

1. **Adding a source.** When you add files, a folder or a website, the app reads the text out of each
   document and splits it into **passages** of a few hundred words (the chunk size in Settings),
   overlapping a little so nothing is cut in half.
2. **Embedding.** Each passage is turned into a **vector** — a list of numbers that captures what it
   means — by an **embedding model**. That is either a model on Amazon Bedrock (Amazon Titan Text
   Embeddings V2 or Cohere Embed) or a small model that runs on your own computer
   (multilingual-e5-small). See [24.6](#246-the-knowledge-base-settings-tab).
3. **Searching.** When the agent searches, its question is embedded the same way, and the app looks for
   passages close in meaning. It also runs a keyword search, so exact names, codes and error messages
   are found even when the meaning-based search misses them, and combines the two rankings. The balance
   between them is a setting.

Each knowledge base is a single file on your computer:
`<app data folder>/knowledgebases/<agent id>/<knowledge base id>.db`, where the app data folder is the
one shown in **Settings → Workspace → Config Directory**. The on-device embedding model, once
downloaded, is kept in `knowledgebases/.models/`.

Files you add are **read where they are, not copied**. The knowledge base stores the passages it took
from them; the original files are never changed. Edits to files and folders are picked up by the
automatic re-sync (hourly by default, see [24.6](#246-the-knowledge-base-settings-tab)) or when you
re-sync the source yourself. If you move or delete a file, its source reports it as missing at the next
sync and keeps the passages it already has until you remove it.

What happens to knowledge bases when you change the agent:

- **Duplicate** an agent and its knowledge bases are copied to the new agent.
- **Delete** an agent and its knowledge bases are deleted with it. So does cancelling a new agent before
  you first save it.
- **Download YAML**, **Save as File**, **Share to Organization** and **Import Agent** never include
  knowledge bases. They stay on your computer, so whoever you share an agent with has to add their own.
- Agents shared through a project file, and built-in agents, cannot have local knowledge bases.
  Duplicate the agent and add them to your copy.

### 24.3 The Knowledge Base tab in the agent editor

Open **My Agents**, edit an agent, and choose **Knowledge Base** in the editor's side tabs (between MCP
Servers and Tools). The tab works for a new agent too, before you save it.

**Creating a knowledge base**

1. Click **New knowledge base**.
2. Enter a **Name** (for example "Product docs") and a **Description** of what it contains. The
   description matters: the agent sees it and uses it to decide when to search this knowledge base.
3. Click **Create**. The form shows which embedding model it will use — the one chosen in Settings at
   that moment. A knowledge base keeps that model for good, unless you rebuild it.

Each knowledge base is a card. Its header shows the name, a health badge, the description and the
embedding model; collapsed, it also shows the number of passages and its size. The pencil button
**renames** it (name and description), and the red trash button **deletes** it after asking. Deleting
removes only the knowledge base's indexed content from your computer; the original files, folders and websites
are not touched.

**Adding files**

**Add files** opens a file picker. You can pick many files at once. Supported:

- Text, Markdown and code files — plain text, logs, CSV/TSV, Markdown, reStructuredText, AsciiDoc,
  JSON, YAML, TOML, XML and config files, and source code in most common languages
- PDF
- Word (`.docx`)
- HTML pages saved as files
- Excel and other spreadsheets (`.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.ods`) — one section per sheet
- PowerPoint (`.pptx`) — one section per slide

Older `.doc` and `.ppt` files, and files over 100 MB, are not supported. A PDF that is only
scanned images, with no text in it, adds nothing searchable. Adding a file that is already in the
knowledge base is skipped.

**Add folder** opens a folder picker and adds the folder as one source that follows its contents:
every supported file in it and its subfolders is a document of that source. Each sync, automatic or
manual, looks at the folder again: new files are added, edited ones re-read, and files deleted from the
folder are removed from the knowledge base. Files whose modification time has not changed are not read
again. A folder source leaves out hidden files and folders (names starting with `.`, such as `.git`),
dependency and build folders (`node_modules`, `target`, `venv`, `__pycache__`), shortcuts and symbolic
links, unsupported file types and files over 100 MB. A file that cannot be read (a damaged PDF, say)
is counted as skipped and tried again when it changes or when you re-sync the folder. There is no limit
on how many files a folder can have, but a folder with 500 or more supported files asks before it is
added, since reading and embedding that many takes a while and, with a Bedrock embedding model, costs
embedding tokens.

**Adding a website**

Type the start address into the website box (for example `https://docs.example.com/guide/`), adjust
the options if you want, and click **Add website**:

| Option                   | What it does                                                                                                           |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| **Link depth**           | How many links away from the start page the crawler goes (0–10). 0 is just the start page. Default from Settings: 2.   |
| **Max pages**            | The most pages it will add from this site (1–1,000). Default from Settings: 50.                                        |
| **Stay under this path** | On by default. Only follow links below the start address's folder — so starting at `/guide/` keeps it out of `/blog/`. |

How the crawler behaves is in [24.5](#245-how-the-website-crawler-behaves).

**Watching progress**

Sources are processed in the background, so you can keep working. While a job runs, the card shows a
progress bar with what it is doing — **Crawling 12/50**, **Reading…**, **Embedding 80/240**,
**Saving…** — and a **Stop** button. When a job finishes, a short message says what happened: for a
website, how many pages were read, updated, removed or skipped; for a folder, the same for its files;
for a file, **Indexed** or **Unchanged**.

**The sources list**

Every file, folder and website appears under **Sources** with its status (**Queued**, **Processing**,
**Ready**, **Error** or **Stopped**), how many documents and passages it produced, and when it was last
synced. A website also shows its link depth and page limit. Errors are shown in red under the source.
Each source has buttons to:

- **Re-sync** — read it again and pick up changes. Documents that have not changed are skipped, so
  re-syncing a large source is quick and, with Bedrock embeddings, cheap. On a website, pages that have
  disappeared are removed from the knowledge base; on a folder, files that were deleted. Files and
  folders are also re-synced automatically when their files change.
- **Crawl settings** (websites only) — change the link depth, page limit and stay-under-path option.
  **Save and re-sync** applies them straight away; **Save** applies them at the next re-sync.
- **Remove** — take the source and its passages out of the knowledge base, after asking.

If you quit the app while a source is being processed, it shows **Interrupted. Re-sync to continue.**
the next time.

**Trying a search**

Once a knowledge base has passages, **Try a search** lets you type a question ("Ask something the
documents answer") and see exactly what the agent would get back: each passage with its title, section,
source and score. Use it to check that a document went in properly and that your question finds it.
These test searches do not count towards the agent's search statistics.

**The statistics tiles**

| Tile                        | What it shows                                                                                                                                                                                                   |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Passages**                | How many passages it holds, from how many documents, files, folders and websites.                                                                                                                               |
| **Storage**                 | The size of the file on disk, with a meter of how much of it is free space that **Optimize** would give back (or **Compact** if there is none to speak of). The meter turns to a warning colour at 20% or more. |
| **Agent searches, 30 days** | How many times the agent searched it over the last 30 days, with a small per-day chart (hover over a day for its count), and when it was last used — or "Not used by the agent yet".                            |
| **Typical search time**     | The median time to embed the question and search, over the last 30 days, and the time 95% of searches came in under.                                                                                            |

A knowledge base the agent never searches is worth a look: the description may not tell the agent what
it is for, or the agent may not have been asked anything it covers.

**The health badge and the Maintenance panel**

The badge next to the name sums up the knowledge base's health: **Healthy**, **Suggestion**, **Needs
attention** or **Problem**. The **Maintenance** panel below the tiles says when the knowledge base was
last optimized and how long its last update took, lists anything that needs attention, and has the
**Optimize** button. Both are explained in [24.7](#247-health-checks-and-maintenance).

**Rebuilding with a different embedding model**

If you change the embedding model in Settings, existing knowledge bases keep the model they were built
with — searches have to use the same model the passages were embedded with. The card then says "Built
with … ; Settings now use …" and offers **Rebuild**, which re-embeds every passage with the new model.
The passages themselves are kept as they are (a new chunk size is not applied), so a rebuild does not
need the original files. It can take a while and, with a Bedrock model, costs embedding tokens.

### 24.4 How the agent uses a knowledge base

As soon as an agent has a knowledge base, it gets the built-in **local-knowledgebase** MCP server and
its three tools — `searchKnowledgeBase`, `listKnowledgeBases` and `listKnowledgeBaseSources`
([Section 9.10](#910-local-knowledge-base-tools)). You do not have to tick anything. You can see them:

- on the agent editor's **MCP Servers** tab, as a read-only **Built-in** card with **Test connection**;
- on its **Tools** tab, in the MCP category with the agent's other MCP tools.

The app also adds a `<local_knowledge_bases>` block to the agent's system prompt, listing each
knowledge base by name and description and telling the agent to search them before answering
questions they may cover, and to cite the file path or URL of the passages it uses. Open the chat's
**System prompt** tab to see it.

To check that the agent really used its knowledge base, look in the conversation for a
`searchKnowledgeBase` tool call and expand it: you see the query the agent searched for and the
passages it got back. If the answer does not match those passages, the agent answered from somewhere
else.

Tips for good results:

- **Write a clear description** for each knowledge base. It is how the agent knows when to search it.
- **Say so in the system prompt** if the agent should always check its knowledge base first, or only
  answer from it.
- **Ask in the words the documents use.** The search understands meaning, but exact product names and
  error codes help it.
- The knowledge base tools work in background tasks and when the agent is called by another agent, but
  not in **Plan mode**, where MCP tools are off.

### 24.5 How the website crawler behaves

The app has its own crawler, so adding a website needs nothing else installed. It is built to be polite
to the sites it visits:

- It starts at the address you give and **stays on that site** (the same address and port), and, with
  **Stay under this path**, under that folder. Links to other sites are ignored, and a page that
  redirects off the site is skipped.
- It follows links **breadth first** up to the link depth, and stops at the page limit.
- It **honours robots.txt**. If the site's robots.txt does not allow the start page, the crawl stops
  with "robots.txt does not allow crawling this URL". This can be turned off in Settings, for sites you
  run yourself.
- It **reads sitemap.xml** (or the sitemaps robots.txt names) to find pages that links alone would miss.
- It **waits between requests** — 500 ms by default, or longer if the site's robots.txt asks for a
  crawl delay (up to 10 seconds).
- It **skips pages that ask not to be indexed** (a `noindex` robots tag), and does not follow the links
  of pages marked `nofollow`.
- It reads HTML pages, PDFs, and plain text and Markdown files. It skips images, video, archives,
  scripts, style sheets and the like, and pages over 5 MB. From an HTML page it keeps the main content,
  leaving out menus and footers where it can tell them apart.
- It identifies itself as `BedrockEngineerKnowledgeBase`, and goes through your proxy if one is set.

The crawler does not log in and does not run a page's scripts, so pages behind a sign-in, and pages
whose content only appears after JavaScript runs, come out empty or nearly so. If the start page itself
cannot be fetched, the whole crawl fails.

Websites change, so **re-sync** them now and then: unlike files and folders, they are never re-synced
automatically. A website not synced for 30 days is flagged in the health checks.

### 24.6 The Knowledge Base settings tab

**Settings → Knowledge Base** has four sections.

**Overview**

The top line says how much space knowledge bases take and where they are stored. Then:

- **Tiles** with the totals: how many knowledge bases, for how many agents; passages and documents;
  storage, with how much could be reclaimed; and agent searches over 30 days, with a typical search
  time and a per-day chart.
- A **table** with one row per knowledge base: its name, agent (or "Agent deleted") and embedding
  model; passages and sources; storage; agent searches over 30 days; search time (median and 95%);
  when it was last used; and its health (hover for the issues). Each row has an **Optimize** and a
  **Delete** button.
- **Refresh**, **Optimize all** (optimizes them one after another) and, when there are any, **Delete
  orphaned knowledge bases** — knowledge bases left behind by an agent that no longer exists. A line
  above the table says how many knowledge bases need attention.

**Embedding model**

The model that turns text into vectors. New knowledge bases use the model chosen here; existing ones
keep the model they were built with until you rebuild them
([24.3](#243-the-knowledge-base-tab-in-the-agent-editor)).

| Provider                     | Model                                             | Notes                                                                                                                                        |
| ---------------------------- | ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **Amazon Bedrock** (default) | **Amazon Titan Text Embeddings V2** (the default) | Good all-round choice. **Dimensions** 1024 (default), 512 or 256 — smaller is a smaller file, slightly less precise.                         |
|                              | **Cohere Embed Multilingual v3**                  | For documents in many languages.                                                                                                             |
|                              | **Cohere Embed English v3**                       | For English documents.                                                                                                                       |
|                              | **Cohere Embed v4**                               | Cohere's newest. Dimensions 1536 (default), 1024, 512 or 256.                                                                                |
| **On this computer**         | **multilingual-e5-small**                         | Runs on your computer's processor, works offline, about 100 languages, and costs nothing per use. Needs a one-time download of about 465 MB. |

A Bedrock model has to be enabled in the Bedrock console for your region
([Section 4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console)), and Bedrock bills per
embedded token. It uses the AWS credentials and region from the AWS tab.

The **Local model: multilingual-e5-small** block is always shown, whichever provider you pick, so you
can download the model ahead of time. **Download model** fetches it from Hugging Face (through your
proxy, if set), checks every file, and shows the progress; **Cancel** stops it. Its status reads
**Downloaded**, **Not downloaded** or **Downloading …%**. **Delete model** frees the space again, but
knowledge bases built with it cannot be searched until you download it again. The on-device model works
on Apple Silicon and Intel Macs, Windows and Linux.

**Chunking and retrieval**

| Setting                         | Default | What it does                                                                                                                                                                              |
| ------------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Chunk size (tokens)**         | 400     | How long each passage is (50–2,000; a token is roughly ¾ of a word). Smaller passages give more precise hits; larger ones give the agent more context per hit. New knowledge bases only.  |
| **Chunk overlap (tokens)**      | 60      | How much consecutive passages share, so a sentence on a boundary is not lost. Up to half the chunk size. New knowledge bases only.                                                        |
| **Results per search**          | 6       | How many passages a search returns when the agent does not ask for a number (1–50). Applies to every search.                                                                              |
| **Semantic vs. keyword weight** | 0.60    | 1.0 ranks by meaning only, 0.0 by keyword matches only; values in between combine both. Lower it if exact terms (part numbers, function names) are being missed. Applies to every search. |

**Automatic re-sync**

| Setting                                             | Default | What it does                                                                                                                                                                                                                                                          |
| --------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Re-sync changed files and folders automatically** | On      | Checks every file and folder source in the background, about 30 seconds after the app starts and then at the interval below, and re-syncs those whose files were added, edited or deleted. Checking reads only file dates; nothing is embedded unless a file changed. |
| **Check every (minutes)**                           | 60      | 5–1,440. Changes made while the app is closed are picked up by the check after it starts.                                                                                                                                                                             |

Websites are not part of it; re-sync them yourself.

**Website crawler**

Defaults for new website sources ([24.5](#245-how-the-website-crawler-behaves)):

| Setting                           | Default | Notes                                                                                    |
| --------------------------------- | ------- | ---------------------------------------------------------------------------------------- |
| **Link depth**                    | 2       | 0–10. Saved with each website when you add it.                                           |
| **Max pages**                     | 50      | 1–1,000. Saved with each website when you add it.                                        |
| **Delay between requests (ms)**   | 500     | Applies to every crawl and re-sync.                                                      |
| **Respect robots.txt**            | On      | Applies to every crawl and re-sync. Only turn it off for sites you are allowed to crawl. |
| **Use sitemap.xml to find pages** | On      | Applies to every crawl and re-sync.                                                      |

### 24.7 Health checks and maintenance

The app checks each knowledge base and shows the result as its health badge, in the Maintenance panel
and in the Settings overview:

| Check                          | Badge               | What it means and what to do                                                                                                                                       |
| ------------------------------ | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Sources failed to sync         | **Problem**         | One or more sources ended in an error. Read the error on the source, then **Re-sync** or **Remove** it.                                                            |
| Local model missing            | **Problem**         | The knowledge base was built with the on-device model, which is not downloaded (or was deleted). Searches fail until you download it in Settings → Knowledge Base. |
| Websites not synced in 30 days | **Needs attention** | The site may have changed since. **Re-sync** it.                                                                                                                   |
| Different embedding model      | **Suggestion**      | Settings now name a different model than the one it was built with. It still works; **Rebuild** if you want to switch.                                             |
| Free space in the file         | **Suggestion**      | At least 1 MB, and 20% or more of the file, is free space left by removed or updated passages. **Optimize** gives it back.                                         |

**Optimize** (on the card, per row in Settings, or **Optimize all**) tidies the keyword index, checks the
file for errors, and compacts the file to give free space back. It reports the size before and after,
or that the integrity check failed — in which case remove and re-add the sources, or delete and
recreate the knowledge base. It waits for any job already running on that knowledge base.

You rarely need to press it: after any update that leaves a lot of free space (at least 4 MB and a
quarter of the file), the app optimizes the knowledge base by itself.

### 24.8 Costs, permissions and privacy

**With a Bedrock embedding model:**

- **Cost.** Bedrock charges per token embedded. Adding or re-syncing a source embeds every new or
  changed passage (unchanged documents are skipped); each search embeds the question, which is tiny.
  Embedding is much cheaper than chatting, but a large website or document collection adds up, and a
  **Rebuild** embeds everything again. Embedding costs are not on a chat's Cost tab; the Accounting
  page shows them as "Knowledge base indexing" and "Knowledge base search"
  ([Section 26](#26-accounting-what-bedrock-is-costing-you)).
- **Permissions.** Your AWS identity needs `bedrock:InvokeModel` on the embedding model. Both policies
  under **Settings → AWS → View required IAM policies** already allow `InvokeModel` on all models, so
  nothing extra is needed if you use one of them
  ([Section 4.3](#43-step-2--give-your-identity-permission-to-call-bedrock)). A hand-written policy that
  lists specific models must include the embedding model.
- **Model access.** The embedding model must be enabled in the Bedrock console in your region, like any
  other model.
- **What is sent.** The text of each passage when it is embedded, and the agent's search question.
  Bedrock returns the vectors; the app stores nothing in AWS.

**With the on-device model**, embedding happens on your computer: no AWS cost, no permissions, and it
works offline. Only the one-time model download goes over the internet.

Either way, the knowledge base itself — passages, vectors and search history — stays on your computer.
The passages the agent finds are sent to the chat model as part of the conversation, like anything
else the agent reads.

### 24.9 Knowledge base troubleshooting

**"No access to the embedding model … Enable it in the Bedrock console for this region"** — an
`AccessDenied` error from Bedrock. Either the embedding model is not enabled in your region
([4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console)), or your IAM policy does not
allow `bedrock:InvokeModel` on it ([4.3](#43-step-2--give-your-identity-permission-to-call-bedrock)).
Or switch to the on-device model.

**"The embedding model is not available … It may not be offered in this region"** — the model you chose
is not offered in your AWS region. Pick another embedding model, switch regions, or use the on-device
model. Changing the model only affects new knowledge bases; **Rebuild** existing ones.

**"AWS authentication failed. Please check your credentials."** — the same credential problems as for
chat; see [Section 22](#22-troubleshooting).

**"The local embedding model is not downloaded yet"** — download it in **Settings → Knowledge Base**,
or switch the provider to Amazon Bedrock. A knowledge base built with the on-device model needs it for
every search, so its health badge shows **Problem** until it is back.

**A website comes out empty or with only a few pages** — the site may block crawling in robots.txt,
need a sign-in, build its pages with JavaScript, or mark them `noindex`. Raise **Link depth** or **Max
pages** if it simply stopped early, or turn off **Stay under this path** if the pages you want are in a
sibling folder. Read the source's error and the job message for which it was.

**"File not found (moved or deleted?)"** on re-sync — the file is no longer where it was added from.
Remove the source and add the file again from its new place.

**"Cannot read …"** on a folder source — the folder was moved, renamed or deleted, or is on a drive
that is not connected. Reconnect the drive and **Re-sync**, or remove the source and add the folder
from its new place.

**Edits to a file don't show up in answers yet** — automatic re-sync checks every hour by default.
**Re-sync** the source to pick them up now, or shorten the interval in **Settings → Knowledge Base →
Automatic re-sync**. Websites are never re-synced automatically.

**The agent answers without searching** — check that you are in **Act** mode (the knowledge base tools
are off in Plan mode), that the knowledge base has passages and a description that matches the
question, and the `<local_knowledge_bases>` block on the **System prompt** tab. Telling the agent in
its system prompt to search first helps. **Try a search** on the Knowledge Base tab shows whether the
question finds anything at all.

**Searches return the wrong passages** — try a lower **Semantic vs. keyword weight** for exact terms,
more **Results per search**, or a smaller chunk size (for new knowledge bases).

---

## 25. Memory: rules and facts every chat knows

### 25.1 What memory is

Memory is one shared list of short items the assistant keeps in mind in every chat — your preferences,
house rules, and facts about your setup — so you do not have to repeat them. Each item is one of two
kinds:

- A **rule** is an instruction the assistant always follows, unless you tell it otherwise in that chat.
  For example: "Use pnpm instead of npm".
- A **memory** is a fact it keeps in mind and uses when it is relevant. It is background, not an
  instruction. For example: "The main repo is at ~/work/app".

The two are given to the model differently: rules as instructions to always follow, which only an
explicit instruction from you in that chat overrides; facts as background to use when relevant.

### 25.2 Saving items from a chat

Just say it: "add a rule: always use TypeScript", "remember that prod is eu-west-1", or "forget the pnpm
rule". The agent uses its memory tools (`addMemory`, `updateMemory`, `removeMemory`;
[Section 9.7](#97-memory-tools)) and the reply shows a small note — **Memory saved**, **Memory updated**
or **Memory removed** — with **View** to open the Memory page and, for saved and removed items,
**Undo**.

This needs the memory tools enabled on the agent; every tool preset except Custom includes them. They
work in Plan mode too.

### 25.3 The Memory page

Open **Memory** (the brain icon) in the sidebar. The page explains the two kinds and lists every item,
with how many are enabled.

- **Add** an item yourself: type it into **Add a memory or rule…**, choose its **Type** (Rule or
  Memory), and click **Add**.
- Each item shows whether it came **from chat** or was **added manually**, and when.
- The checkbox (**Include in context**) switches a single item off without deleting it.
- Click the kind label to switch an item between rule and memory.
- **Edit** changes its text (Enter saves, Shift+Enter makes a new line, Esc cancels); **Delete** removes
  it after asking.

### 25.4 Turning memory on and off for a chat

The **brain button** above the message box adds every switched-on item to the chat's system prompt. It
is highlighted, with the number of items, while on. It is on for every new chat; turning it off only
affects that chat. Its tooltip says whether memory is on. Open the **System prompt** tab to see the
items as the model receives them.

Memory is never added to the Help chat. Items are stored on your computer, in `memory.json` in the app
data folder ([Section 19.1](#191-app-settings-and-chat-history)).

Keep items short and few. Every switched-on item is sent with every message, so a long list costs a
little on every turn and gives the model more to weigh.

---

## 26. Accounting: what Bedrock is costing you

### 26.1 What it records

The **Accounting** page (the dollar sign in the sidebar) shows the estimated Amazon Bedrock cost of
everything the app does, not just chats. Every call to Bedrock is recorded the moment it is made:

- chats, the Website, Diagram and Step Functions generators, chat titles and the agent editor's
  generators
- background agents and sub-agents (`@` delegation)
- knowledge bases that use a Bedrock embedding model, both indexing and searching (the on-device
  model is free and is not recorded)
- tools that call Bedrock: image generation (priced per image), video generation (per second),
  image recognition, Bedrock Agents, Knowledge Base retrieval and Flows
- guardrail checks (per text unit) and Amazon Translate (per character)

Each call is priced when it happens, with the list price at that time, and the record is kept for
good. It is separate from your chat history: deleting a chat does not remove its cost. The first
time the app runs with this page, it also imports the token usage your saved chats already held, so
earlier months are not empty. Before that, only chats saved their usage, so imported months show chat
cost only, and the page says so.

### 26.2 Reading the page

Pick a month at the top. You see:

- **Tiles:** the month's total so far, where the month is heading at the current pace (this month
  only), the previous month's total, the number of calls, how much prompt caching saved, and how many
  calls had no known price.
- **Cost per day:** a bar per day, stacked by model.
- **Cost per month:** every month on record; click a bar to open that month.
- **Tables by model, by feature, by agent and by day**, with input, output and cache tokens, other
  units (images, seconds of video, characters) and the share of the month.
- **The individual calls**, newest first, with **Load more** at the bottom.

Click a row in the model, feature or agent table to show only that on the whole page; click it again,
or the × on its button at the top, to go back. Chats made before the page existed show under
"Earlier chats (agent not recorded)" in the agent table, because chats were not saved with their
agent.

The chat history list shows each chat's cost, and a background task's **Execution History** shows
each run's cost.

### 26.3 Exporting

**Export CSV** saves every call of the month (or of the current filter) as a spreadsheet file, by
default in your Downloads folder.

### 26.4 A monthly budget

On the Accounting page, **Set a budget** takes an amount in dollars per month and the levels to warn
at, as percentages (80 and 100 unless you change them). The card then shows how much of the budget
the month has used.

The first time the month's estimated cost reaches each level, you get a desktop notification — once
per level per month. While the month is over budget, the dollar sign in the sidebar has a red dot.
**Edit** changes the budget and **Remove budget** turns it off. The budget is a warning only: nothing
is ever blocked.

### 26.5 How accurate it is

The figures are estimates from on-demand list prices, and your AWS bill is the final word.

- Where a model's price differs between its global and geography profiles, or for very long prompts,
  and the app's model list records how, that is applied. Calls through your own application inference
  profiles are priced as the model the profile uses (the app looks this up once per profile).
- Discounts, savings plans, taxes and anything the model list doesn't record are not reflected.
- Cohere embedding tokens are estimated from the text length; the Titan model reports its count.
- Knowledge Base retrieval and Flows report no usage, so they are counted but not priced. Calls to a
  model with no known price are counted too, flagged, and left out of the totals.
