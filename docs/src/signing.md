<!--
SPDX-FileCopyrightText: 2026 cyclopentane and robotnix contributors
SPDX-License-Identifier: MIT
-->

# Signing

AOSP builds are cryptographically signed to prevent unauthorised tampering with system components. Roughly speaking, there are three ways builds are signed:

- Android Verified Boot (AVB) signatures: The bootloader verifies the integrity of the system partitions via partition hashes and hashtrees before the Linux kernel is booted.
- Individual APK signatures: Every APK is signed with some key, and Android only permits you to update an app with a new APK if the new APK is signed with the same key as the old one.
- OTA signatures: The OTA zips distributed via update servers are signed, and the system updater app and the recovery system check the signatures before allowing you to apply an upgrade.

In a normal AOSP build process, the components are first signed with so-called "test keys", which are publicly known and committed to the AOSP source repositories.
Then, as soon as the target files are built, the developer has the option to re-sign the components with their own "release keys", which must be kept secret.
This happens via the `sign_target_files_apks` tool.

## Generating signing keys

You can generate your own set of release keys with the `generateKeysScript` build target:

```shell
$ nix build .#robotnixConfigurations.foo.generateKeysScript --out-link generate-keys-script
$ ./generate-keys-script my-keys/
$ tree my-keys/
my-keys
└── tegu
    ├── avb.pem
    ├── avb_pkmd.bin
    ├── avb.pub
    ├── avb.x509.pem
    ├── bluetooth.pk8
    ├── bluetooth.x509.pem
    ├── gmscompat_lib.pk8
    ├── gmscompat_lib.x509.pem
    ├── media.pk8
    ├── media.x509.pem
    ├── networkstack.pk8
    ├── networkstack.x509.pem
    ├── nfc.pk8
    ├── nfc.x509.pem
    ├── platform.pk8
    ├── platform.x509.pem
    ├── releasekey.pk8
    ├── releasekey.x509.pem
    ├── sdk_sandbox.pk8
    ├── sdk_sandbox.x509.pem
    ├── shared.pk8
    └── shared.x509.pem
```


## Signing releases

In robotnix, however, we can't simply do so within the main build process, since the release keys are inaccessible within the Nix sandbox, and importing them into the Nix store would make them world-readable.
Thus, we split the build process into two parts:
The bulk of the build process that doesn't involve any secrets is done within the Nix sandbox.
Then, the very last step of signing the build with your own release keys is done outside of the Nix sandbox:
Robotnix provides a build target called `releaseScript`, which is a Bash script that can be pointed at your release keys directory.
When `releaseScript` is being run, it uses the unsigned target files that have previously been built in the Nix sandbox and your release keys to generate signed release files:

```console
$ nix build .#robotnixConfigurations.foo.releaseScript --out-link release-script
$ ./release-script my-keys/
$ ll tegu-*
-rw-r--r-- 1 c5h10 users 1741206178 Mar 22 20:02 tegu-factory-2026032000.zip
-rw-r--r-- 1 c5h10 users 1688149504 Mar 22 20:01 tegu-img-2026032000.zip
-rw-r--r-- 1 c5h10 users 1310723050 Mar 22 20:01 tegu-ota_update-2026032000.zip
-rw------- 1 c5h10 users 3369654024 Mar 22 19:58 tegu-signed_target_files-2026032000.zip
```

You can then either flash them via USB, or upload them to an update server to update your device over-the-air.


## PKCS#11 signing

Robotnix also allows you to sign your releases with keys residing on a PKCS#11 hardware token, such as an HSM or a YubiKey.
The token needs to have enough keyslots (~10) to accomodate all of the needed keys.
It is **not** permissible to reduce the number of keys by mapping multiple key aliases to a single key, since Android uses the certificate of an app to determine its SELinux context.

In order to set up PKCS#11 signing, the token must contain pairs of keys (`CKA_CLASS = CKO_PRIVATE_KEY`) and certificates (`CKA_CLASS = CKO_CERTIFICATE`) with the same ID (`CKA_ID` attribute), respectively.
This is due to how the Java Sun PKCS#11 keystore provider maps PKCS#11 keys to Java key aliases [^1].

Then, specify the PKCS#11 module, and the key and certificate labels (`CKA_LABEL` attribute) via the `signing.pkcs11` module:

```nix
{ pkgs, ... }:
{
    signing.pkcs11 = {
        enable = true;
        module = "${pkgs.yubico-piv-tool}/bin/libykcs11.so";
        privateKeyLabels = {
            "${config.device}/releasekey" = "Private key for Retired Key 1";
            "${config.device}/media" = "Private key for Retired Key 2";
            "${config.device}/shared" = "Private key for Retired Key 3";
            "${config.device}/platform" = "Private key for Retired Key 4";
            [...]
        };
        certificateLabels = {
            "${config.device}/releasekey" = "X.509 Certificate for Retired Key 1";
            "${config.device}/media" = "X.509 Certificate for Retired Key 2";
            "${config.device}/shared" = "X.509 Certificate for Retired Key 3";
            "${config.device}/platform" = "X.509 Certificate for Retired Key 4";
            [...]
        };
    };
}
```

The keys directory must contain all the public keys, certificates and AVB public key metadata blobs it also contains in the non-PKCS#11 case.

Then, build and run `releaseScript` with the parameters `<keysdir> <prev-buildnumber> <pin-file>`, where `<pin-file>` is a file that contains the PKCS#11 token PIN:

```console
$ nix build .#robotnixConfigurations.foo.releaseScript --out-link release-script
$ tree my-keys/
my-keys
└── tegu
    ├── avb_pkmd.bin
    ├── avb.pub
    ├── avb.x509.pem
    ├── bluetooth.x509.pem
    ├── gmscompat_lib.x509.pem
    ├── media.x509.pem
    ├── networkstack.x509.pem
    ├── nfc.x509.pem
    ├── platform.x509.pem
    ├── releasekey.x509.pem
    ├── sdk_sandbox.x509.pem
    └── shared.x509.pem
$ PIN_FILE=$(mktemp)
$ echo -n "123456" > $PIN_FILE
$ ./release-script my-keys/ "2026021200" "$PIN_FILE"
```

If you do not want to build an incremental OTA, specify `""` as the previous build ID.

## YubiKey PIV preset

Robotnix provides a PKCS#11 preset for the YubiKey PIV mode:

```nix
{
    signing.pkcs11 = {
        enable = true;
        presets.yubikey-piv.enable = true;
    };
}
```

This will set the required `module`, `certificateLabels` and `privateKeyLabels` options.
By default, it maps the AOSP signing keys to the "retired"[^2] PIV keyslots.
You can change the default keyslot mapping via `signing.pkcs11.presets.yubikey-piv.slotMap`.

To generate the required signing keys on your yubikey and place the public keys in your keys directory, run the `yubikeyGenerateKeysScript` target:

```console
$ nix build .#robotnixConfigurations.foo.yubikeyGenerateKeysScript --out-link yubikey-generate-keys-script
$ ./yubikey-generate-keys-script my-keys/
```

You can also export the public keys from your YubiKey after they have already been created:
```console
$ nix build .#robotnixConfigurations.foo.yubikeyExportCertificatesScript --out-link yubikey-export-certificates-script
$ ./yubikey-export-certificates-script my-keys/
```

Alternatively, you can also import pre-existing keys (the ones generated with `generateKeysScript`) into your YubiKey.
This is mainly useful if you want to provision multiple YubiKeys with the same keys as a backup.
```console
$ nix build .#robotnixConfigurations.foo.yubikeyImportKeysScript --out-link yubikey-import-keys-script
$ ./yubikey-import-keys-script my-keys/
```

Then, you can run `releaseScript` as described above.

[^1]: https://docs.oracle.com/javase/8/docs/technotes/guides/security/p11guide.html#KeyStoreRestrictions
[^2]: These key slots are called "retired" for legacy reason - originally, the PIV standard was made for personal identity cards holding four keys, each named after a purpose such as authentication, encryption, or signing.
      The PIV standard also specifies 20 so-called "retired key" slots meant to store old keys from past key rotations.
      However, from a technical point of view, there is nothing "retired" about them - they are fully usable, and they are not going to be removed in the future.
      As the keys we need wouldn't fit into the four main key slots, we use the retired key slots instead.
