FROM debian:trixie-slim

RUN apt update && apt install -y zsh curl git vim httpie jq unzip bash sudo build-essential pkg-config
RUN usermod --shell /usr/bin/zsh root

# Oh My Zsh
RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended
RUN git clone https://github.com/zsh-users/zsh-autosuggestions ~/.oh-my-zsh/custom/plugins/zsh-autosuggestions
RUN git clone https://github.com/zsh-users/zsh-syntax-highlighting.git ~/.oh-my-zsh/custom/plugins/zsh-syntax-highlighting
RUN git clone https://github.com/zsh-users/zsh-completions ~/.oh-my-zsh/custom/plugins/zsh-completions

# Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN . "$HOME/.cargo/env" && rustup component add rustfmt clippy

# Atuin shell history
RUN curl --proto '=https' --tlsv1.2 -sSf https://setup.atuin.sh | bash

RUN curl -fsSL https://claude.ai/install.sh | bash
RUN mkdir -p /root/.claude && ln -sf /root/.claude/.claude.json /root/.claude.json

RUN export CODEX_HOME=/opt/codex-home \
    && curl -fsSL https://chatgpt.com/codex/install.sh | sh

RUN curl -fsSL https://raw.githubusercontent.com/devcontainers/cli/main/scripts/install.sh | sh

COPY .devcontainer/.zshrc /root/.zshrc
