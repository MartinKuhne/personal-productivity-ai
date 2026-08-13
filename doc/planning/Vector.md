# Vector Search & Embeddings Implementation Plan
Status: Aspirational / Planning Phase

## 1. Objective
To index thousands to tens of thousands of markdown documents and enable similarity searches using a simple, embedded model and vector index written in Rust.

## 2. Requirements & Constraints
* **Dataset Size**: 10,000 - 50,000 markdown documents.
* **Storage Model**: Embedded (in-process) and persisted (written to disk automatically or serialized).
* **Language**: Rust.
* **Complexity**: Simple integration, avoiding heavy external infrastructure (like dedicated vector database clusters) if possible.

## 3. Database Options (Embedded & Persisted)

### 3.1. LanceDB
* **Architecture**: Serverless, in-process, persistent.
* **Storage Engine**: Lance columnar format (Apache Arrow ecosystem).
* **Pros**: 
  * Writes to local disk out-of-the-box.
  * Very fast for data science workloads.
  * Can store raw data (markdown text) alongside vectors.
  * Cloud-ready (can easily point to S3 instead of local disk).
* **Cons**: 
  * Relies on the Lance file format (smaller ecosystem than Parquet).
  * Not optimized for ultra-high concurrent mutations.
* **Recommendation**: Best overall choice if feature completeness and ecosystem integration (Arrow) are priorities.

### 3.2. SahomeDB
* **Architecture**: Lightweight, embedded, persistent.
* **Storage Engine**: `sled` (embedded key-value store).
* **Pros**: 
  * Feels like SQLite for vectors.
  * Extremely low overhead, simple API.
  * No heavy data science dependencies.
* **Cons**: 
  * Smaller community and fewer advanced features compared to LanceDB.
* **Recommendation**: Best for a minimalist, dependency-light solution.

### 3.3. SurrealDB (Embedded Mode)
* **Architecture**: Multi-model database (document, graph, vector).
* **Storage Engine**: RocksDB / custom (when run as `file://`).
* **Pros**: 
  * Powerful, solves relational + vector needs simultaneously.
* **Cons**: 
  * Very heavyweight. Increased compile times and binary sizes. Overkill for just a vector search feature.

### 3.4. Similari (In-Memory with Manual Persistence)
* **Architecture**: In-memory library.
* **Pros**: 
  * Blazing fast, zero disk I/O overhead during search.
* **Cons**: 
  * Vectors must fit in RAM.
  * Persistence must be handled manually (e.g., serializing to/from a binary or JSON file on disk during startup/shutdown).

## 4. Embedding Model Options

To convert the markdown text into vectors, an embedding generator is required.

### 4.1. Fastembed (`fastembed-rs`)
* **Type**: Local ONNX Runtime.
* **Pros**: Zero external dependencies (no PyTorch, no Python). Downloads and caches small, fast models like `all-MiniLM-L6-v2` locally. Perfect for this dataset size.
* **Cons**: Limited to models supported by the ONNX export.

### 4.2. Local LLM (Ollama via `ollama-rs`)
* **Type**: Managed local service via HTTP.
* **Pros**: Easy to use if Ollama is already running. Can leverage large LLMs (like `llama3` or `mxbai-embed-large`).
* **Cons**: Requires the Ollama background service to be installed and running on the host machine.

### 4.3. Native Rust LLM (`candle`)
* **Type**: In-process LLM execution.
* **Pros**: Zero external dependencies, self-contained executable. Can run massive LLM models natively.
* **Cons**: High boilerplate and complexity (requires handling weights and tokenization manually).

## 5. Proposed Architecture

For indexing ~10k to 50k markdown documents simply and robustly, the recommended stack is:
1. **Embedding Generation**: Use `fastembed-rs` to locally convert markdown strings into dense vectors.
2. **Storage and Indexing**: Use **LanceDB** or **SahomeDB** to persist those vectors and their associated metadata (file paths) to a local directory.

Both options keep the entire process embedded within a single Rust binary without external clusters.
