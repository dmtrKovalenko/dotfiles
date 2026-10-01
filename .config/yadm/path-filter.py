#!/usr/bin/env python3
"""Translate selected live config paths to/from portable Git content."""

import getpass
import json
import os
from pathlib import Path
import re
import shlex
import sys
import tomllib
from xml.sax.saxutils import escape, unescape

HOME_TOKEN = '@@DOTFILES_HOME@@'
USER_TOKEN = '@@DOTFILES_USER@@'
JSON_STRING = re.compile(r'"(?:\\.|[^"\\])*"')
TOML_STRING = re.compile(r'"""[\s\S]*?"""|\x27\x27\x27[\s\S]*?\x27\x27\x27|"(?:\\.|[^"\\\r\n])*"|\x27[^\x27\r\n]*\x27')


def replace_value(value, operation, home, user):
    if operation == 'clean':
        value = value.replace(home + '/', HOME_TOKEN + '/')
        if value == home:
            value = HOME_TOKEN
        return value.replace('/tmp/rift_' + user + '.', '/tmp/rift_' + USER_TOKEN + '.')
    return value.replace(HOME_TOKEN, home).replace(USER_TOKEN, user)


def transform(content, filename, operation, home=None, user=None):
    home = (home or os.environ['HOME']).rstrip('/') or '/'
    user = user or getpass.getuser()
    if not home.startswith('/') or HOME_TOKEN in home or USER_TOKEN in home:
        raise ValueError('HOME must be an absolute path without filter placeholders')
    if any(ord(character) < 32 for character in home + user):
        raise ValueError('Control characters in HOME or username are unsupported')
    filename = filename.replace('\\', '/')

    def json_string(match):
        original = match.group()
        try:
            value = json.loads(original)
        except ValueError:
            return original
        replacement = replace_value(value, operation, home, user)
        return original if replacement == value else json.dumps(replacement, ensure_ascii=False)

    if filename.endswith(('.json', '.jsonc', '.lua')):
        return JSON_STRING.sub(json_string, content)
    if filename.endswith('.toml'):
        def toml_string(match):
            original = match.group()
            try:
                value = tomllib.loads('value = ' + original)['value']
            except tomllib.TOMLDecodeError:
                return original
            replacement = replace_value(value, operation, home, user)
            return original if replacement == value else json.dumps(replacement, ensure_ascii=False)
        return TOML_STRING.sub(toml_string, content)
    if filename.endswith('.plist'):
        def xml_text(match):
            original = match.group(1)
            value = unescape(original, {'&quot;': '"', '&apos;': "'"})
            replacement = replace_value(value, operation, home, user)
            return match.group() if replacement == value else '>' + escape(replacement) + '<'
        return re.sub(r'>([^<>]*)<', xml_text, content)
    if filename == '.config/kitty/kitty.conf':
        lines = []
        for line in content.splitlines(keepends=True):
            match = re.match(r'(action_alias\s+\S+\s+kitten\s+)([^\r\n]*)(\r?\n)?$', line)
            if match:
                arguments = shlex.split(match[2])
                replacements = [replace_value(arg, operation, home, user) for arg in arguments]
                if arguments != replacements:
                    line = match[1] + shlex.join(replacements) + (match[3] or '')
            elif line.lstrip().startswith('#'):
                line = replace_value(line, operation, home, user)
            lines.append(line)
        return ''.join(lines)
    raise ValueError('No path encoding defined for ' + filename)


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ('clean', 'smudge'):
        sys.exit('usage: path-filter.py clean|smudge repository-relative-path')
    try:
        content = sys.stdin.buffer.read().decode('utf-8')
        sys.stdout.buffer.write(transform(content, sys.argv[2], sys.argv[1]).encode('utf-8'))
    except (UnicodeError, ValueError, KeyError) as error:
        sys.exit('dotfiles path filter: ' + str(error))


if __name__ == '__main__':
    main()
