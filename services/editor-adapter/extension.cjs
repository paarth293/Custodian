'use strict';
const vscode = require('vscode');
const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const { frame, Decoder, exact, digest, editRange } = require('./wire.cjs');

function descriptor() {
  const filename = process.env.CUSTODIAN_EDITOR_DESCRIPTOR;
  if (!filename) return null;
  if (!path.isAbsolute(filename) || fs.lstatSync(filename).isSymbolicLink())
    throw new Error('descriptor');
  const fd = fs.openSync(filename, 'r');
  let value;
  try {
    const info = fs.fstatSync(fd);
    if (!info.isFile() || info.size > 4096 || info.nlink !== 1) throw new Error('descriptor');
    const bytes = Buffer.alloc(info.size);
    if (fs.readSync(fd, bytes, 0, bytes.length, 0) !== bytes.length) throw new Error('descriptor');
    value = JSON.parse(bytes.toString('utf8'));
    bytes.fill(0);
  } finally {
    fs.closeSync(fd);
  }
  exact(value, ['schema', 'session', 'target', 'pipe', 'secret', 'expires']);
  if (
    value.schema !== 1 ||
    !/^[a-f0-9-]{36}$/.test(value.session) ||
    !/^[a-f0-9]{64}$/.test(value.secret) ||
    !path.isAbsolute(value.target) ||
    value.pipe !== String.raw`\\.\pipe\custodian-editor-${value.session}` ||
    !Number.isSafeInteger(value.expires) ||
    value.expires <= Date.now() ||
    value.expires > Date.now() + 1800000
  )
    throw new Error('descriptor');
  fs.unlinkSync(filename); // The bootstrap secret is never left on disk after handoff.
  return value;
}

exports.activate = function activate(context) {
  let config;
  try {
    config = descriptor();
  } catch {
    return;
  }
  if (!config) return;
  const key = Buffer.from(config.secret, 'hex');
  config.secret = '';
  const decoder = new Decoder(key);
  const socket = net.createConnection(config.pipe);
  let document, editor, documentId;
  let sequence = 0,
    busy = false,
    revoked = false;
  const uri = vscode.Uri.file(config.target);
  const same = () =>
    document &&
    !document.isClosed &&
    document.uri.toString() === uri.toString() &&
    editor &&
    editor.document === document &&
    vscode.window.activeTextEditor === editor;
  const stop = () => {
    revoked = true;
    socket.destroy();
    key.fill(0);
  };
  context.subscriptions.push({ dispose: stop });
  socket.on('error', stop);
  socket.on('close', () => {
    revoked = true;
    key.fill(0);
  });
  socket.setTimeout(120000, stop);
  socket.on('connect', () =>
    socket.write(
      frame(
        {
          session: config.session,
          seq: 0,
          result: {
            kind: 'hello',
            pid: process.pid,
            parentPid: process.ppid,
            version: vscode.version,
          },
        },
        key,
      ),
    ),
  );

  async function perform(command) {
    if (revoked || Date.now() > config.expires) throw new Error('revoked');
    if (command.kind === 'bind') {
      exact(command, ['kind', 'documentId']);
      if (
        document ||
        typeof command.documentId !== 'string' ||
        command.documentId.length > 256 ||
        !command.documentId.trim() ||
        [...command.documentId].some((c) => {
          const n = c.codePointAt(0);
          return n < 32 || (n >= 127 && n <= 159);
        })
      )
        throw new Error('binding');
      documentId = command.documentId;
      document = await vscode.workspace.openTextDocument(uri);
      if (document.languageId !== 'plaintext')
        document = await vscode.languages.setTextDocumentLanguage(document, 'plaintext');
      if (revoked) throw new Error('revoked');
      editor = await vscode.window.showTextDocument(document, {
        preview: false,
        preserveFocus: true,
      });
      if (!same()) throw new Error('binding');
      return { kind: 'bound', documentId, version: document.version, dirty: document.isDirty };
    }
    if (command.kind === 'close_window') {
      exact(command, ['kind']);
      if (vscode.window.tabGroups.all.some((group) => group.tabs.length))
        throw new Error('unrelated tabs remain');
      return { kind: 'window_close_ready' };
    }
    if (!same()) throw new Error('stale document');
    if (command.kind === 'metadata') {
      exact(command, ['kind']);
      return { kind: 'metadata', documentId, version: document.version, dirty: document.isDirty };
    }
    if (!Number.isSafeInteger(command.version) || document.version !== command.version)
      throw new Error('stale version');
    if (command.kind === 'inspect') {
      exact(command, ['kind', 'version']);
      const text = document.getText();
      if (Buffer.byteLength(text) > 65536) throw new Error('observation limit');
      return {
        kind: 'snapshot',
        documentId,
        version: document.version,
        text,
        digest: digest(text),
      };
    }
    if (command.kind === 'edit') {
      exact(command, ['kind', 'version', 'digest', 'start', 'end', 'replacement']);
      const text = document.getText();
      if (digest(text) !== command.digest) throw new Error('stale content');
      const selection = editRange(text, command.start, command.end, command.replacement);
      const accepted = await editor.edit(
        (builder) =>
          builder.replace(
            new vscode.Range(
              document.positionAt(selection.start),
              document.positionAt(selection.end),
            ),
            command.replacement,
          ),
        { undoStopBefore: true, undoStopAfter: true },
      );
      if (!accepted || revoked || !same() || document.getText() !== selection.expected)
        throw new Error('edit uncertain');
      return {
        kind: 'edited',
        documentId,
        version: document.version,
        digest: digest(selection.expected),
      };
    }
    if (command.kind === 'save') {
      exact(command, ['kind', 'version', 'digest']);
      if (digest(document.getText()) !== command.digest) throw new Error('stale content');
      if (
        !(await document.save()) ||
        revoked ||
        !same() ||
        document.version !== command.version ||
        document.isDirty
      )
        throw new Error('save uncertain');
      return {
        kind: 'saved',
        documentId,
        version: document.version,
        digest: digest(document.getText()),
      };
    }
    if (command.kind === 'close') {
      exact(command, ['kind', 'version', 'digest']);
      if (document.isDirty || digest(document.getText()) !== command.digest)
        throw new Error('newer work preserved');
      const tab = vscode.window.tabGroups.all
        .flatMap((group) => group.tabs)
        .find(
          (tab) =>
            tab.input instanceof vscode.TabInputText && tab.input.uri.toString() === uri.toString(),
        );
      if (!tab || !(await vscode.window.tabGroups.close(tab, true)))
        throw new Error('close uncertain');
      return { kind: 'closed', documentId };
    }
    throw new Error('unavailable operation');
  }
  socket.on('data', (chunk) => {
    try {
      const request = decoder.push(chunk);
      if (request === null) return;
      exact(request, ['session', 'seq', 'command']);
      if (
        busy ||
        revoked ||
        request.session !== config.session ||
        request.seq !== sequence + 1 ||
        request.seq > 64
      )
        throw new Error('request sequence');
      sequence = request.seq;
      busy = true;
      void perform(request.command)
        .then((result) => {
          if (!revoked)
            socket.write(frame({ session: config.session, seq: sequence, result }, key));
          if (result.kind === 'window_close_ready' && !revoked) {
            // Fixed, profile-local command only after all tabs are gone. No wire
            // operation can choose a command name, URI, executable or shell text.
            setTimeout(() => {
              void vscode.commands.executeCommand('workbench.action.closeWindow');
            }, 100);
          }
        })
        .catch(() => {
          if (!revoked)
            socket.write(
              frame(
                {
                  session: config.session,
                  seq: sequence,
                  result: {
                    kind: 'refused',
                    detail:
                      'Bound document changed, permission expired, or operation unavailable. Inspect recovery state; no automatic replay.',
                  },
                },
                key,
              ),
            );
        })
        .finally(() => {
          busy = false;
        });
    } catch {
      stop();
    }
  });
};
