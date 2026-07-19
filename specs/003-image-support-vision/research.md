# Research & Decisions

## 1. Base64 Encoding Dependency
- **Decision**: Add the `base64` crate to `Cargo.toml`.
- **Rationale**: The standard library does not include base64 encoding. The `base64` crate is the de facto standard in the Rust ecosystem, is highly performant, and is reliable.
- **Alternatives considered**: Manually implementing a base64 encoder (prone to errors, unnecessary work).

## 2. Vision API Request Format
- **Decision**: Update `agent.rs` or create a new API client in `background::vision_processor` to send the OpenAI-compatible vision payload. The `content` field will be an array containing a text request ("Describe this image in Markdown...") and an `image_url` object containing the `data:image/jpeg;base64,...` URI.
- **Rationale**: The application already uses an OpenAI-compatible schema for chat, and the vast majority of vision models on OpenRouter (the default API) expect this exact format.
- **Alternatives considered**: Implementing separate payloads for different providers. Rejected because OpenRouter normalizes the vision payloads to the OpenAI schema.

## 3. Background Processing Strategy
- **Decision**: Create a `background::vision_processor` module that processes image files asynchronously. Since `ureq` is a blocking HTTP client (which is already used by `agent.rs`), the API calls will be wrapped in `tokio::task::spawn_blocking` to prevent blocking the background Tokio event loop.
- **Rationale**: Reuses the existing `ureq` dependency without inflating the project with `reqwest`, while maintaining application responsiveness. This mimics the existing pattern in `pdf_converter.rs`.
- **Alternatives considered**: Adding `reqwest` for native async. Rejected due to the desire to limit external dependencies where possible.

## 4. UI Visibility Filtering
- **Decision**: Update `ui::panels::tree::FileNode` (or similar file listing logic) and the tools like `list_files` to filter out files with image extensions. 
- **Rationale**: The specification mandates that image files should be completely hidden from the standard file UI and LLM tools, acting strictly as background inputs for Markdown generation.
- **Alternatives considered**: Showing them but making them unclickable. Rejected because the spec explicitly requires them NOT to be displayed or exposed.
