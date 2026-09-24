// apfel-rs Documentation and Interactive Showcase Engine

document.addEventListener('DOMContentLoaded', () => {
  initInstallTabs();
  initTerminalTabs();
  initApiExplorer();
  initCopyButtons();
});

// Install Tabs Configuration
const INSTALL_COMMANDS = {
  homebrew: "brew install bhubbard/tap/apfel-rs",
  npx: 'npx apfel-rs "Explain quantum computing in one sentence"',
  cargo: "cargo install apfel-rs",
  npm: "npm install -g apfel-rs",
  source: "git clone https://github.com/bhubbard/apfel-rs.git && cd apfel-rs && cargo build --release"
};

function initInstallTabs() {
  const tabs = document.querySelectorAll('.install-tabs .tab-btn');
  const commandText = document.getElementById('install-command-text');
  if (!tabs.length || !commandText) return;

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      const key = tab.getAttribute('data-tab');
      if (INSTALL_COMMANDS[key]) {
        commandText.textContent = INSTALL_COMMANDS[key];
      }
    });
  });
}

// Terminal Mockup Tabs
const TERMINAL_SAMPLES = {
  stream: `
<span class="t-prompt">$</span> <span class="t-cmd">apfel --stream "Write a haiku about compiling Rust"</span>
<span class="t-meta">[FoundationModels SystemLanguageModel 0.1B | On-Device]</span>

<span class="t-out">Borrow checker smiles,</span>
<span class="t-out">Zero-cost safety unfolds,</span>
<span class="t-out">Binary runs swift.</span>

<span class="t-success">✔ 18 tokens generated in 0.31s (58.1 tok/sec)</span>
`,
  chat: `
<span class="t-prompt">$</span> <span class="t-cmd">apfel --chat</span>
<span class="t-cyan">apfel interactive session (model: apple-foundationmodel, context: 4096 tokens)</span>
<span class="t-dim">Type /help, /info, /clear, or /exit</span>

<span class="t-prompt">&gt;</span> <span class="t-cmd">/info</span>
<span class="t-meta">Context Window : 4,096 tokens (input budget: 3,584)</span>
<span class="t-meta">Active Strategy: sliding-window (preserves last 8 turns)</span>
<span class="t-meta">Memory Usage   : 10.1 MB RSS</span>

<span class="t-prompt">&gt;</span> <span class="t-cmd">Give me a one-liner to inspect Apple Silicon GPU cores</span>
<span class="t-out">sysctl -n machdep.cpu.brand_string &amp;&amp; system_profiler SPDisplaysDataType</span>
`,
  curl: `
<span class="t-prompt">$</span> <span class="t-cmd">curl -s http://127.0.0.1:8080/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -d '{"messages": [{"role": "user", "content": "Ping"}]}' | jq</span>

<span class="t-amber">{
  "id": "chatcmpl-a89e02fb",
  "object": "chat.completion",
  "created": 1727220300,
  "model": "apple-foundationmodel",
  "choices": [{
    "index": 0,
    "message": { "role": "assistant", "content": "Pong! On-device and ready." },
    "finish_reason": "stop"
  }],
  "usage": { "prompt_tokens": 1, "completion_tokens": 6, "total_tokens": 7 }
}</span>
`,
  bench: `
<span class="t-prompt">$</span> <span class="t-cmd">apfel --benchmark</span>
<span class="t-meta">Benchmarking on-device FoundationModels via native Rust bridge...</span>

<span class="t-out">Warmup            : 5 runs completed</span>
<span class="t-out">Token Counting    : 68.35 ms mean (jitter p99: 91.6 ms)</span>
<span class="t-out">HTTP RPS (Axum)   : 4,160.2 req/sec (p95 latency: 9.85 ms)</span>
<span class="t-out">Inference Speed   : 56.4 tokens/sec</span>
<span class="t-out">RSS Memory (Idle) : 10.1 MB</span>

<span class="t-success">✔ Benchmark complete. Zero C-bridge overhead verified.</span>
`
};

function initTerminalTabs() {
  const tabs = document.querySelectorAll('.terminal-tabs .term-tab');
  const termBody = document.getElementById('terminal-content');
  if (!tabs.length || !termBody) return;

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      const sampleKey = tab.getAttribute('data-sample');
      if (TERMINAL_SAMPLES[sampleKey]) {
        termBody.innerHTML = TERMINAL_SAMPLES[sampleKey].trim();
      }
    });
  });
}

// API Explorer Code Snippets
const API_CODE_SAMPLES = {
  curl: `# POST /v1/chat/completions with SSE Streaming
curl -N -X POST http://127.0.0.1:8080/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "apple-foundationmodel",
    "stream": true,
    "temperature": 0.7,
    "messages": [
      {"role": "system", "content": "You are a concise engineering assistant."},
      {"role": "user", "content": "How does Rust guarantee thread safety?"}
    ]
  }'`,

  python: `from openai import OpenAI

# Connect to apfel-rs local daemon
client = OpenAI(
    base_url="http://127.0.0.1:8080/v1",
    api_key="not-needed"
)

response = client.chat.completions.create(
    model="apple-foundationmodel",
    messages=[
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Summarize macOS unified memory in 2 sentences."}
    ],
    stream=True
)

for chunk in response:
    content = chunk.choices[0].delta.content or ""
    print(content, end="", flush=True)`,

  ts: `import OpenAI from "openai";

const openai = new OpenAI({
  baseURL: "http://127.0.0.1:8080/v1",
  apiKey: "not-needed"
});

async function main() {
  const stream = await openai.chat.completions.create({
    model: "apple-foundationmodel",
    messages: [{ role: "user", content: "Write a quick Rust fibonacci function" }],
    stream: true,
  });

  for await (const chunk of stream) {
    process.stdout.write(chunk.choices[0]?.delta?.content || "");
  }
}

main();`,

  rust: `use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let res = client
        .post("http://127.0.0.1:8080/v1/chat/completions")
        .json(&json!({
            "model": "apple-foundationmodel",
            "messages": [{"role": "user", "content": "Ping"}]
        }))
        .send()
        .await?
        .text()
        .await?;

    println!("{res}");
    Ok(())
}`
};

function initApiExplorer() {
  const tabs = document.querySelectorAll('.api-explorer-tabs .api-tab-btn');
  const codeBox = document.getElementById('api-code-content');
  if (!tabs.length || !codeBox) return;

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');
      const lang = tab.getAttribute('data-lang');
      if (API_CODE_SAMPLES[lang]) {
        codeBox.textContent = API_CODE_SAMPLES[lang];
      }
    });
  });
}

// Universal Copy to Clipboard
function initCopyButtons() {
  document.querySelectorAll('[data-copy-target]').forEach(btn => {
    btn.addEventListener('click', () => {
      const targetId = btn.getAttribute('data-copy-target');
      const targetEl = document.getElementById(targetId);
      if (!targetEl) return;

      const text = targetEl.textContent || targetEl.innerText;
      navigator.clipboard.writeText(text.trim()).then(() => {
        const originalText = btn.innerHTML;
        btn.innerHTML = `
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#34d399" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"></polyline>
          </svg>
          <span style="color: #34d399">Copied!</span>
        `;
        setTimeout(() => {
          btn.innerHTML = originalText;
        }, 2000);
      });
    });
  });
}
