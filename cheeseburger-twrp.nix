# SPDX-FileCopyrightText: 2021 Atemu
# SPDX-FileCopyrightText: 2021 Daniel Fullmer and robotnix contributors
# SPDX-License-Identifier: MIT

#
# Usage
# =====
#
# ```
# $ nix-build --arg configuration ./cheeseburger-twrp.nix -A config.build.twrp
# ```
#

{ pkgs, lib, ... }:

{
  device = "cheeseburger_dumpling";
  androidVersion = 12;
  # androidVersion = 10;
  flavor = "twrp";

  productNamePrefix = lib.mkForce "twrp_";

  envVars.ALLOW_MISSING_DEPENDENCIES = "true";

  source.dirs = {
    "device/oneplus/cheeseburger_dumpling" = {
      # src = pkgs.fetchFromGitHub {
      #   owner = "TeamWin";
      #   repo = "android_device_oneplus_cheeseburger_dumpling";
      #   rev = "61f577680c941ca7c1e601105cad4c21cdc2fac9";
      #   sha256 = "sha256-i+87wxWAqvnZOtDeiH1+fnvrjqMIedolbKNcS8S5lZ4=";
      # };
      src = ../../repos/android_device_oneplus_cheeseburger_dumpling;
    };
    # "kernel/oneplus/msm8998".src = pkgs.fetchFromGitHub {
    #   owner = "LineageOS";
    #   repo = "android_kernel_oneplus_msm8998";
    #   rev = "21f69a76e95ed36e41cfd6f3d337e9f447707a3a";
    #   sha256 = "1f4iql2ba3q54sl1cby60dxnh9ibb0svharz1sg0zf8qd3f0gccb";
    # };
    # "device/oneplus/msm8998-common".src = pkgs.fetchFromGitHub {
    #   owner = "LineageOS";
    #   repo = "android_device_oneplus_msm8998-common";
    #   rev = "f786e0a6c674ab68f80f8773966594acfd8822e2";
    #   sha256 = "1qksjm39b673gw5bgkkinnmmj7m8dqbdr0i154avv9923h0pybjv";
    # };
    # "device/qcom/sepolicy-legacy-um".src = pkgs.fetchFromGitHub {
    #   owner = "LineageOS";
    #   repo = "android_device_qcom_sepolicy";
    #   rev = "2be1a7744eec25aa3c71e5e0642152dad0a24f83";
    #   sha256 = "18ibfcc037iy9p6zn1wky1833p7p2b59llnd1nqagc5h342pp82v";
    # };

    # "device/oneplus/cheeseburger".src = pkgs.fetchFromGitHub {
    #   owner = "LineageOS";
    #   repo = "android_device_oneplus_cheeseburger";
    #   rev = "5c00a13c93e9ce61a0c328d9a64b141388038135";
    #   sha256 = "sha256-JilFvXnmSoUa4j72tR3VVuAFA3/EmRKZfDdDkzwy5tg=";
    # };

    # "vendor/twrp".src = pkgs.fetchFromGitHub {
    #   owner = "TeamWin";
    #   repo = "android_vendor_twrp";
    #   rev = "d8454ba98edf2e7c45c7ac788122adcfcd5ddb42";
    #   sha256 = "sha256-2QQWSGcwtK8FOvNTuaHwRq5V+ufFzRTbkhDb6/NmKWY=";
    # };
  };
}
