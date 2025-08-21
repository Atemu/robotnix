# GrapheneOS

Robotnix supports the current, up-to-date releases of GrapheneOS for each
device supported by the GrapheneOS upstream, although only `tegu` is tested
right now.

Signing builds with up-to-date Android versions via robotnix is not tested yet
due to a lack of development devices. If you wish to sign your builds, you can
use the official GrapheneOS signing scripts with the target files built with
the `config.build.android` target.

Example config:
```nix
{
    inputs.robotnix.url = "github:nix-community/robotnix";

    outputs = { self, robotnix }: {
        myGrapheneSystem = robotnix.lib.robotnixSystem (
        { config, lib, pkgs, ... }: {
            flavor = "grapheneos";
            device = "tegu";

            grapheneos.channel = "stable";

            apps.fdroid.enable = true;

            # Not tested yet.
            signing.enable = false;
        });
    };
}
```

## Internals

GrapheneOS uses [its own fork of
adevtool](https://github.com/GrapheneOS/adevtool) to extract the proprietary
vendor files from Google's official Pixel images. Since adevtool needs some tools from the GrapheneOS source tree, we run the vendor blob extraction during the main build.
