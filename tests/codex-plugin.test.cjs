'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const builder = require('../scripts/build-codex-plugin.cjs');

const RECEIVER = path.join(os.tmpdir(), 'quotabar-receiver.exe');

/**
 * Fixture shaped exactly like the Rust generator in
 * src-tauri/src/task_integration.rs (generate(), tool="codex", windows=true):
 * outer { target, content: JSON.stringify({hooks:{Event:[{hooks:[handler]}]}}), note }
 * with handler { type, command, timeout:3, statusMessage, commandWindows }.
 */
function rustGeneratorFixture({ windows = true, withStatusMessage = true } = {}) {
  const hooks = {};
  for (const event of builder.EVENTS) {
    const cmd = `'${RECEIVER}' task-event codex`;
    const handler = {
      type: 'command',
      command: windows ? cmd.replace(/'/g, '"') : cmd,
      timeout: 3,
    };
    if (withStatusMessage) {
      handler.statusMessage = `QuotaBar local task status: ${event}`;
    }
    if (windows) {
      handler.commandWindows = handler.command;
    }
    hooks[event] = [{ hooks: [handler] }];
  }
  const content = JSON.stringify({ hooks }, null, 2);
  return {
    outer: JSON.stringify({
      target: '~/.codex/hooks.json',
      content,
      note: '可点击安装接入，或手动合并 hooks 条目；不要覆盖已有配置。重启工具；Codex 还需信任此 hook。',
    }),
    hooks,
  };
}

test('buildArtifacts emits the Rust generator nesting: event -> [group] -> hooks[handler]', () => {
  const fixture = rustGeneratorFixture();
  const hookConfig = builder.parseReceiverOutput(fixture.outer);
  const { hooks } = builder.buildArtifacts(hookConfig);
  assert.deepEqual(Object.keys(hooks.hooks), builder.EVENTS);
  assert.equal(Object.keys(hooks.hooks).length, 8);
  for (const event of builder.EVENTS) {
    const groups = hooks.hooks[event];
    assert.equal(groups.length, 1, `exactly 1 group for ${event}`);
    assert.ok(Array.isArray(groups[0].hooks));
    assert.equal(groups[0].hooks.length, 1, `exactly 1 handler for ${event}`);
    assert.equal(groups[0].hooks[0].type, 'command');
  }
});

test('roundtrip preserves timeout, commandWindows, and command verbatim', () => {
  const fixture = rustGeneratorFixture({ windows: true });
  const { hooks } = builder.buildArtifacts(builder.parseReceiverOutput(fixture.outer));
  for (const event of builder.EVENTS) {
    const got = hooks.hooks[event][0].hooks[0];
    const want = fixture.hooks[event][0].hooks[0];
    assert.equal(got.command, want.command, `command preserved for ${event}`);
    assert.equal(got.commandWindows, want.commandWindows, `commandWindows preserved for ${event}`);
    assert.equal(got.timeout, 3, `timeout preserved for ${event}`);
  }
});

test('statusMessage is added when the generator omitted it, kept canonical otherwise', () => {
  const without = rustGeneratorFixture({ withStatusMessage: false });
  const { hooks } = builder.buildArtifacts(builder.parseReceiverOutput(without.outer));
  for (const event of builder.EVENTS) {
    assert.equal(
      hooks.hooks[event][0].hooks[0].statusMessage,
      `QuotaBar local task status: ${event}`
    );
  }
  const withMsg = rustGeneratorFixture({ withStatusMessage: true });
  const again = builder.buildArtifacts(builder.parseReceiverOutput(withMsg.outer));
  for (const event of builder.EVENTS) {
    assert.equal(
      again.hooks.hooks[event][0].hooks[0].statusMessage,
      `QuotaBar local task status: ${event}`
    );
  }
});

test('manifest matches the official accepted shape exactly', () => {
  const fixture = rustGeneratorFixture();
  const { manifest } = builder.buildArtifacts(builder.parseReceiverOutput(fixture.outer));
  assert.equal(manifest.name, 'quotabar');
  assert.equal(manifest.version, '0.1.0');
  assert.ok(/本地/.test(manifest.description));
  assert.deepEqual(manifest.author, { name: 'kimhero110' });
  assert.equal(manifest.repository, 'https://github.com/kimhero110/desktoken');
  assert.equal(manifest.license, 'MIT');
  assert.equal(manifest.interface.displayName, 'QuotaBar');
  assert.equal(typeof manifest.interface.shortDescription, 'string');
  assert.equal(typeof manifest.interface.longDescription, 'string');
  assert.equal(manifest.interface.developerName, 'kimhero110');
  assert.equal(manifest.interface.category, 'Productivity');
  assert.deepEqual(manifest.interface.capabilities, []);
  assert.deepEqual(manifest.interface.defaultPrompt, []);
  assert.deepEqual(
    Object.keys(manifest).sort(),
    ['author', 'description', 'interface', 'license', 'name', 'repository', 'version']
  );
  assert.deepEqual(
    Object.keys(manifest.interface).sort(),
    ['capabilities', 'category', 'defaultPrompt', 'developerName', 'displayName',
     'longDescription', 'shortDescription']
  );
  for (const banned of ['displayName', 'developerName', 'category', 'capabilities',
    'hooks', 'skills', 'apps', 'mcp', 'mcpServers', 'prompts', 'defaultPrompt']) {
    assert.equal(banned in manifest, false, `manifest root must not declare ${banned}`);
  }
});

test('artifacts serialize as readable UTF-8 JSON round-trips', () => {
  const fixture = rustGeneratorFixture();
  const { hooks, manifest } = builder.buildArtifacts(builder.parseReceiverOutput(fixture.outer));
  assert.deepEqual(JSON.parse(`${JSON.stringify(hooks, null, 2)}\n`), hooks);
  assert.deepEqual(JSON.parse(`${JSON.stringify(manifest, null, 2)}\n`), manifest);
});

test('normalizeHookConfig rejects missing, extra, multi-group, multi-handler, malformed', () => {
  const fixture = rustGeneratorFixture();
  const good = builder.parseReceiverOutput(fixture.outer);

  const missing = { ...good };
  delete missing.SessionEnd;
  assert.throws(() => builder.normalizeHookConfig(missing), /missing required hook events: SessionEnd/);

  const extra = { ...good, TrustRequest: good.Stop };
  assert.throws(() => builder.normalizeHookConfig(extra), /unexpected hook events/);

  const twoGroups = { ...good, Stop: [good.Stop[0], good.Stop[0]] };
  assert.throws(() => builder.normalizeHookConfig(twoGroups), /exactly 1 group/);

  const twoHandlers = { ...good, Stop: [{ hooks: [good.Stop[0].hooks[0], good.Stop[0].hooks[0]] }] };
  assert.throws(() => builder.normalizeHookConfig(twoHandlers), /exactly 1 handler/);

  const noHooksKey = { ...good, Stop: [{ matcher: '*' }] };
  assert.throws(() => builder.normalizeHookConfig(noHooksKey), /"hooks" array/);

  const badType = { ...good, Stop: [{ hooks: [{ type: 'prompt', command: 'x' }] }] };
  assert.throws(() => builder.normalizeHookConfig(badType), /type must be "command"/);

  const noCommand = { ...good, Stop: [{ hooks: [{ type: 'command' }] }] };
  assert.throws(() => builder.normalizeHookConfig(noCommand), /no usable command/);

  const notArray = { ...good, Stop: { hooks: [] } };
  assert.throws(() => builder.normalizeHookConfig(notArray), /array of groups/);
});

test('parseReceiverOutput requires outer content as a JSON string', () => {
  const fixture = rustGeneratorFixture();
  assert.deepEqual(builder.parseReceiverOutput(fixture.outer), fixture.hooks);

  assert.throws(() => builder.parseReceiverOutput(''), /no stdout/);
  assert.throws(() => builder.parseReceiverOutput('not json'), /not valid JSON/);
  assert.throws(() => builder.parseReceiverOutput('[]'), /JSON object/);
  assert.throws(
    () => builder.parseReceiverOutput(JSON.stringify({ content: { hooks: {} } })),
    /`content` as a JSON string/
  );
  assert.throws(
    () => builder.parseReceiverOutput(JSON.stringify({ content: '{bad' })),
    /not valid JSON/
  );
  assert.throws(
    () => builder.parseReceiverOutput(JSON.stringify({ content: '{"hooks":[]}' })),
    /hook config not found/
  );
});

test('validateOutputDir enforces basename, safety, and outside-repo via realpath', () => {
  const repoRoot = path.resolve(__dirname, '..');
  const outside = path.join(os.tmpdir(), 'codex-plugins', 'quotabar');
  assert.equal(builder.validateOutputDir(outside, repoRoot), path.resolve(outside));

  assert.throws(
    () => builder.validateOutputDir(path.join(os.tmpdir(), 'other-name'), repoRoot),
    /basename must be "quotabar"/
  );
  assert.throws(
    () => builder.validateOutputDir(path.join(repoRoot, 'dist', 'quotabar'), repoRoot),
    /outside the repository/
  );
  assert.throws(
    () => builder.validateOutputDir(`bad${String.fromCharCode(0)}path`, repoRoot),
    /unsafe output path/
  );
});

test('hooks file writes are exclusive (wx) and never replace installed definitions', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'qb-hooks-'));
  try {
    assert.doesNotThrow(() => builder.assertCanWriteHooks(dir));
    const { hooks } = builder.buildArtifacts(
      builder.parseReceiverOutput(rustGeneratorFixture().outer)
    );
    builder.writeHooksFile(dir, hooks);
    const written = JSON.parse(fs.readFileSync(path.join(dir, 'hooks', 'hooks.json'), 'utf8'));
    assert.deepEqual(Object.keys(written.hooks), builder.EVENTS);
    assert.throws(() => builder.assertCanWriteHooks(dir), /refusing to replace/);
    assert.throws(() => builder.writeHooksFile(dir, hooks), /refusing to replace/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('existing matching scaffold manifest is required; unknown paths are refused', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'qb-manifest-'));
  try {
    assert.throws(() => builder.requireExistingScaffoldManifest(dir), /refusing to create/);

    fs.mkdirSync(path.join(dir, '.codex-plugin'), { recursive: true });
    fs.writeFileSync(
      path.join(dir, '.codex-plugin', 'plugin.json'),
      `${JSON.stringify({ name: 'something-else', skills: ['x'], defaultPrompt: 'y' })}\n`,
      'utf8'
    );
    assert.throws(() => builder.requireExistingScaffoldManifest(dir), /name must be "quotabar"/);

    fs.writeFileSync(
      path.join(dir, '.codex-plugin', 'plugin.json'),
      `${JSON.stringify({ name: 'quotabar', skills: ['x'], defaultPrompt: 'y' })}\n`,
      'utf8'
    );
    const manifestPath = builder.requireExistingScaffoldManifest(dir);
    assert.equal(manifestPath, path.join(dir, '.codex-plugin', 'plugin.json'));
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
