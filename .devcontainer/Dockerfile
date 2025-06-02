# Use the official Rust image.
FROM rust:latest

# Set default values for user creation (will be overridden if passed from devcontainer.json)
ARG USERNAME=coder
ARG USER_UID=1000
ARG USER_GID=1000

RUN DEBIAN_FRONTEND="noninteractive" apt-get update -y && \
    apt-get install --no-install-recommends -y \
    curl \
    wget \
    netcat-traditional \
    procps \
    net-tools \
    traceroute \
    iproute2 \
    iputils-ping \
    lsof \
    jq \
    gawk \
    ssh \
    git \
    htop \
    sudo \
    lldb \
    gdb \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME

ENV HOME=/home/$USERNAME
WORKDIR $HOME

# Install rust-analyzer and other tools
RUN rustup update && \
    rustup component add rust-analyzer clippy

