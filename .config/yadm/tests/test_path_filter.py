import importlib.util
import json
from pathlib import Path
import plistlib
import shlex
import tomllib
import unittest

spec = importlib.util.spec_from_file_location('path_filter', Path(__file__).parents[1] / 'path-filter.py')
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)


class PortablePaths(unittest.TestCase):
    source_home = '/Users/alice'
    other_home = '/srv/home/Zoë \'quoted\' "double" & $cash \\ backslash'

    def round_trip(self, content, filename):
        canonical = driver.transform(content, filename, 'clean', self.source_home, 'alice')
        self.assertNotIn(self.source_home, canonical)
        self.assertEqual(driver.transform(canonical, filename, 'clean', self.source_home, 'alice'), canonical)
        rendered = driver.transform(canonical, filename, 'smudge', self.other_home, 'bob')
        self.assertNotIn(driver.HOME_TOKEN, rendered)
        self.assertEqual(driver.transform(rendered, filename, 'clean', self.other_home, 'bob'), canonical)
        self.assertEqual(driver.transform(rendered, filename, 'smudge', self.other_home, 'bob'), rendered)
        return canonical, rendered

    def test_json_paths_and_formatting(self):
        source = '{\n  "command": ["/Users/alice/.local/bin/tool"], "other": 7\n}\n'
        canonical, rendered = self.round_trip(source, 'config.json')
        self.assertEqual(json.loads(rendered)['command'], [self.other_home + '/.local/bin/tool'])
        self.assertEqual(canonical, source.replace(self.source_home, driver.HOME_TOKEN))

    def test_jsonc_comment_and_encoded_home(self):
        source = '// Keep this comment\n' + json.dumps({'path': self.other_home + '/tool'})
        canonical = driver.transform(source, 'settings.json', 'clean', self.other_home, 'bob')
        rendered = driver.transform(canonical, 'settings.json', 'smudge', self.source_home, 'alice')
        self.assertTrue(rendered.startswith('// Keep this comment\n'))
        self.assertEqual(json.loads(rendered.split('\n', 1)[1])['path'], self.source_home + '/tool')

    def test_toml_basic_and_literal_strings(self):
        for quote in ('"', "'", '"""', "'''" ):
            with self.subTest(quote=quote):
                source = 'path = ' + quote + self.source_home + '/tool' + quote + '\n'
                _, rendered = self.round_trip(source, 'config.toml')
                self.assertEqual(tomllib.loads(rendered)['path'], self.other_home + '/tool')

    def test_xml_home_and_username(self):
        source = plistlib.dumps({'ProgramArguments': [self.source_home + '/tool'], 'StandardOutPath': '/tmp/rift_alice.out.log'}).decode()
        canonical, rendered = self.round_trip(source, 'service.plist')
        parsed = plistlib.loads(rendered.encode())
        self.assertEqual(parsed['ProgramArguments'], [self.other_home + '/tool'])
        self.assertEqual(parsed['StandardOutPath'], '/tmp/rift_bob.out.log')
        self.assertIn(driver.USER_TOKEN, canonical)

    def test_kitty_argument_quoting(self):
        source = 'action_alias scrollback kitten /Users/alice/.local/tool.py\n'
        _, rendered = self.round_trip(source, '.config/kitty/kitty.conf')
        self.assertEqual(shlex.split(rendered)[-1], self.other_home + '/.local/tool.py')

    def test_lua_quoted_path(self):
        source = 'File = "/Users/alice/Downloads/background.png",\n'
        _, rendered = self.round_trip(source, '.wezterm.lua')
        encoded = rendered.split(' = ', 1)[1].rstrip(',\n')
        self.assertEqual(json.loads(encoded), self.other_home + '/Downloads/background.png')

    def test_other_users_and_prefixes_unchanged(self):
        source = json.dumps({'paths': ['/Users/alice-extra/tool', '/Users/bob/tool'], 'email': 'alice@example.com'})
        self.assertEqual(driver.transform(source, 'config.json', 'clean', self.source_home, 'alice'), source)

    def test_unconfigured_format_and_invalid_home_fail(self):
        with self.assertRaises(ValueError):
            driver.transform('data', 'config.unknown', 'clean', self.source_home, 'alice')
        for home in ('relative', '/home/line\nbreak', '/home/' + driver.HOME_TOKEN):
            with self.subTest(home=home), self.assertRaises(ValueError):
                driver.transform('{}', 'config.json', 'clean', home, 'alice')


if __name__ == '__main__':
    unittest.main()
