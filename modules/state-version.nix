# SPDX-FileCopyrightText: 2026 cyclopentane and robotnix contributors
# SPDX-License-Identifier: MIT

{
  lib,
  options,
  ...
}:

{
  options.stateVersion = lib.mkOption rec {
    type =
      with lib.types;
      (enum [
        "1"
        "2"
        "3"
      ]);
    # also bump in:
    # - docs/default.nix
    # - templates/*.nix
    default = "3";
    description = ''
      Analogously to the NixOS option `system.stateVersion`, this option
      tells the robotnix build what kind of state to expect to the device.
      Once you have flashed a robotnix build with some specific
      `stateVersion` to your device, `stateVersion` should be kept constant
      until you entirely wipe and re-flash the device.
    '';
    apply =
      v:
      if (options.stateVersion.highestPrio == (lib.mkOptionDefault { }).priority) then
        builtins.throw ''
          You have not set the `stateVersion` config option.

          ${description}

          If you are currently creating a configuration from scratch to be newly
          flashed onto your device, use the most recent one, i.e. `stateVersion = "3";`.

          If you have flashed your device before `stateVersion` was introduced,
          use `stateVersion = "1";`.
        ''
      else
        v;
  };
}
