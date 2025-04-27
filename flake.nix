{
  description = "Build Android (AOSP) using Nix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";

    androidPkgs.url = "github:tadfisher/android-nixpkgs/stable";

    flake-compat.url = "github:nix-community/flake-compat";
  };

  outputs = { self, nixpkgs, androidPkgs, flake-compat,  ... }@inputs: let
    pkgs = import ./pkgs/default.nix { inherit inputs; };
  in rec {
    # robotnixSystem evaluates a robotnix configuration
    lib.robotnixSystem = configuration: import ./default.nix {
      inherit configuration pkgs;
    };

    defaultTemplate = {
      path = ./template;
      description = "A basic robotnix configuration";
    };

    nixosModule = import ./nixos; # Contains all robotnix nixos modules
    nixosModules.attestation-server = import ./nixos/attestation-server/module.nix;

    packages.x86_64-linux = {
      manual = (import ./docs { inherit pkgs; }).manual;
      gitRepo = pkgs.gitRepo;
      robotnix-updater = pkgs.callPackage ./updater/package.nix {};
    };

    devShell.x86_64-linux = pkgs.mkShell {
      name = "robotnix-scripts";
      nativeBuildInputs = with pkgs; [
        # For android updater scripts
        (python3.withPackages (p: with p; [ mypy flake8 pytest ]))
        gitRepo nix-prefetch-git
        curl pup jq
        shellcheck
        wget

        # For chromium updater script
        # python2
        # cipd git

        cachix

        packages.x86_64-linux.robotnix-updater

        cargo rustc pkg-config openssl clippy
      ];
      PYTHONPATH=./scripts;
    };

    test = (lib.robotnixSystem (import ./configuration.nix));
  };
}
