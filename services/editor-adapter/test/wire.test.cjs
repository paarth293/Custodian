'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { frame, Decoder, exact, editRange, LIMIT } = require('../wire.cjs');
const key = Buffer.alloc(32, 11);
test('authentication rejects corrupted payload and oversized headers before allocation', () => {
  const bytes = frame({ value: 'private fixture' }, key);
  bytes[bytes.length - 2] ^= 1;
  assert.throws(() => new Decoder(key).push(bytes), /authentication/);
  const header = Buffer.alloc(8);
  header.write('CUST');
  header.writeUInt32LE(LIMIT + 1, 4);
  assert.throws(() => new Decoder(key).push(header), /limit/);
});
test('partial frames remain pending and strict shape refuses shell/extra fields', () => {
  const bytes = frame({ seq: 1 }, key),
    decoder = new Decoder(key);
  assert.equal(decoder.push(bytes.subarray(0, 9)), null);
  assert.deepEqual(decoder.push(bytes.subarray(9)), { seq: 1 });
  assert.throws(() => exact({ kind: 'edit', shell: 'anything' }, ['kind']), /schema/);
});
test('UTF-8 offsets map to UTF-16 without splitting emoji or changing unrelated lines', () => {
  const text = 'Heading\n😀First line\nSecond line\n';
  const start = Buffer.byteLength('Heading\n😀');
  const changed = editRange(text, start, start + 10, 'Revised line');
  assert.equal(changed.start, 10);
  assert.equal(changed.expected, 'Heading\n😀Revised line\nSecond line\n');
  assert.throws(() => editRange(text, 9, 12, 'x'), /boundary/);
  assert.throws(() => editRange(text, 0, 0, '\0'), /bounds/);
});
