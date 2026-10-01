# User Guide

A complete, plain-language guide to this app: what every screen does, what every tool does, what
every setting means, and what to do when something goes wrong.

**Who this is for:** anyone comfortable using a computer. You do not need to be a programmer. Where
a technical word is unavoidable, it is explained the first time it appears, and again in the
[Glossary](#21-glossary) at the end.

**How to find things fast:** use the [Table of contents](#table-of-contents) for topics, or the
[Question index](#20-question-index-how-do-i) if you have a specific "how do I…" question.

**Or just ask.** The **Help** button in the bottom-left corner of the app opens a chat with this
guide already attached, so you can ask your question in your own words instead of hunting for the
section that answers it. See [Section 23](#23-getting-help-inside-the-app).

> **Note for the AI assistant in this app**
>
> This file is written to be read by you as well as by people. If the user asks how the app works,
> answer from this file instead of guessing — read it with `readFiles` if you have that tool, or use
> the copy already in your context if it was attached to the conversation, as it is in the Help chat.
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

On a Mac, you will probably see a warning that the app was blocked, because it is not distributed
through the Mac App Store. That is expected. Go to **System Settings → Privacy & Security**, scroll
down, find the blocked app, and click **Open Anyway**.

After installing on a Mac, and again after every update, run this one command in the Terminal app
before opening the app:

```bash
sudo codesign --force --deep --sign - "/Applications/Bedrock Engineer.app"
```

The app is self-signed rather than signed by Apple, so each copy has to be signed on the Mac it runs
on. Without it, permission pop-ups (screen recording, camera) may appear twice or behave strangely.

On Windows, if SmartScreen says "Windows protected your PC", click **More info**, then **Run anyway**.

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

Every screen in the app is reachable from the icon strip down the left edge.

| Icon           | Page                         | What it is for                                                                    |
| -------------- | ---------------------------- | --------------------------------------------------------------------------------- |
| House          | **Home**                     | The starting screen.                                                              |
| Speech bubbles | **Chat**                     | The main workspace. Talk to an agent, which uses tools to get things done.        |
| Robot          | **My Agents**                | Create, edit, arrange and hide your agents.                                       |
| Book           | **Agent Directory**          | Browse ready-made agents other people have shared, and add them to your own list. |
| Pulse          | **Background Agent**         | Set an agent to run automatically on a schedule.                                  |
| Feather        | **Website Generator**        | Describe a web page and watch it get built, with a live preview.                  |
| Merging arrows | **Step Functions Generator** | Build an AWS Step Functions workflow from a description.                          |
| Layout         | **Diagram Generator**        | Turn a description into an AWS architecture diagram.                              |
| Gear           | **Settings**                 | Everything configurable.                                                          |

You can hide any icon you never use: **Settings → General → Sidebar Settings**. Hiding an icon does
not disable the feature; the page still exists at its address.

Two more buttons sit at the **bottom** of the strip, below the gap. These are not pages, and they
cannot be hidden:

| Icon          | What it does                                                                                                         |
| ------------- | -------------------------------------------------------------------------------------------------------------------- |
| Question mark | **Help** — opens a chat that can answer questions about this app. See [Section 23](#23-getting-help-inside-the-app). |
| GitHub cat    | Opens the project's page on GitHub in your web browser.                                                              |

### 3.2 Keyboard shortcuts

| Shortcut                          | What it does                                                                         |
| --------------------------------- | ------------------------------------------------------------------------------------ |
| `Cmd`/`Ctrl` + `K`                | Open the command palette — a search box for jumping to any page.                     |
| `Cmd`/`Ctrl` + `,`                | Open Settings.                                                                       |
| `Cmd`/`Ctrl` + `1` … `9`          | Jump straight to the 1st through 9th page in the sidebar.                            |
| `Cmd`/`Ctrl` + `Shift` + `A`      | On the Chat page, switch between **Plan** and **Act** mode.                          |
| `Cmd`/`Ctrl` + `+` / `-` / `0`    | Make everything bigger, smaller, or back to normal size.                             |
| `Cmd`/`Ctrl` + `R`                | Reload the window, if it ever looks stuck. Your settings and chats are kept.         |
| `Enter` or `Cmd`/`Ctrl` + `Enter` | Send your message. Which one sends is your choice — see [Section 5.4](#54-chat-tab). |

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
   this app is built around. Ticking Amazon **Nova** models as well is worth it: Nova Canvas and
   Reel are used by the image and video tools.
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

Copy the JSON from that dialog, then in the AWS console:

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
- **AWS Session Token** — only if your keys are temporary (see above). Leave it blank otherwise.
- **AWS Region** — the region you enabled models in, such as `us-east-1`.

The app stores these on your own machine in its settings file. Nothing is sent anywhere except to
AWS.

**Option B — use an AWS profile (better if you have one)**

Turn on **Use AWS Profile** and type the profile name — `default`, or `bedrock` if you followed the
naming above. The app then reads `~/.aws/credentials` every time it makes a request, so your secret
never gets copied into the app's own storage, and rotating or refreshing the key in that file takes
effect without touching the app.

Set the **AWS Region** in the app either way. The app uses the region you pick here, not the one in
your profile's config file.

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
  --model-id us.anthropic.claude-sonnet-4-5-20250929-v1:0 \
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
for text, images and video, plus a few read-only permissions to list what is available.
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

Settings are split into five tabs listed down the left. Each tab has its own web-style address, so a
link can point at one directly (for example `#/setting/aws`).

### 5.1 General tab

| Setting                         | What it does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Language**                    | Switches the app's own text between English and Japanese. It does not change what language the AI replies in — for that, just ask the agent, or say so in its system prompt.                                                                                                                                                                                                                                                                                                                          |
| **Appearance → Theme**          | **Light**, **Newspaper**, **Dim**, **Charcoal**, **Dark**, or **System** (follow your operating system) — listed from lightest to darkest. Dim is the default: dark, but softer than full black. **Newspaper** is flat black-on-white like a printed page or an e-ink reader — square corners, no shadows, and colour used only for success and error. **Charcoal** is a warm grey with an amber accent; because amber is the accent there, warnings use a different colour and always carry an icon. |
| **Appearance → Interface font** | The typeface used for labels, menus and body text: **Inter** (default), **Geist**, or your **system font**. All are bundled with the app, so any choice works offline and looks the same on macOS, Windows and Linux.                                                                                                                                                                                                                                                                                 |
| **Appearance → Code font**      | The typeface used for code, JSON, file paths and identifiers: **JetBrains Mono** (default), **Geist Mono**, or your **system font**. This is deliberately separate from the interface font — JetBrains Mono makes `1`, `l` and `I`, and `0` and `O`, much easier to tell apart, which matters when you're reading a tool-use ID or an ARN. Your font choices apply to every appearance.                                                                                                               |
| **User Avatar / Name**          | Pick an emoji and a display name for yourself. These appear on your messages in chat and in exported documents.                                                                                                                                                                                                                                                                                                                                                                                       |
| **Sidebar Settings**            | Show or hide each navigation icon.                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| **Notification Settings**       | Turn on desktop notifications so you get a pop-up when a long answer finishes or a scheduled background task completes. Useful if you switch to other work while the agent runs.                                                                                                                                                                                                                                                                                                                      |

### 5.2 AWS tab

Covered in full in [Section 4](#4-setting-up-access-to-amazon-bedrock): credentials, region, IAM policy help
and proxy.

### 5.3 Models tab

| Setting                                           | What it does                                                                                                                                                                                                                                                  |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **LLM (Large Language Model)**                    | Your default model for new conversations. You can still change it per conversation from the Chat page.                                                                                                                                                        |
| **Visible Models**                                | An allowlist. Add models here to trim the dropdown down to only the ones you want to see. Leave it empty to see everything your region offers.                                                                                                                |
| **Enable Region Failover on ThrottlingException** | If AWS is busy and rejects your request ("throttling"), automatically retry in another region you list under **Failover Regions**. Turn this on if you get "too many requests" errors.                                                                        |
| **Enable Application Inference Profiles**         | For cost tracking. Lets you route requests through a tagged profile so your AWS bill can be split by project or team. See [the inference profile guide](./inference-profile/INFERENCE_PROFILE.md).                                                            |
| **Inference Parameters**                          | Fine controls over how the model writes. See below.                                                                                                                                                                                                           |
| **Light Processing Model**                        | A smaller, cheaper model used for small background jobs like naming your chats and suggesting follow-up questions. Set it to something cheap; it does not affect the quality of your actual answers. Choose **Use Main Conversation Model** to turn this off. |
| **Guardrails**                                    | Connect an Amazon Bedrock Guardrail to filter content. See below.                                                                                                                                                                                             |

**Inference parameters, in plain terms:**

- **Max Tokens** — the longest reply the model is allowed to produce. A "token" is roughly ¾ of a
  word. Raise it if answers get cut off mid-sentence; lower it to keep replies short and cheap. Each
  model has its own ceiling — 128,000 tokens on Claude Opus 5, 64,000 on Haiku 4.5, 5,120 on the
  first-generation Nova models — and requests are capped at whichever is lower, this setting or the
  ceiling of the model you are using. So you can leave it high without a smaller model refusing the
  request; that model simply uses as much of it as it can.
- **Temperature** — how much randomness. `0` is focused and repetitive; higher values are more
  creative and more unpredictable. For code and factual work, keep it low.
- **topP** — another randomness dial that limits the model to its most likely word choices. Most
  people should leave this alone and adjust only Temperature.

**Guardrails** are an AWS feature that blocks or masks content you do not want — profanity, personal
information, particular topics. You create the guardrail in the AWS console, then enter its
**Guardrail Identifier** and **Version** here. **Trace** logs why something was blocked, which is
useful while you are tuning it.

### 5.4 Chat tab

| Setting                           | What it does                                                                                                                                                                                                                                                                                    |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Context Length**                | How many past messages get sent along with each new one. Higher means the agent remembers more of the conversation, but every message costs money to send, so long context costs more. The default of 10 is fine for most work; raise it if the agent keeps forgetting things you said earlier. |
| **Request Timeout (minutes)**     | How long to wait before giving up on a reply. Raise it if you ask for long, tool-heavy jobs that legitimately take a while.                                                                                                                                                                     |
| **Enable Prompt Cache**           | Lets AWS remember the unchanging front part of your conversation so it does not have to be re-processed every turn. This makes replies faster and cheaper. Leave it on.                                                                                                                         |
| **Tavily Search API Key**         | Required for the web search tool. Get a free key from [tavily.com](https://tavily.com/). Without it, `tavilySearch` will not work.                                                                                                                                                              |
| **Advanced → Enter key behavior** | Choose whether `Enter` sends your message (and `Shift+Enter` makes a new line), or the other way round. Pick whichever matches the other apps you use.                                                                                                                                          |

### 5.5 Workspace tab

| Setting               | What it does                                                                                                                                                                                            |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Project Directory** | The folder the agent works in. All file tools are anchored here, and attachments and Docker sandboxes are stored inside it. Choose a real project folder, or a scratch folder if you are experimenting. |
| **Config Directory**  | Where the app keeps its own settings and chat history. Shown so you can find it; see [Section 19](#19-where-the-app-keeps-your-files).                                                                  |

---

## 6. The Chat page

This is where most work happens.

![chat-conversation](../assets/chat-conversation.png)

### 6.1 The layout

```
┌──────────────────────────────────────────────────────────────┐
│ [history]     Chat title              [TODO] [$cost] [tokens]│  top bar
├───────────┬──────────────────────────────────────────────────┤
│           │                                                  │
│  Chat     │   Your messages and the agent's replies           │
│  history  │   (tool calls shown inline, collapsible)          │
│  list     │                                                  │
│  📎 🐳    │                                                  │
│           │                                                  │
├───────────┴──────────────────────────────────────────────────┤
│ [MD] [Word] [PDF] [paperclip] [whale]        [stop] [new]    │  toolbar
├──────────────────────────────────────────────────────────────┤
│ Type your message here…                                      │  input box
│                                                              │
│ [Agent ▾] [Model ▾] [Thinking ▾] [Plan/Act] [tools] [folder] │  controls
└──────────────────────────────────────────────────────────────┘
```

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
gives better answers on hard problems, but costs more and takes longer.

| Thinking setting | When to use it                                                                                                                                                                        |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **None**         | Simple questions, quick lookups, chatting.                                                                                                                                            |
| **Adaptive**     | Let the model decide how hard to think. A good default on newer models.                                                                                                               |
| **Quick** (1K)   | A little planning.                                                                                                                                                                    |
| **Normal** (4K)  | Everyday multi-step work.                                                                                                                                                             |
| **Deep** (16K)   | Hard debugging, tricky analysis, long plans.                                                                                                                                          |
| **Deeper** (32K) | The hardest problems. On models that use effort levels instead of a thinking budget (Grok 4.7, Grok 4.6, GPT-6.1, GPT-6, GPT-5.6), this asks for the highest effort the model offers. |

Some models always think and cannot be turned down; the control will tell you when that is the case.

There is also an **Interleave** toggle. When on, you see the model's reasoning woven into its answer
as it works, instead of only the final result. Handy for understanding _why_ it did something.

### 6.3 Plan mode and Act mode

The **Plan / Act** toggle next to the input box (or `Cmd`/`Ctrl` + `Shift` + `A`) controls whether
the agent is allowed to change anything.

- **Plan mode** — only read-only tools work. The agent can look at your files, search the web and
  think out loud, but it cannot write, move or delete anything, and it cannot run commands.
- **Act mode** — every tool the agent has is available.

Use Plan mode when you want advice, a review, or a proposal before anything happens. Switch to Act
mode when you are happy with the plan.

### 6.4 Sending a message

Type in the box and press `Enter` (or `Cmd`/`Ctrl`+`Enter`, depending on your
[Advanced setting](#54-chat-tab)).

While the agent works you will see tool calls appear in the conversation — a line for each tool it
used, which you can expand to see exactly what it did and what came back. This is the single most
useful thing to read when an answer surprises you.

The **stop** button is red while a reply is being generated. It stops only the conversation you are
looking at.

### 6.5 Answers keep running when you look away

If you switch to a different chat, start a new chat, or leave the Chat page entirely, the agent keeps
working. It carries on calling tools, and the finished reply is waiting when you come back.

Chats still working show **"Still responding"** in the history list on the left.

The one thing that does cancel everything is reloading or closing the window.

### 6.6 The top-bar buttons

- **TODO list** — for long jobs, the agent can keep a checklist of what it plans to do and tick items
  off. This button shows that list. If the agent is not using a checklist, the list is empty.
- **Conversation cost** — a running total in dollars for this conversation.
- **Token analytics** — a detailed breakdown: how many tokens went in, came out, were served from
  cache, and what each turn cost. Open this if a conversation feels expensive and you want to know
  why.

### 6.7 Exporting a conversation

Three buttons above the input box export the current conversation:

| Format               | Best for                                 | Notes                                                                                                                                                                                                 |
| -------------------- | ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Markdown** (`.md`) | Keeping notes, putting in a repo, GitHub | Mermaid diagrams stay as editable `mermaid` code blocks, so GitHub, VS Code and Obsidian draw them for you. Draw.io diagrams and pasted images are written into an `images/` folder next to the file. |
| **Word** (`.docx`)   | Sharing with colleagues, editing further | Every diagram is rendered as a picture, in the same greyscale used on screen.                                                                                                                         |
| **PDF**              | Sending something that will not change   | Every diagram is rendered as a picture.                                                                                                                                                               |

All three exports leave out tool calls and tool results, so you get the readable conversation rather
than the machinery. Each turn is labelled — `Assistant – <model name>` and `User – <your name>` — and
your avatar and the model's logo are embedded.

### 6.8 Copying part of a message

Select any text inside a message and a small floating toolbar appears. It copies just what you
selected, either as **Markdown** (plain text with `**bold**` style markers) or as **rich text**
(formatting preserved, for pasting into Word or email). You can also copy a whole message either way.

### 6.9 Chat history

The panel on the left lists your past conversations, newest first, and opens automatically when you
go to the Chat page. Click one to reopen it — everything is still there.

You can select several at once and delete them together. Deleting a chat also deletes that chat's
[attachments folder](#7-attachments-giving-the-agent-your-files) and its
[Docker sandbox container](#10-the-docker-sandbox).

Chat titles are generated for you from the first thing you say, using the
[Light Processing Model](#53-models-tab). A title is always short plain text — a handful of words, no
formatting — and is trimmed to fit the sidebar. In the history panel, a chat's ⋮ menu offers **Edit
title** to type your own and **Generate title** to ask for a new one; **Generate All Titles** at the top
of the panel redoes every chat in the list.

### 6.10 The folder button

Next to the tool settings there is a folder button showing your current project directory. Click it
to switch project folders for this conversation without going into Settings.

---

## 7. Attachments: giving the agent your files

Attachments in this app are **real files in a real folder**, not text pasted into a message. That
one design choice explains everything else about how they behave.

### 7.1 Adding files

Three ways, all equivalent:

- **Drag and drop** a file onto the chat.
- **Paste** a file or an image from your clipboard.
- Click the **paperclip** button (next to the export buttons) and choose **Add files**.

The moment you do, the file is copied into your project folder at:

```
<your project folder>/attachments/<chat-title>-<id>/
```

The folder is named after the chat, and gets renamed automatically when the chat title changes —
including when a title is generated for you.

### 7.2 Managing files

The paperclip button always shows a badge with how many files this chat has. Its menu lists each
file with its size, and lets you:

- **Delete** a single file on the spot
- **Add** more files through a file picker
- **Open the folder** in Finder, File Explorer, or your Linux file manager

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

Very long files are trimmed, with a note telling the agent to read the rest itself using `readFiles`.

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

- **Create** a new agent, **edit** an existing one, **duplicate** one as a starting point, or
  **remove** one.
- **Move agents in and out as files.** **Import Agent**, at the top next to Add New Agent, reads an
  agent file someone sent you. **Download YAML**, in each agent's **⋮** menu, writes an agent out to
  a file. See [8.5](#85-sharing-an-agent) and [8.6](#86-importing-an-agent).
- **Card view or table view** — toggle with the button at the top. Table view is easier for sorting
  and scanning when you have a lot of agents.

![my-agents-table](../assets/my-agents-table.png)

- **Rearrange by dragging.** The order you set is saved and reused everywhere agents are listed —
  the Agent dropdown in chat and `@` mentions. Dragging is switched off while a table column sort is
  active, because two orderings at once makes no sense.
- **Hide agents you do not use.** The app ships with several built-in agents. You cannot delete those
  outright, because they get restored at every launch, but **Hide** takes them out of every list and
  the setting sticks across restarts. The **Unhide** dropdown lists everything you have hidden, so
  you can bring back one agent, or all of them.
- Agents **you** created are deleted for real, and the button says **Delete** rather than **Hide**.

### 8.3 The agent editor

![agent-editor](../assets/agent-editor.png)

Clicking an agent opens its editor, which has three tabs.

#### Tab 1 — Basic Settings

**Name & Icon.** The icon button opens a picker. It starts with a curated, categorised list, and adds
ten full icon collections you can browse or search: Tabler, Lucide, Phosphor, Material, Heroicons,
Bootstrap, Font Awesome (including brand logos), Simple Icons and Game Icons — roughly 38,000 icons
in total. Choose **All libraries** to search all of them at once. Long result lists are capped, so if
you do not see what you want, search a more specific word. Collections load the first time you open
one, so this does not slow the app's startup.

**Description.** A sentence about what this agent is for. This is shown in lists, and is also used
when suggesting MCP servers.

**System Prompt.** The most important field. See [Section 8.4](#84-writing-a-good-system-prompt).

**Scenarios.** Example prompts shown as clickable starters when you open a new chat with this agent.
Good scenarios make an agent much easier for someone else to pick up.

**Tags.** Labels for finding this agent later.

**Knowledge Bases.** If you have an Amazon Bedrock Knowledge Base — a searchable collection of your
own documents — list its ID here. The `retrieve` tool can then search it.

**Bedrock Agents.** IDs of Amazon Bedrock Agents you want this agent to be able to call, via
`invokeBedrockAgent`.

**Flows.** Amazon Bedrock Flows this agent may run, via `invokeFlow`.

**Allowed Commands.** A safety list for the `executeCommand` tool. Only commands matching a pattern
here can be run on your computer. `*` is a wildcard, so `npm *` allows every `npm` command and
nothing else. If this list is empty, the agent cannot run anything on your machine at all.

> **Safety note:** every pattern you add here is a command the AI can run on your computer without
> asking you first. Add specific patterns (`git status`, `npm test`) rather than broad ones. Never
> add a bare `*`. If you want the agent to be able to install packages and run anything it likes,
> use the [Docker sandbox](#10-the-docker-sandbox) instead — that keeps it away from your machine
> entirely.

#### Tab 2 — MCP Servers

Connect external tool servers. See [Section 11](#11-mcp-servers-adding-tools-from-outside-the-app).

#### Tab 3 — Tools

![select-tools](../assets/select-tools.png)

Tick the tools this agent may use, grouped into categories: File System, Web & Search, AI Services,
System, Thinking, Agent Delegation, and MCP. Every tool is documented in
[Section 9](#9-the-complete-tool-reference).

The rule of thumb: **give an agent the fewest tools it needs.** Fewer tools means less to go wrong,
lower cost per message (the tool list is sent with every request), and a model that stays focused.

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

| Placeholder           | Replaced with                     |
| --------------------- | --------------------------------- |
| `{{projectPath}}`     | Your current project folder       |
| `{{date}}`            | Today's date                      |
| `{{allowedCommands}}` | The commands this agent may run   |
| `{{knowledgeBases}}`  | The knowledge bases it can search |
| `{{bedrockAgents}}`   | The Bedrock agents it can call    |
| `{{flows}}`           | The flows it can run              |
| `{{allowedWindows}}`  | Windows it may screenshot         |
| `{{allowedCameras}}`  | Cameras it may use                |

**Helpers in the editor:**

- **Generate system prompt** — describe what you want in a sentence and let the model draft the
  prompt for you. A great starting point that you then edit.
- **Show preview** — see the prompt with all placeholders filled in and all automatic additions
  included, exactly as the model will receive it. Use this when an agent behaves unexpectedly.
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

A shared agent loaded from a project file is edited by editing that file. If you want to change it
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
  Q -->|"search my company's<br/>own documents"| KB["<b>AI Services</b><br/>retrieve"]
  Q -->|"run a program,<br/>script, or command"| SYS["<b>System</b><br/>executeCommand, codeInterpreter,<br/>dockerSandbox"]
  Q -->|"see my screen<br/>or camera"| CAP["<b>System</b><br/>screenCapture, cameraCapture"]
  Q -->|"stop and reason,<br/>or track a long job"| TH["<b>Thinking</b><br/>think, todo"]
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

| Tool                 | What it does                                                                                                                                                                                                                                                                                                                        | Notes                                                                                                                                                                           |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `generateImage`      | Makes an image from a text description. Defaults to Stability SD 3.5 Large, and also supports Amazon Nova Canvas and Titan Image Generator. Choose aspect ratio and size; output as PNG, JPEG or WebP. Supports a **negative prompt** (things to leave out) and a **seed** (the same seed and prompt gives the same picture again). | Only offered in regions that have image models: `us-east-1`, `us-west-2`, `ap-northeast-1`, `eu-west-1`, `eu-west-2`, `ap-south-1`. Which models you get depends on the region. |
| `recognizeImage`     | Looks at an image file and describes it: objects, text in the picture, the scene, a caption.                                                                                                                                                                                                                                        | Good for pulling text out of screenshots, describing images for accessibility, or tagging a folder of photos.                                                                   |
| `generateVideo`      | Makes a video with Amazon Nova Reel, from text or from a starting image. Modes: `TEXT_VIDEO` (6 seconds), `MULTI_SHOT_AUTOMATED` (12–120 seconds), `MULTI_SHOT_MANUAL`.                                                                                                                                                             | Video takes minutes, so this returns a job reference immediately rather than waiting. Requires an S3 bucket configured.                                                         |
| `checkVideoStatus`   | Checks whether a video job has finished.                                                                                                                                                                                                                                                                                            | Use the job reference from `generateVideo`.                                                                                                                                     |
| `downloadVideo`      | Downloads a finished video from S3 to your project folder.                                                                                                                                                                                                                                                                          | Only works once `checkVideoStatus` reports "Completed".                                                                                                                         |
| `retrieve`           | Searches an Amazon Bedrock Knowledge Base — a collection of your own documents that AWS has indexed.                                                                                                                                                                                                                                | Needs a knowledge base ID listed on the agent. This is how you get an agent to answer from _your_ documents rather than from general knowledge.                                 |
| `invokeBedrockAgent` | Calls an Amazon Bedrock Agent you built in the AWS console, and returns its answer. Keeps a session ID so a conversation can continue across calls.                                                                                                                                                                                 | Needs the agent ID and alias ID on the agent config.                                                                                                                            |
| `invokeFlow`         | Runs an Amazon Bedrock Flow — a data-processing pipeline you designed in AWS. Accepts text, numbers, true/false, objects and lists as input.                                                                                                                                                                                        | Needs the flow configured on the agent. Useful for multi-step processing you have already built elsewhere and do not want to rebuild in a prompt.                               |

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

Gives the chat its own long-lived Linux container to work in. It also brings the sandbox panel, which
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

| Tool                                      | What it does                                                                                                                                    | When it helps                                                                                                                                                         |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `think`                                   | Gives the model a scratchpad to reason in the middle of a task, without changing any data or fetching anything.                                 | Multi-step problems, decisions with trade-offs, checking its own work against rules it was given. It costs a little and often saves a lot by preventing a wrong turn. |
| `todo` (with `todoInit` and `todoUpdate`) | Lets the agent write down a checklist for a long job and tick items off as it goes. You can watch the list from the TODO button in the top bar. | Jobs with many steps, where you want to see progress and be sure nothing was skipped.                                                                                 |

These are different from the **Thinking** dropdown. The dropdown controls private reasoning before
the model starts answering; the `think` tool is something it can reach for partway through.

### 9.7 Agent Delegation

| Tool          | What it does                                                                                                                                                                                        |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `invokeAgent` | Hands one step of the work to a different agent of yours, which runs with its own system prompt and its own tools, and returns its result. This is also what an `@mention` in the message box uses. |

Full explanation in [Section 12](#12-handing-work-to-another-agent-with-).

### 9.8 MCP tools

Any tool provided by an MCP server you have connected shows up in this category, named after the
server's own tool names. See [Section 11](#11-mcp-servers-adding-tools-from-outside-the-app).

### 9.9 Default tool sets

When you create an agent you can start from a preset rather than ticking boxes one at a time:

| Preset       | Tools it turns on                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------------------ |
| **General**  | `readFiles`, `listFiles`, `tavilySearch`, `fetchWebsite`, `think`                                      |
| **Coding**   | The file tools including `applyDiffEdit`, plus `tavilySearch`, `executeCommand`, `invokeFlow`, `think` |
| **Design**   | `readFiles`, `listFiles`, `fetchWebsite`, `tavilySearch`, `generateImage`, `think`                     |
| **Data**     | `readFiles`, `listFiles`, `tavilySearch`, `fetchWebsite`, `retrieve`, `executeCommand`, `think`        |
| **Business** | `readFiles`, `listFiles`, `tavilySearch`, `fetchWebsite`, `retrieve`, `think`                          |
| **All**      | Everything in the list above, combined                                                                 |
| **Custom**   | Nothing — you choose                                                                                   |

---

## 10. The Docker sandbox

### 10.1 The problem it solves

An agent that can run commands is enormously more useful — it can install a library, run your tests,
try an idea. But an agent that can run commands _on your computer_ is also a risk, which is why the
allowed-commands list exists, and why that list keeps the agent boxed in.

The Docker sandbox removes the trade-off. It gives the chat its own Linux computer to make a mess in.

### 10.2 How it works

Turn on the **Docker Sandbox** tool for an agent. Then, the first time that agent runs a command in a
chat, the app creates a container for that chat based on `ubuntu:26.04`, and from then on commands
go there by default.

```mermaid
flowchart LR
  subgraph Your["Your computer"]
    P["Project folder"]
    D["docker-sandboxes/&lt;chat&gt;/"]
    H["Everything else on<br/>your machine"]
  end
  subgraph C["The chat's container (ubuntu:26.04)"]
    WS["/workspace<br/>(your project, read+write)"]
    DATA["/data<br/>(saved on your machine)"]
    PKG["Anything the agent<br/>installs — apt, pip, npm"]
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

A **whale icon** appears in the chat toolbar whenever the current chat has a container. Its menu can:

- **Open the sandbox panel** (see [10.8](#108-the-sandbox-panel))
- **Stop**, **start** or **remove** the container
- **Open the sandbox folder** in Finder, File Explorer, or your Linux file manager

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
  for your platform.
- **Docker Compose** is used when available, and is required for stacks with more than one service.
- **The terminal needs Docker to be local.** Docker Desktop, OrbStack, Colima, Rancher Desktop and
  rootless installs all work. If your Docker context points at a remote daemon, the Terminal tab is
  disabled and explains why; everything else in the panel still works.

### 10.8 The sandbox panel

The panel slides in from the right-hand edge of the chat. Open it from the whale menu, or with the
narrow tab on the right — the mirror image of the chat history's tab on the left. It only appears for
chats that actually have a container, and it starts closed.

Three tabs:

**Overview** — what the container is and what it is doing: its name and image, how long it has been
up, and CPU, memory, network and disk against the limits from
[Settings → Tools → Docker Sandbox](#the-docker-sandbox-tool). Each service is listed with its state,
and each published port is a button that opens `localhost:<port>` in your browser. The usual actions
— open folder, start, stop, restart, remove — are at the bottom.

CPU is shown the way `docker stats` shows it: 100% means one core fully used, so a container working
across two cores reads 200%. It says _measuring…_ for the first few seconds after you open the tab,
because a percentage needs two readings to compare.

**Compose** — only for sandboxes that use Docker Compose. A diagram of the stack: which services
exist, what image each runs, whether it is up, which published ports reach it from your browser
(solid arrows) and which folders are mounted into it (dashed). Services in one stack can reach each
other by service name, which the diagram notes, because that is the detail people most often miss.
Click the diagram to open it full window, with zoom controls and Esc to close — worth it for a stack
of several services, since the panel itself is only so wide. Below the diagram is the compose file
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
its own shell and its own scrollback; a dot shows which are running. It is a proper terminal: colours, `vim` and `htop`, tab completion, command history,
Ctrl-C, and it resizes with the panel.

This is the quickest way to see what the agent actually left behind, or to fix something yourself
without describing it to the model first.

**Read this once, then it will stop telling you:**

- `/workspace` is your real project folder, mounted read-write. A `rm -rf` typed here deletes real
  files. There is no undo and no confirmation.
- Nothing you type is checked against the allowed-commands list. That list exists to protect your
  machine from the model; this shell is you, in a container, on purpose.
- The shell runs as **root**, so files it creates may end up owned by root on Linux.

The app asks you to acknowledge that once, before the first time you open a terminal, and then
remembers it.

**The model cannot use this.** There is no tool for it, so agents, sub-agents and background tasks
have no way to open a shell or type into yours. It is reachable only from this panel.

A shell you open keeps running while you look at other tabs, collapse the panel, or switch chats — so
a dev server you started by hand does not die when you look away. Switching between service tabs is
free for the same reason: the shell lives outside the panel, and its recent output is replayed when
you come back. It ends when the sandbox is stopped
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

Once a server connects, switch to the **Tools** tab and its tools will be listed there for you to
enable.

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

| Field                       | What to put in it                                                                                                                                                             |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Task Name**               | Something you will recognize in the list.                                                                                                                                     |
| **Agent**                   | Which agent runs.                                                                                                                                                             |
| **Model**                   | Which model it uses. Scheduled tasks are a good place to use a cheaper model.                                                                                                 |
| **Max Tokens**              | The longest reply allowed.                                                                                                                                                    |
| **Project Directory**       | Which folder it works in.                                                                                                                                                     |
| **Wake Word**               | The message sent to the agent when the task runs. This is the actual instruction — for example _"Summarize commits from the last 24 hours and write it to daily-summary.md"_. |
| **Schedule**                | When to run, as a cron expression. Presets are provided.                                                                                                                      |
| **Continue Session**        | Whether each run continues the same conversation or starts fresh.                                                                                                             |
| **Continue Session Prompt** | The message used on continuing runs, if different from the wake word.                                                                                                         |
| **Enable Task**             | Turn the schedule on or off without deleting the task.                                                                                                                        |

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

The app provides these as presets, so you rarely need to write one by hand.

### 14.3 Watching and running tasks

- **Run now** executes a task immediately, without waiting for the schedule. Use this to test.
- **Execution history** shows every past run, what it did, and how long it took. Open it from the
  task, and close it when you are done.
- **Notifications** (if enabled in Settings → General) tell you when a scheduled run finishes or
  fails.
- Inside a task's history you can also send a follow-up instruction into that session.

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

| If you want…                                             | Use                                                     |
| -------------------------------------------------------- | ------------------------------------------------------- |
| A good all-round default                                 | **Claude Sonnet 5.5**                                   |
| The strongest reasoning for hard problems                | **Claude Opus 5.5** or **Claude Fable 5.1**             |
| Speed and low cost for simple work                       | **Claude Haiku 4.5** or **Amazon Nova Lite**            |
| The cheapest option for background tasks and chat titles | **Amazon Nova Micro** or **Nova Lite**                  |
| A very large context window for huge documents           | **Claude Fable 5.1** (1M tokens) or **Grok 4.7** (500K) |
| OpenAI's most capable model, cost no object              | **GPT-6 Astra** (1.05M tokens, $11/$55 per 1M)          |
| Near-Astra quality at a mid-tier OpenAI price            | **GPT-6.1 Sol** (1M tokens, $2.20/$11 per 1M)           |
| A mid-priced OpenAI model for everyday work              | **GPT-6 Sol** ($2/$10 per 1M)                           |
| The cheapest OpenAI model for high-volume work           | **GPT-6 Luna** ($0.10/$0.50 per 1M)                     |

The model dropdown shows the price per million tokens in and out for each model, so you can compare
before you commit.

### 18.2 The full list

Which of these you see depends on your region and your **Visible Models** allowlist.

**Anthropic (Claude):** Haiku 4.5, Sonnet 4, Sonnet 4.5, Sonnet 4.6, Sonnet 5, Sonnet 5.5, Opus 4,
Opus 4.1, Opus 4.6, Opus 4.7, Opus 4.8, Opus 5, Opus 5.5, Fable 5, Fable 5.1

**Amazon (Nova):** Nova Micro, Nova Lite, Nova 2 Lite, Nova Pro, Nova Premier

**OpenAI:** GPT-6 Astra, GPT-6.1 Sol, GPT-6 Sol, GPT-6 Luna, GPT-5.6 Sol, GPT-5.6 Terra, GPT-5.6 Luna,
GPT-OSS 20B, GPT-OSS 120B

**Others:** DeepSeek R1, Kimi K2.5, Grok 4.6, Grok 4.7

**Image models** (used by `generateImage`): Amazon Nova Canvas, Amazon Titan Image Generator v1 and
v2, Stability SD3 Large, SD3.5 Large, Stable Image Core v1.0 and v1.1, Stable Image Ultra v1.0 and
v1.1

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
2. **Set a cheap Light Processing Model.** Chat titles and suggestions do not need your best model.
3. **Give agents fewer tools.** The tool list is sent with every request.
4. **Turn thinking down for simple work.** Reasoning tokens are billed.

Watch the **conversation cost** and **token analytics** buttons in the chat top bar to see where your
money is going.

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
```

The exact path for your machine is shown in **Settings → Workspace → Config Directory**.

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

| What                                                 | Goes where                                                       |
| ---------------------------------------------------- | ---------------------------------------------------------------- |
| Your messages, and the file contents the agent reads | Amazon Bedrock, in your AWS account and chosen region            |
| Web search terms                                     | Tavily                                                           |
| A URL you ask it to fetch                            | That website                                                     |
| Image, video and knowledge base requests             | Amazon Bedrock / your AWS account                                |
| Anything an MCP server does                          | Wherever that MCP server sends it — check before you install one |
| Everything else                                      | Nowhere. The app has no telemetry and no server of its own.      |

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
- _…see what this conversation is costing me?_ → [Section 6.6](#66-the-top-bar-buttons)
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

**Tools**

- _…let the agent search the web?_ → `tavilySearch` plus a Tavily key,
  [Section 9.3](#93-web--search-tools)
- _…let the agent run commands safely?_ → [Section 10](#10-the-docker-sandbox)
- _…see what the agent is doing inside its container?_ → the sandbox panel,
  [Section 10.8](#108-the-sandbox-panel)
- _…get a shell inside the container myself?_ → [Section 10.9](#109-the-interactive-terminal)
- _…let the agent run commands on my actual machine?_ → Allowed Commands,
  [Section 8.3](#83-the-agent-editor) — and read the safety note
- _…let the agent analyze a spreadsheet?_ → `codeInterpreter`, [Section 9.5](#95-system-tools)
- _…let the agent search my company's documents?_ → `retrieve` plus a knowledge base,
  [Section 9.4](#94-ai-services-tools)
- _…make an image?_ → `generateImage`, [Section 9.4](#94-ai-services-tools)
- _…make a video?_ → `generateVideo`, [Section 9.4](#94-ai-services-tools)
- _…show the agent my screen?_ → `screenCapture`, [Section 9.5](#95-system-tools)
- _…add a tool the app does not have?_ → MCP,
  [Section 11](#11-mcp-servers-adding-tools-from-outside-the-app)
- _…find an MCP server for a service I use?_ → [Section 11.2](#112-finding-servers)

**Automation**

- _…run something every morning?_ → [Section 14](#14-background-agent-work-on-a-schedule)
- _…write a cron schedule?_ → [Section 14.2](#142-cron-expressions-briefly)
- _…see whether a scheduled task worked?_ → [Section 14.3](#143-watching-and-running-tasks)

**Building things**

- _…build a web page?_ → [Section 15](#15-website-generator)
- _…draw an AWS architecture diagram?_ → [Section 16](#16-diagram-generator)
- _…build a Step Functions workflow?_ → [Section 17](#17-step-functions-generator)

**Cost and models**

- _…spend less money?_ → [Section 18.5](#185-managing-cost)
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

**Cron expression** — five values describing a repeating schedule. See
[Section 14.2](#142-cron-expressions-briefly).

**Docker** — the program that runs containers. Needed for `codeInterpreter` and `dockerSandbox`.

**Guardrail** — an AWS feature that filters or blocks content according to rules you define.

**IAM** — AWS's permissions system. An IAM policy is a document saying what your account may do.

**Inference profile** — a Bedrock routing option that decides which regions can serve your request,
and can also carry tags for cost tracking.

**Knowledge base** — a collection of your own documents that AWS has indexed so an AI can search it.
Reached with the `retrieve` tool.

**MCP (Model Context Protocol)** — an open standard for programs that supply tools to an AI. See
[Section 11](#11-mcp-servers-adding-tools-from-outside-the-app).

**Model** — the AI itself. Claude, Nova and Grok are models.

**Model access** — the per-region switch in the Bedrock console that decides which models your AWS
account may call at all. Separate from IAM permissions, and both have to be in place. See
[Section 4.2](#42-step-1--turn-on-the-models-you-want-in-the-bedrock-console).

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

Raise **Context Length** in **Settings → Chat**. Be aware this makes each message more expensive.

### The agent forgot a rule I gave it in the system prompt

Use **Show preview** in the agent editor's system prompt section to see exactly what the model
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
  `docker context use default` (or whichever local context you have) and reopen the panel.
- **"No Docker socket at …"** or **"Permission denied …"** — Docker is not running, or your user
  cannot open its socket. Start Docker, and on Linux add yourself to the `docker` group
  (`sudo usermod -aG docker $USER`, then log out and back in).

Everything else in the panel — state, services, ports and the activity log — keeps working either way.

### Web search does not work

The `tavilySearch` tool needs an API key. Get a free one from [tavily.com](https://tavily.com/) and
put it in **Settings → Chat → Tavily Search API Key**.

### Image generation is not available

Image models only exist in some regions: `us-east-1`, `us-west-2`, `ap-northeast-1`, `eu-west-1`,
`eu-west-2`, `ap-south-1`. Switch region, or use a different approach.

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

Open **token analytics** in the chat top bar to see where. Then work through
[Section 18.5](#185-managing-cost) — Context Length and the Light Processing Model are usually the
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

You do not have to read this guide to use it. The **Help** button — the question mark at the very
bottom of the sidebar, just above the GitHub link — opens a chat that has this whole guide in front
of it, so you can ask in your own words:

- "Where do I put my AWS keys?"
- "What's the difference between Plan and Act mode?"
- "Which tool lets an agent look something up on the web?"
- "Why is my chat costing so much?"

### 23.1 What the help chat is

It is an ordinary chat, on the Chat page, using an agent called **Bedrock Engineer Help**. It shows
up in your chat history like any other conversation, and you can export it, rename it or delete it
the same way.

Two things make it different:

- **The guide is already attached.** The version of the guide that shipped with your build is
  attached to the chat as a file, so the answers match the app you are actually running. Nothing is
  downloaded, and it works with no internet connection.
- **It has no tools.** It cannot read your files, run commands, search the web or look at your
  settings. It answers from the guide and nothing else. That is deliberate: a help button should not
  be able to touch your machine. It also means that when the guide does not cover your question, it
  will tell you so instead of guessing — and if answering needs something checked on your computer,
  it will tell you what to look at rather than looking itself.

### 23.2 Which model it uses

The help chat runs on whichever model you selected as the **Light Processing Model** in
**Settings → Models** (see [Section 5.3](#53-models-tab)). The guide is long, so every question
carries a lot of text with it; a small, cheap model like Haiku or Nova handles that comfortably and
keeps the cost low. If you have not chosen a Light Processing Model, the help chat uses your main
conversation model instead.

The agent and model shown at the bottom of a help chat are displayed as plain text rather than
dropdowns, because the help chat pins both and the pickers would not be able to change them.

### 23.3 Reopening it

Clicking **Help** again returns to the same help conversation, so you can carry on where you left
off. To start a fresh one, use the **New chat** button inside the help chat — that gives you a normal
chat, then click **Help** again.

When the app is updated, the attached guide is replaced with the new version the next time you open
Help, so it never goes stale.

### 23.4 If you have not set a project directory

Attachments are saved into your project directory, so without one the guide cannot be attached as a
file. Help still works: the guide is handed to the agent directly instead, and a short notice
explains why no attachment is listed. Setting a project directory
([Section 5.5](#55-workspace-tab)) makes the attachment appear on the next open.

### 23.5 When to use the help chat, and when not to

Use it for "how does this app work" questions. It is the fastest way to find a setting, understand
what a tool does, or work out which page you need. It is also a good way through first-time AWS
setup, because [Section 4](#4-setting-up-access-to-amazon-bedrock) walks that whole process through
step by step — the help chat can talk you along it and tell you which step a given error belongs to.

Do not use it for work — it has no tools, so it cannot write files, run anything or search. And it
only knows what is in this guide, so it cannot answer general questions about Amazon Bedrock, AWS or
the models beyond what Sections 4 and 19 cover, and it cannot look at your AWS account or run the
verification commands for you. For anything wider, ask a normal agent with the web search tool
enabled, or read the AWS documentation.
