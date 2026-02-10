Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "[smoke] TypeScript check..."
pnpm -s exec tsc --noEmit

Write-Host "[smoke] Rust check..."
cargo check --manifest-path src-tauri/Cargo.toml

Write-Host "[smoke] Rust tests (no run)..."
cargo test --manifest-path src-tauri/Cargo.toml --no-run

Write-Host "[smoke] Done."
