{ pkgs ? import <nixpkgs> { } }:

assert pkgs.lib.assertMsg (pkgs.lib.versionAtLeast pkgs.rustc.version "1.88")
  "changelog-merge requires Rust 1.88 or newer; supply a recent pinned nixpkgs as pkgs";

pkgs.rustPlatform.buildRustPackage {
  pname = "changelog-merge";
  version = "0.1.0";
  src = pkgs.lib.cleanSourceWith {
    src = ./.;
    filter = path: type:
      pkgs.lib.cleanSourceFilter path type
      && !(builtins.elem (builtins.baseNameOf path) [
        "target" ".agents" ".aws" ".codex" "result"
      ]);
  };
  cargoLock.lockFile = ./Cargo.lock;
  doCheck = true;
  nativeCheckInputs = [ pkgs.git ];
  meta = {
    description = "Semantic three-way merging for Keep a Changelog";
    license = pkgs.lib.licenses.asl20;
    mainProgram = "changelog-merge";
  };
}
