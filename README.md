# OpenAI Unofficial Provider Extension

A WASM extension for ABK that provides OpenAI-compatible API communication.

## Features

- **OpenAI-compatible API**: Works with any OpenAI-compatible endpoint (OpenAI, Azure, local LLMs, etc.)
- **Streaming support**: Full SSE streaming for real-time responses
- **Function calling**: Complete tool/function calling support
- **Minimal headers**: Only standard headers (Authorization, Content-Type) - no proprietary headers

## Installation

### For Trustee/ABK agents

Copy the extension to your agent's extensions directory:

```bash
mkdir -p ~/.trustee/extensions/openai-unofficial
cp openai_unofficial_wasm.wasm ~/.trustee/extensions/openai-unofficial/
cp extension.toml ~/.trustee/extensions/openai-unofficial/
```

### Build from source

```bash
# Install WASM target
rustup target add wasm32-wasip1

# Build
cargo build --target wasm32-wasip1 --release

# Copy output
cp target/wasm32-wasip1/release/openai_unofficial_wasm.wasm ./
```

## Configuration

Set environment variables for your OpenAI-compatible endpoint:

```bash
# OpenAI
export OPENAI_API_KEY=sk-...
export OPENAI_BASE_URL=https://api.openai.com/v1
export OPENAI_DEFAULT_MODEL=gpt-4o-mini

# Or any OpenAI-compatible endpoint
export OPENAI_BASE_URL=https://your-endpoint.com/v1
export OPENAI_DEFAULT_MODEL=your-model-name
```

## Headers

This provider sends **only standard headers**:

| Header | Value |
|--------|-------|
| `Authorization` | `Bearer {api_key}` |
| `Content-Type` | `application/json` |
| `Accept` | `text/event-stream` (for streaming) |

**NOT sent** (GitHub Copilot-specific headers):
- `X-Request-Id`
- `X-Initiator`
- `X-Interaction-Id`
- `Copilot-Integration-Id`

## API Compatibility

Works with any endpoint implementing the OpenAI Chat Completions API:

- OpenAI
- Azure OpenAI
- Ollama (with OpenAI compatibility mode)
- LM Studio
- vLLM
- LocalAI
- Any other OpenAI-compatible server

## License

MIT OR Apache-2.0
