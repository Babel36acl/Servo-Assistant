import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import ts from 'typescript';

const read = path => readFileSync(new URL(path, import.meta.url), 'utf8');

function frontendCommands(directory = new URL('../src/', import.meta.url)) {
  const commands = new Set();
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = new URL(entry.name + (entry.isDirectory() ? '/' : ''), directory);
    if (entry.isDirectory()) {
      for (const command of frontendCommands(path)) commands.add(command);
      continue;
    }
    if (!/\.(ts|vue)$/.test(entry.name)) continue;
    let source = readFileSync(path, 'utf8');
    if (entry.name.endsWith('.vue')) {
      source = [...source.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/g)].map(match => match[1]).join('\n');
    }
    const ast = ts.createSourceFile(entry.name + '.ts', source, ts.ScriptTarget.Latest, true);
    function visit(node) {
      if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'invoke') {
        assert.ok(node.arguments[0] && ts.isStringLiteral(node.arguments[0]), `${path}: IPC command must be explicit`);
        commands.add(node.arguments[0].text);
      }
      ts.forEachChild(node, visit);
    }
    visit(ast);
  }
  return commands;
}

test('frontend IPC commands are registered and explicitly allowed for the local main window', () => {
  const capability = JSON.parse(read('../src-tauri/capabilities/default.json'));
  assert.deepEqual(capability.windows, ['main']);
  assert.equal(capability.remote, undefined);
  assert.notEqual(capability.local, false);
  assert.ok(capability.permissions.includes('servo-runtime'));

  const permission = read('../src-tauri/permissions/servo-runtime.toml');
  assert.match(permission, /identifier\s*=\s*"servo-runtime"/);
  const allowList = permission.match(/commands\.allow\s*=\s*\[([^\]]*)\]/)?.[1];
  assert.ok(allowList, 'explicit command allow list is required');
  const allowed = new Set([...allowList.matchAll(/"([^"]+)"/g)].map(match => match[1]));
  const handler = read('../src-tauri/src/lib.rs').match(/tauri::generate_handler!\[([^\]]*)\]/)?.[1];
  assert.ok(handler, 'Tauri handler registration must exist');
  const registered = new Set(handler.split(',').map(value => value.trim().split('::').at(-1)).filter(Boolean));
  const invoked = frontendCommands();
  assert.ok(invoked.has('configure_communication'), 'connection path must be covered');
  for (const command of invoked) {
    assert.ok(registered.has(command), `${command}: frontend command has no backend handler`);
    assert.ok(allowed.has(command), `${command}: frontend command not allowed by ACL`);
  }
  for (const command of allowed) {
    assert.ok(registered.has(command), `${command}: permission has no backend handler`);
  }
});
