#!/usr/bin/env node
'use strict';

/**
 * QuotaBar local Codex plugin packager (prototype).
 *
 * Scaffolds a bounded, passive, local-only Codex plugin directory:
 *   <output>/
 *     .codex-plugin/plugin.json   (clean supported manifest, no hooks field)
 *     hooks/hooks.json            (exactly 8 passive session events)
 *
 * The hook shape mirrors the Rust generator in
 * src-tauri/src/task_integration.rs exactly:
 *   { "hooks": { "<Event>": [ { "hooks": [ { type, command, timeout,
 *       statusMessage, commandWindows? } ] } ] } }
 * Groups and handlers are preserved verbatim (including commandWindows and
 * timeout); the builder only ensures statusMessage is present.
 *
 * Safety contract:
 *  - no shell, no network, no dependencies
 *  - never edits global Codex config, marketplace, trust store, or user hooks
 *  - receiver is invoked once via spawnSync (shell: false, bounded timeout)
 *  - receiver output must match the canonical nested shape exactly
 *  - an existing matching scaffold manifest is REQUIRED (name "quotabar");
 *    nothing is silently created on an unknown path
 *  - hooks/hooks.json is written with flag "wx" and is never replaced
 *  - outside-repo check resolves realpath to handle junctions/symlinks
 */

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const PLUGIN_NAME = 'quotabar';
const DISPLAY_NAME = 'QuotaBar';
const VERSION = '0.1.0';
const LICENSE = 'MIT';
const REPOSITORY = 'https://github.com/kimhero110/desktoken';
const DESCRIPTION = 'QuotaBar 本地任务状态插件（仅限本地使用的原型打包，被动 hooks，不读取任何全局配置）';
const SHORT_DESCRIPTION = 'QuotaBar 本地任务状态（仅本地）';
const LONG_DESCRIPTION =
  'QuotaBar 本地任务状态插件：通过 8 个被动会话事件（UserPromptSubmit、PreToolUse、PostToolUse、PermissionRequest、Stop、SessionStart、SessionEnd、Interrupt）向本机 QuotaBar 上报任务状态。仅限本地使用，不读取任何全局配置。';
const AUTHOR_NAME = 'kimhero110';
const INTERFACE_CATEGORY = 'Productivity';
const STATUS_PREFIX = 'QuotaBar local task status: ';
const RECEIVER_TIMEOUT_MS = 15000;

const EVENTS = Object.freeze([
  'UserPromptSubmit',
  'PreToolUse',
  'PostToolUse',
  'PermissionRequest',
  'Stop',
  'SessionStart',
  'SessionEnd',
  'Interrupt',
]);

/**
 * Parse receiver stdout: outer JSON object whose `content` is a JSON string
 * containing `{ "hooks": { "<Event>": [ { "hooks": [handler] } ] } }`.
 */
function parseReceiverOutput(stdout) {
  if (typeof stdout !== 'string' || !stdout.trim()) {
    throw new Error('receiver produced no stdout');
  }
  let outer;
  try {
    outer = JSON.parse(stdout);
  } catch (err) {
    throw new Error(`receiver stdout is not valid JSON: ${err.message}`);
  }
  if (outer === null || typeof outer !== 'object' || Array.isArray(outer)) {
    throw new Error('receiver stdout must be a JSON object');
  }
  if (typeof outer.content !== 'string' || !outer.content.trim()) {
    throw new Error('receiver output must carry `content` as a JSON string');
  }
  let content;
  try {
    content = JSON.parse(outer.content);
  } catch (err) {
    throw new Error(`receiver content is not valid JSON: ${err.message}`);
  }
  if (
    content === null ||
    typeof content !== 'object' ||
    Array.isArray(content) ||
    content.hooks === null ||
    typeof content.hooks !== 'object' ||
    Array.isArray(content.hooks)
  ) {
    throw new Error('hook config not found in receiver content (expected {"hooks":{...}})');
  }
  return content.hooks;
}

function validateHandler(handler, event) {
  if (handler === null || typeof handler !== 'object' || Array.isArray(handler)) {
    throw new Error(`event ${event}: handler must be an object`);
  }
  if (handler.type !== 'command') {
    throw new Error(`event ${event}: handler type must be "command" (got ${JSON.stringify(handler.type)})`);
  }
  if (typeof handler.command !== 'string' || !handler.command.trim()) {
    throw new Error(`event ${event}: handler has no usable command`);
  }
}

/**
 * Validate the canonical nested shape against the Rust generator:
 * exactly the 8 passive events, each exactly one group holding exactly one
 * command handler. Returns a deep copy with every group/handler field
 * preserved (matcher, timeout, commandWindows, ...).
 */
function normalizeHookConfig(raw) {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    throw new Error('hook config must be an object mapping events to group arrays');
  }
  const keys = Object.keys(raw).filter((k) => raw[k] !== undefined && raw[k] !== null);
  const unknown = keys.filter((k) => !EVENTS.includes(k));
  if (unknown.length > 0) {
    throw new Error(
      `unexpected hook events are not allowed (passive session events only): ${unknown.join(', ')}`
    );
  }
  const missing = EVENTS.filter((e) => !keys.includes(e));
  if (missing.length > 0) {
    throw new Error(`missing required hook events: ${missing.join(', ')}`);
  }
  const normalized = {};
  for (const event of EVENTS) {
    const groups = raw[event];
    if (!Array.isArray(groups)) {
      throw new Error(`event ${event}: value must be an array of groups`);
    }
    if (groups.length !== 1) {
      throw new Error(`event ${event}: expected exactly 1 group (got ${groups.length})`);
    }
    const group = groups[0];
    if (group === null || typeof group !== 'object' || Array.isArray(group)) {
      throw new Error(`event ${event}: group must be an object`);
    }
    if (!Array.isArray(group.hooks)) {
      throw new Error(`event ${event}: group must carry a "hooks" array`);
    }
    if (group.hooks.length !== 1) {
      throw new Error(`event ${event}: group must hold exactly 1 handler (got ${group.hooks.length})`);
    }
    validateHandler(group.hooks[0], event);
    normalized[event] = [{ ...group, hooks: [{ ...group.hooks[0] }] }];
  }
  return normalized;
}

function buildArtifacts(hookConfig) {
  const normalized = normalizeHookConfig(hookConfig);
  const hooks = { hooks: {} };
  for (const event of EVENTS) {
    const group = normalized[event][0];
    const handler = { ...group.hooks[0] };
    handler.statusMessage = `${STATUS_PREFIX}${event}`;
    hooks.hooks[event] = [{ ...group, hooks: [handler] }];
  }
  const manifest = {
    name: PLUGIN_NAME,
    version: VERSION,
    description: DESCRIPTION,
    author: { name: AUTHOR_NAME },
    repository: REPOSITORY,
    license: LICENSE,
    interface: {
      displayName: DISPLAY_NAME,
      shortDescription: SHORT_DESCRIPTION,
      longDescription: LONG_DESCRIPTION,
      developerName: AUTHOR_NAME,
      category: INTERFACE_CATEGORY,
      capabilities: [],
      defaultPrompt: [],
    },
  };
  return { hooks, manifest };
}

function realpathNearest(target) {
  let current = path.resolve(target);
  const remainder = [];
  while (!fs.existsSync(current)) {
    remainder.unshift(path.basename(current));
    const parent = path.dirname(current);
    if (parent === current) {
      return path.resolve(target);
    }
    current = parent;
  }
  let real;
  try {
    real = fs.realpathSync(current);
  } catch {
    return path.resolve(target);
  }
  for (const part of remainder) {
    real = path.join(real, part);
  }
  return real;
}

function validateOutputDir(outputArg, repoRoot) {
  if (typeof outputArg !== 'string' || !outputArg.trim()) {
    throw new Error('output directory is required');
  }
  if (outputArg.includes('\0')) {
    throw new Error('unsafe output path');
  }
  const resolved = path.resolve(outputArg);
  if (path.parse(resolved).root === resolved) {
    throw new Error('unsafe output path (filesystem root)');
  }
  if (path.basename(resolved) !== PLUGIN_NAME) {
    throw new Error(
      `output directory basename must be "${PLUGIN_NAME}" (got "${path.basename(resolved)}")`
    );
  }
  const realOut = realpathNearest(resolved);
  const realRepo = fs.realpathSync(repoRoot);
  const rel = path.relative(realRepo, realOut);
  const insideRepo = rel === '' || (!rel.startsWith(`..${path.sep}`) && rel !== '..' && !path.isAbsolute(rel));
  if (insideRepo) {
    throw new Error(
      'output directory must live outside the repository so the local receiver path is never committed'
    );
  }
  return resolved;
}

function hooksPathOf(outputDir) {
  return path.join(outputDir, 'hooks', 'hooks.json');
}

function assertCanWriteHooks(outputDir) {
  if (fs.existsSync(hooksPathOf(outputDir))) {
    throw new Error(
      `refusing to replace existing hooks file: ${hooksPathOf(outputDir)} (installed hook definitions must not be overwritten)`
    );
  }
}

function writeHooksFile(outputDir, hooks) {
  fs.mkdirSync(path.join(outputDir, 'hooks'), { recursive: true });
  assertCanWriteHooks(outputDir);
  fs.writeFileSync(hooksPathOf(outputDir), `${JSON.stringify(hooks, null, 2)}\n`, {
    encoding: 'utf8',
    flag: 'wx',
  });
}

/**
 * An existing matching scaffold manifest is REQUIRED: plugin.json must
 * already exist with name "quotabar". Nothing is created silently on an
 * unknown path, and no scaffold metadata (skills, defaultPrompt, ...) is
 * merged — a clean supported manifest is written instead.
 */
function requireExistingScaffoldManifest(outputDir) {
  const manifestPath = path.join(outputDir, '.codex-plugin', 'plugin.json');
  if (!fs.existsSync(manifestPath)) {
    throw new Error(
      `expected an existing scaffold manifest at ${manifestPath} with name "${PLUGIN_NAME}" — refusing to create a plugin on an unknown path`
    );
  }
  let existing;
  try {
    existing = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  } catch (err) {
    throw new Error(`existing plugin.json is not valid JSON: ${err.message}`);
  }
  if (existing === null || typeof existing !== 'object' || Array.isArray(existing)) {
    throw new Error('existing plugin.json must be a JSON object');
  }
  if (existing.name !== PLUGIN_NAME) {
    throw new Error(
      `existing .codex-plugin/plugin.json name must be "${PLUGIN_NAME}" (got "${existing.name}")`
    );
  }
  return manifestPath;
}

function runReceiver(receiverExe) {
  const result = spawnSync(receiverExe, ['task-config', 'codex'], {
    shell: false,
    timeout: RECEIVER_TIMEOUT_MS,
    encoding: 'utf8',
    windowsHide: true,
  });
  if (result.error) {
    throw new Error(`failed to launch receiver: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const stderr = result.stderr ? String(result.stderr).trim().slice(0, 500) : '';
    throw new Error(`receiver exited with status ${result.status}${stderr ? `: ${stderr}` : ''}`);
  }
  return parseReceiverOutput(result.stdout);
}

function usage() {
  console.log(
    [
      'Usage: node scripts/build-codex-plugin.cjs <receiverExe> <outputDir>',
      '',
      '  receiverExe  absolute path to the QuotaBar receiver executable (must exist)',
      '  outputDir    existing plugin scaffold directory, basename must be "quotabar",',
      '               must already contain .codex-plugin/plugin.json with name "quotabar",',
      '               and must live OUTSIDE this repository',
      '',
      'Produces <outputDir>/hooks/hooks.json and rewrites .codex-plugin/plugin.json',
      'as a clean supported manifest. Never replaces an existing hooks/hooks.json.',
    ].join('\n')
  );
}

function main(argv) {
  const [receiverExe, outputArg] = argv;
  if (!receiverExe || !outputArg || argv.includes('-h') || argv.includes('--help')) {
    usage();
    process.exitCode = receiverExe || outputArg ? 1 : 0;
    return;
  }
  if (!path.isAbsolute(receiverExe)) {
    throw new Error('receiverExe must be an absolute path');
  }
  if (!fs.existsSync(receiverExe) || !fs.statSync(receiverExe).isFile()) {
    throw new Error(`receiverExe does not exist or is not a file: ${receiverExe}`);
  }
  const repoRoot = path.resolve(__dirname, '..');
  const outputDir = validateOutputDir(outputArg, repoRoot);
  const manifestPath = requireExistingScaffoldManifest(outputDir);
  assertCanWriteHooks(outputDir);

  const rawHookConfig = runReceiver(receiverExe);
  const { hooks, manifest } = buildArtifacts(rawHookConfig);

  writeHooksFile(outputDir, hooks);
  fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');

  console.log(`QuotaBar plugin scaffolded at: ${outputDir}`);
  console.log(`  hooks/hooks.json         (${EVENTS.length} passive events, 1 group x 1 handler each)`);
  console.log('  .codex-plugin/plugin.json (clean supported manifest, no hooks field)');
  console.log('');
  console.log('Installation (local only):');
  console.log('  1. Register/copy this plugin through Codex normal plugin flow.');
  console.log('  2. BEFORE trusting the plugin, remove/disable your own user-level QuotaBar');
  console.log('     hooks so plugin and user hooks never both execute.');
  console.log('  3. Approve the plugin through Codex trust/approval — never bypass it.');
  console.log('  4. If anything looks wrong, re-enable your preserved user hooks to switch back.');
  console.log('The generated files embed a local absolute receiver path — never commit them.');
}

if (require.main === module) {
  try {
    main(process.argv.slice(2));
  } catch (err) {
    console.error(`build-codex-plugin: ${err.message}`);
    process.exitCode = 1;
  }
}

module.exports = {
  EVENTS,
  STATUS_PREFIX,
  PLUGIN_NAME,
  DISPLAY_NAME,
  VERSION,
  LICENSE,
  REPOSITORY,
  DESCRIPTION,
  parseReceiverOutput,
  normalizeHookConfig,
  buildArtifacts,
  validateOutputDir,
  assertCanWriteHooks,
  writeHooksFile,
  requireExistingScaffoldManifest,
};
