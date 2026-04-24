# embedding-provider-rs

Pure Rust embedding provider for devbase. Loads GGUF models directly via llama.cpp (through `embellama`) — no Ollama server required.

## Prerequisites

> ⚠️ **Windows 编译需要 C++ 工具链**。如果以下工具缺失，请先安装。

| 工具 | 用途 | 安装方式 |
|------|------|---------|
| **CMake** ≥ 3.14 | 编译 llama.cpp C++ 代码 | [cmake.org/download](https://cmake.org/download/) 或 `pip install cmake` |
| **Visual Studio 2022 Build Tools** | C++ 编译器 (cl.exe) | [Visual Studio 下载页](https://visualstudio.microsoft.com/downloads/) → 选择 "Desktop development with C++" |
| **CUDA Toolkit** 12.x (可选) | GPU 加速 | 已安装于本机 (CUDA 12.6) |

验证安装：
```powershell
cmake --version        # 应输出 ≥ 3.14
cl.exe                 # 应找到 Visual C++ 编译器
nvcc --version         # 应输出 CUDA 12.x
```

## Build

```powershell
cd tools\embedding-provider-rs

# CPU only (推荐先验证基础功能)
cargo build --release

# With CUDA acceleration (需要 CMake + VS Build Tools + CUDA)
cargo build --release --features cuda
```

> **Note**: RTX 4060 有 8GB VRAM。Qwen2.5-7B Q4_K_M (~4.5GB) 可以完整加载到 GPU；14B 模型 (~8.5GB) 可能超出显存，会自动回退到 CPU offload。

## Usage

```powershell
# 自动发现模型 (优先 7B，其次 14B)
.\target\release\embedding-provider-rs --repo-id claude-code-rust

# 指定模型路径
.\target\release\embedding-provider-rs `
    --model-path "C:\Users\22414\Desktop\model\Qwen2.5-7B-Instruct.Q4_K_M.gguf" `
    --repo-id claude-code-rust

# 跳过已存在的 embedding
.\target\release\embedding-provider-rs --repo-id claude-code-rust --skip-existing

# 自定义 registry 路径
.\target\release\embedding-provider-rs `
    --repo-id claude-code-rust `
    --db-path "C:\Users\22414\AppData\Local\devbase\registry.db"
```

## Architecture

```
GGUF model file (Qwen2.5-7B/14B)
        │
        ▼
[embellama / llama.cpp] ──► embedding vector (f32[])
        │
        ▼
devbase registry.db ──► code_embeddings BLOB
        │
        ▼
devkit_hybrid_search(repo_id, query_text, query_embedding?)
```

## Model Selection Guide

| 模型 | 文件大小 | VRAM 需求 | 推荐场景 |
|------|---------|----------|---------|
| Qwen2.5-7B Q4_K_M | ~4.5 GB | ~5 GB | **首选** — RTX 4060 8GB 可完整加载 |
| Qwen2.5-14B Q4_K_M | ~8.5 GB | ~9 GB | 需 CPU offload 部分层，速度较慢 |

## Troubleshooting

### `cmake not found`
安装 CMake 并确保其在 PATH 中：
```powershell
# 方法 1: pip
pip install cmake

# 方法 2: 官方安装包 (推荐)
# https://cmake.org/download/
```

### `cl.exe not found`
需要安装 Visual Studio Build Tools：
1. 下载 [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
2. 安装 "Desktop development with C++" 工作负载
3. 从 "x64 Native Tools Command Prompt" 运行 cargo build

### CUDA out of memory
14B 模型超出 RTX 4060 8GB 显存时，llama.cpp 会自动将部分层 offload 到系统内存。可以通过降低 `--batch-size` 减少峰值显存占用。

## Differences from Python Provider

| 维度 | Python (`tools/embedding-provider/`) | Rust (`tools/embedding-provider-rs/`) |
|------|--------------------------------------|---------------------------------------|
| 依赖 | `requests` (HTTP) → Ollama | `embellama` (本地 GGUF 直接加载) |
| 运行时 | 需要 Ollama 服务 | 零外部服务，纯本地推理 |
| 性能 | 受 HTTP 往返影响 | 内存内 batch 处理，更快 |
| 编译 | 零编译，pip 安装 | 首次编译 10-30 分钟（llama.cpp C++） |
| 适用场景 | 快速验证、已有 Ollama 环境 | 生产部署、无网络环境、GPU 最大化利用 |
