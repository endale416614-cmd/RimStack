#!/usr/bin/env python3
"""Writes the metainfo's newest release as the version Cargo.toml names, dated the
day of the build: SOURCE_DATE_EPOCH when set, today in UTC otherwise.

    packaging/flatpak/metainfo-version.py <metainfo.xml> <out.xml>
"""
import datetime, os, pathlib, re, sys, tomllib

RELEASE = re.compile(r"<release\b[^>]*>", re.S)


def version_named():
    root = pathlib.Path(__file__).resolve().parents[2]
    with open(root / "Cargo.toml", "rb") as file:
        return tomllib.load(file)["workspace"]["package"]["version"]


def day():
    epoch = os.environ.get("SOURCE_DATE_EPOCH")
    if not epoch:
        return datetime.datetime.now(datetime.UTC).strftime("%Y-%m-%d")
    return datetime.datetime.fromtimestamp(int(epoch), datetime.UTC).strftime("%Y-%m-%d")


def say(tag, name, value):
    given = re.compile(r'\b%s="[^"]*"' % name)
    if given.search(tag):
        return given.sub('%s="%s"' % (name, value), tag, count=1)
    return tag.replace("<release", '<release %s="%s"' % (name, value), 1)


def newest(text, version, when):
    found = RELEASE.search(text)
    if not found:
        sys.exit("the metainfo has no <release> entry to date")
    tag = say(say(found.group(0), "version", version), "date", when)
    return text[: found.start()] + tag + text[found.end() :]


def main(argv):
    if len(argv) != 3:
        sys.exit("usage: %s <metainfo.xml> <out.xml>" % argv[0])
    text = pathlib.Path(argv[1]).read_text(encoding="utf-8")
    pathlib.Path(argv[2]).write_text(newest(text, version_named(), day()), encoding="utf-8")


main(sys.argv)
