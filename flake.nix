{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      utils,
      crane,
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;
        world-cup-bot = craneLib.buildPackage {
          src = craneLib.cleanCargoSource (craneLib.path ./.);
          strictDeps = true;
          nativeBuildInputs = with pkgs; [ pkg-config ];
          buildInputs = with pkgs; [ openssl ];
        };
      in
      {
        packages.default = world-cup-bot;

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            cargo-release
            rustc
            gcc
            just
            pkg-config
            openssl
          ];

          shellHook = ''
            export SSL_CERT_FILE="${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          '';
        };
      }
    );
}
