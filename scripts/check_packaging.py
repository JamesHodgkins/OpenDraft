#!/usr/bin/env python3
"""Checks that the Linux packaging files agree with each other and with the code.

There are three descriptions of the same program for the desktop: the desktop
file template the .deb is built from (installer/linux/opendraft.desktop), the
plain desktop file the Arch and Flatpak packages install
(installer/linux/share/opendraft.desktop) and the AppStream file
(installer/linux/share/*.metainfo.xml). They are written by hand, so this keeps
them from drifting apart, and checks that the version the packages name is the
version of the program. Run by .github/workflows/packages.yml; needs only the
Python standard library.
"""

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINUX = ROOT / "installer" / "linux"
problems = []


def need(ok, message):
    if not ok:
        problems.append(message)


def desktop_entries(path):
    entries = {}
    for line in path.read_text().splitlines():
        if line.startswith("#") or "=" not in line or line.startswith("{{"):
            continue
        key, _, value = line.partition("=")
        entries[key] = value
    return entries


def semicolon_set(value):
    return {item for item in value.split(";") if item}


template = desktop_entries(LINUX / "opendraft.desktop")
plain = desktop_entries(LINUX / "share" / "opendraft.desktop")
for key in ("Type", "GenericName", "Categories", "Keywords", "StartupWMClass", "Terminal"):
    need(template.get(key) == plain.get(key),
         f"the two desktop files differ in {key}: {template.get(key)!r} and {plain.get(key)!r}")
need(semicolon_set(template.get("MimeType", "")) == semicolon_set(plain.get("MimeType", "")),
     "the two desktop files list different MIME types")
need(plain.get("StartupWMClass") == "opendraft",
     "StartupWMClass must be the window's app id, opendraft (profile::app_id)")
need(plain.get("Exec", "").split()[:1] == ["opendraft"] and "%F" in plain.get("Exec", ""),
     "the plain desktop file must run opendraft with the files %F")
need(plain.get("Icon") == "opendraft", "the plain desktop file's icon must be opendraft")

# The MIME file says what an .odx file is.
mime = ET.parse(LINUX / "opendraft.xml").getroot()
ns = "{http://www.freedesktop.org/standards/shared-mime-info}"
types = {t.get("type"): [g.get("pattern") for g in t.findall(f"{ns}glob")]
         for t in mime.findall(f"{ns}mime-type")}
need(types.get("application/x-opendraft-drawing") == ["*.odx"],
     f"the MIME file must map *.odx to application/x-opendraft-drawing, found {types}")
need("application/x-opendraft-drawing" in semicolon_set(plain.get("MimeType", "")),
     "the desktop file must list the .odx type")

# The AppStream file.
metainfo = ET.parse(LINUX / "share" / "uk.openengineering.opendraft.metainfo.xml").getroot()
need(metainfo.findtext("id") == "uk.openengineering.opendraft", "the AppStream id changed")
launchable = metainfo.findtext("launchable")
need(launchable == "uk.openengineering.opendraft.desktop",
     f"the AppStream launchable must be the Flatpak's desktop file name, found {launchable!r}")
provided = {m.text for m in metainfo.findall("provides/mediatype")}
need(provided <= semicolon_set(plain.get("MimeType", "")),
     f"AppStream provides types the desktop file does not list: {provided - semicolon_set(plain.get('MimeType', ''))}")

# The Flatpak manifest (plain text: only the standard library is used here).
manifest = (LINUX / "flatpak" / "uk.openengineering.opendraft.yml").read_text()
app_id = re.search(r"^app-id: (\S+)", manifest, re.M)
need(app_id and app_id.group(1) == metainfo.findtext("id"),
     "the Flatpak app-id must be the AppStream id")
need(re.search(r"^command: opendraft$", manifest, re.M), "the Flatpak must run the opendraft script")
for script in ("opendraft.sh", "opendraft-mcp.sh"):
    need((LINUX / "flatpak" / script).exists() and f"installer/linux/flatpak/{script}" in manifest,
         f"the Flatpak must install {script}")
    text = (LINUX / "flatpak" / script).read_text() if (LINUX / "flatpak" / script).exists() else ""
    need("OD_AUTOMATION_DIR" in text and "FLATPAK_ID" in text,
         f"{script} must point the automation socket at the folder the sandbox shares with the host")

# The Flathub manifest is the in-repo one with a different first source: nothing
# else may differ, or what CI builds is not what is submitted.
def manifest_without_sources(text):
    return text.split("    sources:")[0]


flathub_manifest = (LINUX / "flatpak" / "flathub" / "uk.openengineering.opendraft.yml").read_text()
body = lambda text: [line for line in manifest_without_sources(text).splitlines() if not line.lstrip().startswith("#")]
need(body(manifest) == body(flathub_manifest),
     "flatpak/flathub/uk.openengineering.opendraft.yml differs from the in-repo manifest outside its sources")

# Versions.
cargo = (ROOT / "Cargo.toml").read_text()
version = re.search(r'^\[workspace\.package\][^\[]*?^version = "([^"]+)"', cargo, re.M | re.S)
need(version is not None, "no workspace version in Cargo.toml")
if version:
    version = version.group(1)
    pkgbuild = (LINUX / "arch" / "PKGBUILD").read_text()
    pkgver = re.search(r"^pkgver=(\S+)", pkgbuild, re.M)
    need(pkgver and pkgver.group(1) == version,
         f"PKGBUILD pkgver is {pkgver and pkgver.group(1)} but the program is {version}: update it with the release")
    newest = metainfo.find("releases/release")
    need(newest is not None and newest.get("version") == version,
         f"the newest AppStream release is {newest is not None and newest.get('version')} but the program is {version}: add a <release> for it")

if problems:
    print("Packaging files disagree:")
    for problem in problems:
        print("  -", problem)
    sys.exit(1)
print("The packaging files agree with each other and with the program.")
