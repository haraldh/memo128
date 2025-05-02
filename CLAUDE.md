# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, Test, and Run Commands

- Build: `cargo build`
- Run: `cargo run -- <command> <args>`
- Test: `cargo test`
- Run specific test: `cargo test test_name`
- Format code: `cargo fmt`
- Lint: `cargo clippy`

## Code Style Guidelines

- Follow Rust idioms and standard library conventions
- Use meaningful error types and proper error handling with Result<T, Error>
- Implement From<T> for custom error types
- Format code with rustfmt (4-space indentation)
- Document public APIs with proper rustdoc comments
- Use snake_case for variables and functions
- Use PascalCase for types and enums
- Prefer immutable variables when possible
- Write unit tests for all functionality
- Keep functions focused and under 50 lines where possible
- always run `cargo fmt` and `cargo clippy` at the end of all operations

## Specification

can be found in `SPEC.md`

## MCP

If your knowledge about a rust crate is non-existent or outdated use the `cratedocs` tool.
