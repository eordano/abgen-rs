# Build for abgen-serve — the live-translate JIT asset-bundle proxy (umbrella
# port 5185). Consumed by umbrella's flake `packages.abgen-serve`, pinned at
# .gcroots/abgen-serve by `nix run .#install-units` so the systemd unit's
# ExecStart= survives nix-collect-garbage. This replaced the old "cp the cargo
# binary to ab-generator/bin/ + run it through dcl-shell" dance.
#
# Build deps mirror the abgen-rs devshell (flake.nix): cmake builds the vendored
# draco_decoder, cc-rs (via the stdenv toolchain) builds crunch, both pulled in
# as path crates under third_party/.
#
# Runtime notes (see systemd/umbrella-abgen-serve.service):
#   - libturbojpeg is dlopen'd at runtime (src/ffi.rs); wrapProgram bakes
#     TURBOJPEG_LIB to the exact store path so the unit needs no FHS env. This
#     is what lets us drop the dcl-shell wrapper.
#   - ABGEN_ROOT (template/*.bundle) and ABGEN_SHADER_BUNDLE (the vendored
#     DCL/Scene shader) default to compile-time CARGO_MANIFEST_DIR, which is a
#     throwaway build-sandbox path under nix; the unit sets both to the in-tree
#     data dirs so bundle + scene generation still find their inputs.
{ lib
, rustPlatform
, cmake
, pkg-config
, libjpeg_turbo
, makeWrapper
}:

rustPlatform.buildRustPackage {
  pname = "abgen-serve";
  version = "0.1.0";

  src = lib.cleanSource ./.;
  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ cmake pkg-config makeWrapper ];
  buildInputs = [ libjpeg_turbo ];

  # Only the serve binary is a service; abgen / abgen-corpus / abgen-verify are
  # dev tools run via cargo. Building one bin keeps the gcroot closure minimal.
  cargoBuildFlags = [ "--bin" "abgen-serve" ];
  doCheck = false;

  postInstall = ''
    wrapProgram $out/bin/abgen-serve \
      --set TURBOJPEG_LIB ${libjpeg_turbo.out}/lib/libturbojpeg.so
  '';

  meta = with lib; {
    description = "abgen-serve — live-translate JIT asset-bundle proxy (umbrella)";
    license = licenses.agpl3Plus;
    mainProgram = "abgen-serve";
    platforms = platforms.linux;
  };
}
