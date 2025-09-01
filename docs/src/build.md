# Build from Source 🧱

Want to hack on `aj`? Let’s go! 🧑‍💻

## 🤢 Install dependencies
```shell
brew install miniconda               # or use the official installer
conda create -n aj python=3.11 -y
conda activate aj
pip install torch==2.4.0 --index-url https://download.pytorch.org/whl/cp
export LIBTORCH_USE_PYTORCH=1
export LIBTORCH="/opt/homebrew/Caskroom/miniconda/base/pkgs/pytorch-2.4.0-py3.11_0/lib/python3.11/site-packages/torch"
export DYLD_LIBRARY_PATH="$LIBTORCH/lib"
```

## 🛠️ Clone & Build
```shell
git clone https://github.com/graves/awful_aj.git
cd awful_aj
cargo build
```

## ✅ Run Tests
```shell
cargo test
```

> Tip: If you modify features that touch embeddings, ensure your Python + PyTorch environment is active before running commands that exercise memory/vector search.

## 🧯 Common Troubleshooting
- Linker/PyTorch libs not found: Recheck the `LIBTORCH` environment variable and your platform’s dynamic library path env var (`DYLD_LIBRARY_PATH` on macOS, `LD_LIBRARY_PATH` on Linux, `PATH` on Windows).
- Model not downloading: 
    - Ensure the config directory exists and is writable. See Config Paths on your OS's [Install](./install/mac_os.md) page.
    - Check your network connection.