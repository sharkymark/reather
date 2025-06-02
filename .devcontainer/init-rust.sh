#!/bin/sh
# Script to initialize Rust project if Cargo.toml does not exist.

if [ ! -f Cargo.toml ]; then
  echo "Cargo.toml not found. Initializing Rust binary project in the current directory..."
  cargo init --bin
  echo "Rust binary project initialized."
else
  echo "Cargo.toml found. Skipping Rust project initialization."
fi
echo "Dev container: Rust initialization script finished."
