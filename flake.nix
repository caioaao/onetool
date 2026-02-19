{
  description = "better-agent: A better approach to building AI Agents";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Specific Rust version matching mise.toml
        rustVersion = pkgs.rust-bin.stable."1.93.1".default;

        # Build inputs for mlua vendored Lua
        buildInputs = with pkgs; [
          # Required for mlua with vendored Lua feature
          openssl
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          inherit buildInputs nativeBuildInputs;

          packages = [
            rustVersion
            pkgs.cargo-watch # Optional: useful for development
          ];

          # Environment variables
          RUST_SRC_PATH = "${rustVersion}/lib/rustlib/src/rust/library";
        };
      }
    );
}
