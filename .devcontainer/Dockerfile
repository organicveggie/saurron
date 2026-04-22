FROM rust:latest

# Install Dist and Cargo Release tools
RUN <<RUN_CMD_EOF
curl --proto '=https' --tlsv1.2 -LsSf \
    https://github.com/axodotdev/cargo-dist/releases/latest/download/cargo-dist-installer.sh | sh

    for tool in cargo-outdated cargo-release ; do
    if ! command -v $tool &> /dev/null; then
        echo "Installing $tool..."
        cargo install $tool
    else
        echo "$tool is already installed."
    fi
done
RUN_CMD_EOF
