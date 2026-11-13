import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, lstatSync, mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REPOSITORY = 'anjing-le/happy-creator-journey';
const ENDPOINT = `https://github.com/${REPOSITORY}/releases/latest/download/latest.json`;
const PLATFORMS = ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64'];
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, value, index, args) => {
  if (value.startsWith('--')) pairs.push([value.slice(2), args[index + 1]?.startsWith('--') ? true : (args[index + 1] ?? true)]);
  return pairs;
}, []));

function json(file) {
  return JSON.parse(readFileSync(file, 'utf8'));
}

function versions() {
  const pkg = json(path.join(ROOT, 'package.json'));
  const config = json(path.join(ROOT, 'src-tauri/tauri.conf.json'));
  const cargo = readFileSync(path.join(ROOT, 'src-tauri/Cargo.toml'), 'utf8');
  const cargoPackage = cargo.split('[package]')[1]?.split(/^\[/m)[0];
  const rustVersion = cargoPackage?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  assert(SEMVER.test(pkg.version), 'package.json version must be a stable X.Y.Z version');
  assert.equal(config.version, pkg.version, 'Tauri and frontend versions must match');
  assert.equal(rustVersion, pkg.version, 'Rust and frontend versions must match');
  assert.equal(config.identifier, 'cc.anjing.creatorx', 'application identifier changed');
  assert.deepEqual(config.plugins?.updater?.endpoints, [ENDPOINT], 'unexpected updater endpoint');
  assert.equal(config.bundle?.createUpdaterArtifacts, true, 'signed updater artifacts must be enabled');
  if (options.version) assert.equal(options.version, pkg.version, 'requested release version does not match source');
  if (options.tag) assert.equal(options.tag, `creatorx-v${pkg.version}`, 'tag does not match source version');
  return { version: pkg.version, config };
}

function decodeBase64(value, description) {
  assert(typeof value === 'string' && value.length > 0, `${description} is missing`);
  const normalized = value.trim();
  assert(/^[A-Za-z0-9+/]+={0,2}$/.test(normalized), `${description} must be base64`);
  const bytes = Buffer.from(normalized, 'base64');
  assert.equal(bytes.toString('base64'), normalized, `${description} contains invalid base64`);
  return bytes;
}

function publicKey(config) {
  const text = decodeBase64(config.plugins.updater.pubkey, 'updater public key').toString('utf8');
  const lines = text.trimEnd().split(/\r?\n/);
  assert.equal(lines.length, 2, 'updater public key must contain a minisign public key');
  assert(lines[0].startsWith('untrusted comment: '), 'invalid public key comment');
  const bytes = decodeBase64(lines[1], 'minisign public key');
  assert.equal(bytes.length, 42, 'minisign public key must contain 42 bytes');
  assert(['Ed', 'ED'].includes(bytes.subarray(0, 2).toString('ascii')), 'unsupported public key algorithm');
  return lines[1];
}

function signature(value) {
  const text = decodeBase64(value.trim(), 'update signature').toString('utf8');
  const lines = text.trimEnd().split(/\r?\n/);
  assert.equal(lines.length, 4, 'update signature must be a complete minisign signature');
  assert(lines[0].startsWith('untrusted comment: '), 'invalid signature comment');
  assert(lines[2].startsWith('trusted comment: '), 'invalid trusted signature comment');
  const payload = decodeBase64(lines[1], 'minisign signature');
  assert.equal(payload.length, 74, 'minisign signature must contain 74 bytes');
  assert(['Ed', 'ED'].includes(payload.subarray(0, 2).toString('ascii')), 'unsupported signature algorithm');
  assert.equal(decodeBase64(lines[3], 'minisign global signature').length, 64, 'invalid minisign global signature');
  return text;
}

function regularFile(file) {
  assert(existsSync(file) && lstatSync(file).isFile(), `missing regular file: ${path.basename(file)}`);
  assert(lstatSync(file).size > 0, `empty artifact: ${path.basename(file)}`);
}

function findOne(directory, expression) {
  const matches = readdirSync(directory).filter(name => expression.test(name));
  assert.equal(matches.length, 1, `expected one ${expression} artifact in ${directory}`);
  const file = path.join(directory, matches[0]);
  regularFile(file);
  return file;
}

function assetNames(platform, version) {
  const arch = platform === 'darwin-aarch64' ? 'aarch64' : 'x64';
  const prefix = `creatorx_${version}_${arch}`;
  return platform.startsWith('darwin-')
    ? { updater: `${prefix}.app.tar.gz`, installer: `${prefix}.dmg` }
    : { updater: `${prefix}-setup.exe`, installer: `${prefix}-setup.exe` };
}

function stage(version, config) {
  const platform = options.platform;
  assert(PLATFORMS.includes(platform), 'unsupported release platform');
  publicKey(config);
  const root = path.resolve(options['bundle-dir']);
  const output = path.resolve(options['output-dir']);
  mkdirSync(output, { recursive: true });
  const mac = platform.startsWith('darwin-');
  const updater = findOne(path.join(root, mac ? 'macos' : 'nsis'), mac ? /\.app\.tar\.gz$/ : /-setup\.exe$/);
  const installer = mac ? findOne(path.join(root, 'dmg'), /\.dmg$/) : updater;
  regularFile(`${updater}.sig`);
  const sig = readFileSync(`${updater}.sig`, 'utf8').trim();
  signature(sig);
  const names = assetNames(platform, version);
  copyFileSync(updater, path.join(output, names.updater));
  copyFileSync(`${updater}.sig`, path.join(output, `${names.updater}.sig`));
  if (installer !== updater) copyFileSync(installer, path.join(output, names.installer));
  writeFileSync(path.join(output, `${platform}.json`), JSON.stringify({
    platform, version,
    url: `https://github.com/${REPOSITORY}/releases/download/creatorx-v${version}/${names.updater}`,
    signature: sig,
  }, null, 2) + '\n');
}

function verifySignedArtifact(file, encodedSignature, key) {
  const temporary = mkdtempSync(path.join(tmpdir(), 'creatorx-verify-'));
  try {
    const sigFile = path.join(temporary, 'artifact.minisig');
    writeFileSync(sigFile, signature(encodedSignature));
    const result = spawnSync('minisign', ['-Vm', file, '-x', sigFile, '-P', key], { encoding: 'utf8' });
    assert(!result.error, 'minisign is required to verify release artifacts');
    assert.equal(result.status, 0, `invalid update signature: ${path.basename(file)}`);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

function assemble(version, config) {
  const directory = path.resolve(options['artifacts-dir']);
  const output = path.resolve(options['output-dir']);
  assert.notEqual(directory, output, 'assembly output must differ from downloaded artifacts');
  assert(!existsSync(output) || readdirSync(output).length === 0, 'assembly output must be empty');
  const key = publicKey(config);
  const notes = readFileSync(path.resolve(options['notes-file']), 'utf8').trim();
  assert(notes.length > 0 && [...notes].length <= 8000, 'release notes must contain 1 to 8000 characters');
  assert(!/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/u.test(notes), 'invalid control character in release notes');
  const platforms = {};
  const expectedFiles = new Set();
  const distributionFiles = new Set();
  for (const platform of PLATFORMS) {
    const fragmentName = `${platform}.json`;
    regularFile(path.join(directory, fragmentName));
    const fragment = json(path.join(directory, fragmentName));
    const names = assetNames(platform, version);
    assert.equal(fragment.platform, platform, 'release fragment platform mismatch');
    assert.equal(fragment.version, version, 'release fragment version mismatch');
    assert.equal(fragment.url, `https://github.com/${REPOSITORY}/releases/download/creatorx-v${version}/${names.updater}`, 'unexpected artifact URL');
    expectedFiles.add(fragmentName);
    for (const name of [names.updater, `${names.updater}.sig`, names.installer]) {
      regularFile(path.join(directory, name));
      expectedFiles.add(name);
      distributionFiles.add(name);
    }
    assert.equal(readFileSync(path.join(directory, `${names.updater}.sig`), 'utf8').trim(), fragment.signature, 'manifest signature does not match signature asset');
    verifySignedArtifact(path.join(directory, names.updater), fragment.signature, key);
    platforms[platform] = { url: fragment.url, signature: fragment.signature };
  }
  assert.deepEqual(readdirSync(directory).sort(), [...expectedFiles].sort(), 'unexpected or duplicate release artifacts');
  mkdirSync(output, { recursive: true });
  for (const name of distributionFiles) copyFileSync(path.join(directory, name), path.join(output, name));
  writeFileSync(path.join(output, 'latest.json'), JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2) + '\n');
}

try {
  const { version, config } = versions();
  if (options.release) publicKey(config);
  if (options.stage) stage(version, config);
  if (options.assemble) assemble(version, config);
  console.log(`Release validation passed: ${version}`);
} catch (error) {
  console.error(`Release validation failed: ${error.message}`);
  process.exitCode = 1;
}
