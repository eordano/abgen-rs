{
  description = "abgen-rs — dev shell with the toolchain to build the Rust crate and its vendored C++ deps (crunch via cc-rs, draco via CMake)";

  # Indirect ref: resolves through the flake registry to the host's pinned
  # nixpkgs, so `nix develop` works offline against the local store.
  inputs.nixpkgs.url = "nixpkgs";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system: f system nixpkgs.legacyPackages.${system});
    in {
      devShells = forAllSystems (system: pkgs:
        let
          # cc-rs passes `--target=arm64-apple-macosx`, which the nix cc-wrapper
          # rejects with a stderr warning ("!= arm64-apple-darwin"). cc-rs then
          # reads that warning as "flag unsupported" and silently drops EVERY
          # probed flag — including crunch's load-bearing -fno-strict-aliasing.
          # The wrapper already targets the host, so strip cc-rs's --target to
          # keep it silent. No-op on non-Darwin (the warning is Darwin-only).
          mkWrap = name: real: pkgs.writeShellScriptBin name ''
            args=()
            for a in "$@"; do
              case "$a" in
                --target=*) ;;          # drop; nix wrapper already targets host
                *) args+=("$a") ;;
              esac
            done
            exec ${real} "''${args[@]}"
          '';
          ccWrap = mkWrap "cc" "${pkgs.stdenv.cc}/bin/cc";
          cxxWrap = mkWrap "c++" "${pkgs.stdenv.cc}/bin/c++";
          isDarwin = pkgs.stdenv.hostPlatform.isDarwin;
        in {
        default = pkgs.mkShell {
          # cmake -> draco_decoder's build; pkg-config + a C/C++ toolchain
          # (mkShell's stdenv cc) -> crunch's cc-rs build.
          nativeBuildInputs = [ pkgs.cmake pkgs.pkg-config ]
            ++ pkgs.lib.optionals isDarwin [ ccWrap cxxWrap ];
          # libturbojpeg, dlopen'd at runtime for byte-exact JPEG decode.
          buildInputs = [ pkgs.libjpeg_turbo ];

          # rustc/cargo come from the host on PATH; uncomment to pin them here.
          # packages = [ pkgs.cargo pkgs.rustc ];

          TURBOJPEG_LIB =
            "${pkgs.libjpeg_turbo.out}/lib/libturbojpeg${pkgs.stdenv.hostPlatform.extensions.sharedLibrary}";

          # Export AFTER stdenv setup (which sets CC=clang and would clobber a
          # plain attribute) so cc-rs/CMake actually pick up the wrappers.
          shellHook = pkgs.lib.optionalString isDarwin ''
            export CC="${ccWrap}/bin/cc"
            export CXX="${cxxWrap}/bin/c++"
          '' + ''
            echo "abgen-rs devshell: cmake=$(command -v cmake)  CC=''${CC:-$(command -v cc)}  cargo=$(command -v cargo)"
            echo "  TURBOJPEG_LIB=$TURBOJPEG_LIB"
          '';
        };
      });
    };
}
