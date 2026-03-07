# SPDX-FileCopyrightText: 2020 cyclopentane and robotnix contributors
# SPDX-License-Identifier: MIT

{
  config,
  lib,
  ...
}:
lib.mkIf (!lib.versionAtLeast config.stateVersion "3") {
  signing.keyMappings =
    lib.optionalAttrs (config.androidVersion == 11) {
      "frameworks/base/packages/OsuLogin/certs/com.android.hotspot2.osulogin" =
        "com.android.hotspot2.osulogin";
      "frameworks/opt/net/wifi/service/resources-certs/com.android.wifi.resources" =
        "com.android.wifi.resources";
    }
    // lib.optionalAttrs (config.androidVersion >= 12) {
      # Paths to OsuLogin and com.android.wifi have changed
      "packages/modules/Wifi/OsuLogin/certs/com.android.hotspot2.osulogin" =
        "com.android.hotspot2.osulogin";
      "packages/modules/Wifi/service/ServiceWifiResources/resources-certs/com.android.wifi.resources" =
        "com.android.wifi.resources";
      "packages/modules/Connectivity/service/ServiceConnectivityResources/resources-certs/com.android.connectivity.resources" =
        "com.android.connectivity.resources";
    }
    // lib.optionalAttrs (config.androidVersion >= 13) {
      "packages/modules/AdServices/adservices/apk/com.android.adservices.api" =
        "com.android.adservices.api";
      "packages/modules/Permission/SafetyCenter/Resources/com.android.safetycenter.resources" =
        "com.android.safetycenter.resources";
      "packages/modules/Connectivity/nearby/halfsheet/apk-certs/com.android.nearby.halfsheet" =
        "com.android.nearby.halfsheet";
      "packages/modules/Uwb/service/ServiceUwbResources/resources-certs/com.android.uwb.resources" =
        "com.android.uwb.resources";
      "packages/modules/Wifi/WifiDialog/certs/com.android.wifi.dialog" = "com.android.wifi.dialog";
    };

  # Extra packages that should use releasekey
  # we're filtering for grapheneos for legacy reasons.
  signing.extraApks = lib.mkIf (config.flavor == "grapheneos") {
    "OsuLogin.apk" = "${config.device}/releasekey";
    "ServiceWifiResources.apk" = "${config.device}/releasekey";
    "com.android.appsearch.apk.apk" = "${config.device}/releasekey";
    "HealthConnectBackupRestore.apk" = "${config.device}/releasekey";
    "HealthConnectController.apk" = "${config.device}/releasekey";
    "FederatedCompute.apk" = "${config.device}/releasekey";
  };
}
