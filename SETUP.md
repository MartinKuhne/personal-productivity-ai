# Setup

## Warnings

- This is an unfinished project, under development. All software has bugs.
- AI and LLMs are new technology
- This is a project for developers and power users

## Installation

```
cargo install --git https://github.com/MartinKuhne/personal-productivity-ai
```

## Setup

FastMD uses a YAML configuration file for its internal settings, AI agent configurations, and external API integrations (such as JMAP and SearXNG). 

On Windows, the preferred location is `%AppData%\fastmd\config.yaml`

The application searches for the configuration file in the following order:
1. `%APPDATA%\fastmd\config.yaml`
2. `%USERPROFILE%\.fastmd.yaml`
3. `.fastmd.yaml` (in the current working directory)

If no configuration file is found, a default one will be automatically created at the first available location.

### Setting up a library

FastMD is centered around one or more libraries of markdown formatted notes. Set them up like this

```yaml
content_libraries:
  - name: "Notes"
    root_folder: "C:\\path\\to\\your\\workspace"
    kind: "text"
    readonly: false
  - name: "Reference"
    root_folder: "C:\\path\\to\\reference\\docs"
    kind: "text"
    readonly: true
```

### LLM

FastMD requires an OpenAI compatible chat endpoint.
It provides no facility to run a LLM locally. It is strongly recommended to self-host if you use it as your personal information base to protect your PII.
If you are just getting started, [LM studio]{https://lmstudio.ai/) is a reasonably easy way to run models locally.
As of summer 2026 I've tested FastMD with the _google/gemma-4-12B_ on a 16GB VRAM GPU with good success. My daily driver is [Qwen/Qwen3.6-35B-A3B](https://huggingface.co/Qwen/Qwen3.6-35B-A3B)

[Openrouter](https://openrouter.ai/) offers free and pay-per-use models.

Set up your LLMs as follows:

```yaml
models:
  gpt4:
    model: "openai/gpt-4"
    api_url: "https://api.openai.com/v1"
    api_key: "your-openai-key"
  claude:
    model: "anthropic/claude-3-opus"
    api_url: "https://api.anthropic.com/v1"
    api_key: "your-anthropic-key"
```

There is a cost field and it will prefer the lowest cost model. I promise to update the documentation later.

### Web search

Web search is not required, however it makes the application a lot more useful. Web search turned out a hard problem to solve,
as search engines want you to see the additional content they provide with their search results. 

FastMD supports SearXNG to perform web searches. To my disappointment, SearXNG doesn't know how to get around _bot blocking_ either.
It does provide a framework for you to configure one or more providers to do so.

Once you have SearXNG established, configure it as follows

```yaml
searxng_url: http://localhost:3001
```

### E-Mail

A JMAP speaking host can be added like so

```yaml
jmap_clients:
  work:
    url: "https://api.fastmail.com/jmap/api"
    token: "your-fastmail-token"
```

### DAV (Calendar and contacts)

Uses one config section for CalDAV and CardDAV

```yaml
caldav_clients:
  personal:
    url: "https://caldav.fastmail.com/"
    username: "you@fastmail.com"
    password: "app-password"
```

### ToDo lists

FastMD can talk to Trello to maintain To-Do lists

```yaml
trello_client:
  token: do.not.share
  api_key: my.key
```

### PDF conversion

When provided with a PDF-to-Markdown conversion command, FastMD will create Markdown files for every PDF file it encounters.
That in turn will make the PDF files' content available to the AI agent (and yourself)

```yaml
pdf_converter_command:
- marker_single
- '{input}'
- --output_dir
- '{output}'
- --output_format
- markdown
- --disable_image_extraction
```

### Other options

The `config.yaml` file supports the following options:

| Option | Type | Default Value | Description |
|--------|------|---------------|-------------|
| `user_name` | String (Optional) | `null` | The name of the user. |
| `user_address` | String (Optional) | `null` | The address of the user. |
| `user_birthdate` | String (Optional) | `null` | The birthdate of the user. |
| `user_gender` | String (Optional) | `null` | The gender of the user. |
| `system_prompt_extension` | String (Optional) | `null` | Additional text to append to the AI system prompt. |
| `inline_editor_enabled` | Boolean | `false` | Enable the built-in inline text editor. |
| `csv_db_path` | String (Optional) | `null` | Override the default storage location for CSV databases. |
