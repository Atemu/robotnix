#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2020 Daniel Fullmer and robotnix contributors
# SPDX-License-Identifier: MIT

from typing import Any
import json
import urllib.request
import os
import pathlib
import tomllib

from robotnix_common import save, get_store_path, checkout_git


def fetch_metadata(
        hudson_url: str = 'https://github.com/LineageOS/hudson',
        lineage_build_targets_path: str = 'lineage-build-targets',
        devices_json_path: str = 'updater/devices.json',
        device_deps_json_path: str = 'updater/device_deps.json',
        ) -> Any:
    metadata = {}

    hudson_path = get_store_path(checkout_git(hudson_url, 'refs/heads/main')['path'])

    supported_devices_toml = os.path.join(os.path.dirname(__file__), 'supported_devices.toml')
    supported_devices = tomllib.loads(open(supported_devices_toml).read())

    lineage_build_targets = open(f'{hudson_path}/{lineage_build_targets_path}').readlines()
    for line in lineage_build_targets:
        line = line.strip()
        if line == "":
            continue
        if line.startswith("#"):
            continue

        device, variant, branch, updatePeriod = line.split()

        if device in supported_devices['supported'] and not device in supported_devices['unsupported']:
            metadata[device] = {
                'variant': variant,
                'branch': branch,
                'deps': [],
            }

    ###

    devices = json.load(open(f'{hudson_path}/{devices_json_path}'))
    for data in devices:
        if data['model'] not in metadata:
            continue

        device = data['model']
        vendor = data['oem'].lower()

        metadata[data['model']].update({
            'vendor': vendor,
            'name': data['name'],
            'lineage_recovery': data.get('lineage_recovery', False)
        })

    device_deps = json.load(open(f'{hudson_path}/{device_deps_json_path}'))
    for device, deps in device_deps.items():
        if device not in supported_devices['supported']:
            continue
        if device not in metadata:
            print(f"Warning: {device} in device_deps.json but not in devices.json")
            continue
        metadata[device]['deps'] = deps

    return metadata


if __name__ == '__main__':
    metadata = fetch_metadata()
    os.chdir(pathlib.Path(__file__).parent.resolve())
    save('device-metadata.json', metadata)
