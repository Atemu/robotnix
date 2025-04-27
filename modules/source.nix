# SPDX-FileCopyrightText: 2020 Daniel Fullmer and robotnix contributors
# SPDX-License-Identifier: MIT

{ config, pkgs, lib, ... }:

let
  inherit (lib) mkIf mkDefault mkOption types literalExpression mkMerge;
  # A tree (attrset containing attrsets) which matches the source directories relpath filesystem structure.
  # e.g.
  # {
  #   "build" = {
  #     "make" = {};
  #     "soong" = {};
  #     ...
  #    }
  #    ...
  #  };
  dirsTree = let
    listToTreeBranch = xs:
      if builtins.length xs == 0 then {}
      else { "${builtins.head xs}" = listToTreeBranch (builtins.tail xs); };
    combineTreeBranches = branches:
      lib.foldr lib.recursiveUpdate {} branches;
    enabledDirs = lib.filterAttrs (name: dir: dir.enable) config.source.dirs;
  in
    combineTreeBranches (lib.mapAttrsToList (name: dir: listToTreeBranch (lib.splitString "/" dir.relpath)) enabledDirs);

  manifestModule = types.submodule {
    options = {
      manifest = mkOption {
        type = types.path;
        description = "The manifest metadata file to use.";
      };
      lockfile = mkOption {
        type = types.path;
        description = "The manifest lockfile to use.";
      };
      branch = mkOption {
        type = types.str;
        description = "The manifest branch to use.";
      };
    };
  };

  dirModule = let
    _config = config;
  in types.submodule ({ name, config, ... }: {
    options = {
      enable = mkOption {
        default = true;
        type = types.bool;
        description = "Whether to include this directory in the android build source tree.";
      };

      relpath = mkOption {
        default = name;
        type = types.str;
        description = "Relative path under android source tree to place this directory. Defaults to attribute name.";
      };

      src = mkOption {
        type = types.path;
        description = "Source to use for this android source directory.";
        default = pkgs.runCommand "empty" {} "mkdir -p $out";
        apply = src: # Maybe replace with with pkgs.applyPatches? Need patchFlags though...
          if (config.patches != [] || config.postPatch != "")
          then (pkgs.runCommand "${builtins.replaceStrings ["/"] ["="] config.relpath}-patched" {} ''
            cp --reflink=auto --no-preserve=ownership --no-dereference --preserve=links -r ${src} $out/
            chmod u+w -R $out
            ${lib.concatMapStringsSep "\n" (p: "echo Applying ${p} && patch -p1 --no-backup-if-mismatch -d $out < ${p}") config.patches}
            ${lib.concatMapStringsSep "\n" (p: "echo Applying ${p} && ${pkgs.git}/bin/git apply --directory=$out --unsafe-paths ${p}") config.gitPatches}
            cd $out
            ${config.postPatch}
          '')
          else src;
      };

      patches = mkOption {
        default = [];
        type = types.listOf types.path;
        description = "Patches to apply to source directory.";
      };

      # TODO: Ugly workaround since "git apply" doesn't handle fuzz in the hunk
      # line numbers like GNU patch does.
      gitPatches = mkOption {
        default = [];
        type = types.listOf types.path;
        description = "Patches to apply to source directory using 'git apply' instead of GNU patch.";
        internal = true;
      };

      postPatch = mkOption {
        default = "";
        type = types.lines;
        description = "Additional commands to run after patching source directory.";
      };

      unpackScript = mkOption {
        type = types.str;
        internal = true;
      };

      copyfiles = mkOption {
        type = types.attrsOf types.str;
        default = {};
      };

      linkfiles = mkOption {
        type = types.attrsOf types.str;
        default = {};
      };

      groups = mkOption {
        type = types.listOf types.str;
        default = [];
      };

      lastUpdated = mkOption {
        type = types.nullOr types.int;
        default = null;
        description = ''
          UNIX timestamp of the latest Git commit in the repository. Used to determine buildDateTime.
        '';
      };
    };

    config = {
      enable = mkDefault (
        (lib.any (g: lib.elem g config.groups) _config.source.includeGroups)
        || (!(lib.any (g: lib.elem g config.groups) _config.source.excludeGroups))
      );

      postPatch = let
        # Check if we need to make mountpoints in this directory for other repos to be mounted inside it.
        relpathSplit = lib.splitString "/" config.relpath;
        mountPoints = lib.attrNames (lib.attrByPath relpathSplit {} dirsTree);
      in mkIf (mountPoints != [])
        ((lib.concatMapStringsSep "\n" (mountPoint: "mkdir -p ${mountPoint}") mountPoints) + "\n");

      unpackScript = (lib.optionalString config.enable ''
        mkdir -p ${config.relpath}
        ${pkgs.util-linux}/bin/mount --bind ${config.src} ${config.relpath}
      '')
      + (lib.concatStringsSep "\n" (lib.mapAttrsToList (dest: src: ''
        mkdir -p $(dirname ${dest})
        cp --reflink=auto -f ${config.relpath}/${src} ${dest}
      '') config.copyfiles))
      + (lib.concatStringsSep "\n" (lib.mapAttrsToList (dest: src: ''
        mkdir -p $(dirname ${dest})
        ln -sf --relative ${config.relpath}/${src} ${dest}
      '') config.linkfiles));
    };
  });
in
{
  options = {
    source = {
      manifests = mkOption {
        default = {};
        type = types.attrsOf manifestModule;
        description = "Manifest files to read the source dir tree from. Generated by the updater.";
        example = literalExpression ''
          {
            manifest = ./repo-metadata.json;
            lockfile = ./repo.lock;
          }
        '';
      };

      dirs = mkOption {
        default = {};
        type = types.attrsOf dirModule;
        description = ''
          Additional directories to include in the Android build process.
        '';
      };

      excludeGroups = mkOption {
        default = [ "darwin" "mips" ];
        type = types.listOf types.str;
        description = "Project groups to exclude from source tree";
      };

      includeGroups = mkOption {
        default = [];
        type = types.listOf types.str;
        description = "Project groups to include in source tree (overrides `excludeGroups`)";
      };

      unpackScript = mkOption {
        default = "";
        internal = true;
        type = types.lines;
      };
    };
  };

  config = {
    buildDateTime = lib.mkDefault (
      lib.foldr lib.max 0 (
        (builtins.filter (x: !isNull x) (lib.mapAttrsToList (_: x: x.lastUpdated) config.source.dirs))
      )
    );

    source = {
      unpackScript = lib.concatMapStringsSep "\n" (d: d.unpackScript) (lib.attrValues config.source.dirs);

      dirs = lib.mkMerge (lib.mapAttrsToList (_: value:
        lib.listToAttrs (builtins.map (project: {
          name = project.path;
          value = let
            fetchgitArgs = (lib.importJSON value.lockfile)."${project.path}";
          in {
            inherit (project.branch_settings."${value.branch}") groups linkfiles copyfiles;
            lastUpdated = fetchgitArgs.date;
            src = pkgs.fetchgit {
              inherit (fetchgitArgs) url rev hash fetchLFS fetchSubmodules;
            };
          };
        }) (builtins.filter (p: builtins.hasAttr value.branch p.branch_settings) (lib.importJSON value.manifest)))
      ) config.source.manifests);
    };

    build = {
      unpackScript = pkgs.writeShellScript "unpack.sh" config.source.unpackScript;

      # Extract only files under robotnix/ (for debugging with an external AOSP build)
      debugUnpackScript = pkgs.writeShellScript "debug-unpack.sh" (''
        rm -rf robotnix
        '' +
        (lib.concatStringsSep "" (map (d: lib.optionalString (d.enable && (lib.hasPrefix "robotnix/" d.relpath)) ''
          mkdir -p $(dirname ${d.relpath})
          echo "${d.src} -> ${d.relpath}"
          cp --reflink=auto --no-preserve=ownership --no-dereference --preserve=links -r ${d.src} ${d.relpath}/
        '') (lib.attrValues config.source.dirs))) + ''
        chmod -R u+w robotnix/
      '');

      # Patch files in other sources besides robotnix/*
      debugPatchScript = pkgs.writeShellScript "debug-patch.sh"
        (lib.concatStringsSep "\n" (map (d: ''
          ${lib.concatMapStringsSep "\n" (p: "patch -p1 --no-backup-if-mismatch -d ${d.relpath} < ${p}") d.patches}
          ${lib.optionalString (d.postPatch != "") ''
          pushd ${d.relpath} >/dev/null
          ${d.postPatch}
          popd >/dev/null
          ''}
        '')
        (lib.filter (d: d.enable && ((d.patches != []) || (d.postPatch != ""))) (lib.attrValues config.source.dirs))));
    };
  };
}
