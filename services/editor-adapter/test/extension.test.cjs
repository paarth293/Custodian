'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { EventEmitter } = require('node:events');
const wire = require('../wire.cjs');

function harness() {
  const session = '00000000-0000-4000-8000-000000000001';
  const key = Buffer.alloc(32, 17);
  const target = path.resolve('synthetic-document.txt');
  const descriptor = Buffer.from(
    JSON.stringify({
      schema: 1,
      session,
      target,
      pipe: String.raw`\\.\pipe\custodian-editor-${session}`,
      secret: key.toString('hex'),
      expires: Date.now() + 60000,
    }),
  );
  let reads = 0,
    edits = 0,
    commands = 0,
    unlinked = false;
  const document = {
    uri: { toString: () => target },
    isClosed: false,
    languageId: 'plaintext',
    version: 1,
    isDirty: false,
    getText: () => {
      reads++;
      return 'Original public text';
    },
    positionAt: (n) => n,
    save: async () => true,
  };
  const editor = {
    document,
    edit: async () => {
      edits++;
      return true;
    },
  };
  const vscode = {
    Uri: { file: (value) => ({ toString: () => value }) },
    workspace: {
      openTextDocument: async (uri) => {
        assert.equal(uri.toString(), target);
        return document;
      },
    },
    window: {
      activeTextEditor: editor,
      showTextDocument: async () => editor,
      tabGroups: { all: [], close: async () => true },
    },
    commands: {
      executeCommand: async () => {
        commands++;
      },
    },
    Range: class {},
    version: '1.140.0',
  };
  const socket = new EventEmitter();
  socket.frames = [];
  socket.closed = false;
  socket.write = (bytes) => {
    socket.frames.push(new wire.Decoder(key).push(bytes));
  };
  socket.setTimeout = () => {};
  socket.destroy = () => {
    socket.closed = true;
    socket.emit('close');
  };
  const fakeFs = {
    lstatSync: () => ({ isSymbolicLink: () => false }),
    openSync: () => 1,
    fstatSync: () => ({ isFile: () => true, size: descriptor.length, nlink: 1 }),
    readSync: (_, buffer) => {
      descriptor.copy(buffer);
      return descriptor.length;
    },
    closeSync: () => {},
    unlinkSync: () => {
      unlinked = true;
    },
  };
  const exported = {};
  const context = vm.createContext({
    exports: exported,
    Buffer,
    Date,
    setTimeout: () => {},
    process: {
      env: { CUSTODIAN_EDITOR_DESCRIPTOR: path.resolve('bootstrap.json') },
      pid: 1,
      ppid: 2,
    },
    require: (name) =>
      ({
        vscode,
        'node:fs': fakeFs,
        'node:path': path,
        'node:net': { createConnection: () => socket },
        './wire.cjs': wire,
      })[name],
  });
  vm.runInContext(fs.readFileSync(path.join(__dirname, '../extension.cjs'), 'utf8'), context);
  exported.activate({ subscriptions: [] });
  let sequence = 0;
  return {
    vscode,
    document,
    socket,
    key,
    session,
    counts: () => ({ reads, edits, commands, unlinked }),
    async send(command) {
      const before = socket.frames.length;
      socket.emit('data', wire.frame({ session, seq: ++sequence, command }, key));
      await new Promise((resolve) => setImmediate(resolve));
      return socket.frames.length > before ? socket.frames.at(-1).result : null;
    },
  };
}
test('switching to a private tab refuses before any further content read or edit', async () => {
  const h = harness();
  assert.equal((await h.send({ kind: 'bind', documentId: 'native-original' })).kind, 'bound');
  const before = h.counts().reads;
  h.vscode.window.activeTextEditor = {
    document: {
      getText: () => {
        throw new Error('Private content must not be touched');
      },
    },
  };
  assert.equal((await h.send({ kind: 'inspect', version: 1 })).kind, 'refused');
  assert.equal(
    (
      await h.send({
        kind: 'edit',
        version: 1,
        digest: wire.digest('Original public text'),
        start: 0,
        end: 1,
        replacement: 'x',
      })
    ).kind,
    'refused',
  );
  assert.equal(h.counts().reads, before);
  assert.equal(h.counts().edits, 0);
});
test('stale versions and arbitrary commands never reach document editing or command execution', async () => {
  const h = harness();
  await h.send({ kind: 'bind', documentId: 'native-original' });
  h.document.version = 2;
  const before = h.counts().reads;
  assert.equal((await h.send({ kind: 'inspect', version: 1 })).kind, 'refused');
  assert.equal((await h.send({ kind: 'shell', command: 'anything' })).kind, 'refused');
  assert.equal(h.counts().reads, before);
  assert.equal(h.counts().commands, 0);
});
test('bad authentication closes transport without document observation', async () => {
  const h = harness();
  h.socket.emit(
    'data',
    wire.frame(
      { session: h.session, seq: 1, command: { kind: 'bind', documentId: 'native-original' } },
      Buffer.alloc(32, 18),
    ),
  );
  assert.equal(h.socket.closed, true);
  assert.equal(h.counts().reads, 0);
  assert.equal(h.counts().unlinked, true);
});
