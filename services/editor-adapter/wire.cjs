'use strict';
const crypto = require('node:crypto');
const LIMIT = 256 * 1024;
function frame(value, key) {
  const payload = Buffer.from(JSON.stringify(value), 'utf8');
  if (payload.length > LIMIT) throw new Error('frame limit');
  const header = Buffer.alloc(8);
  header.write('CUST');
  header.writeUInt32LE(payload.length, 4);
  // Match custodian-ipc: authenticate payload, not an untrusted decoded value.
  const mac = crypto.createHmac('sha256', key).update(payload).digest();
  return Buffer.concat([header, mac, payload]);
}
class Decoder {
  constructor(key) {
    this.key = key;
    this.buffer = Buffer.alloc(0);
  }
  push(chunk) {
    if (this.buffer.length + chunk.length > LIMIT + 40) throw new Error('frame limit');
    this.buffer = Buffer.concat([this.buffer, chunk]);
    if (this.buffer.length < 8) return null;
    if (this.buffer.toString('ascii', 0, 4) !== 'CUST') throw new Error('frame magic');
    const length = this.buffer.readUInt32LE(4);
    if (length > LIMIT) throw new Error('frame limit');
    if (this.buffer.length < length + 40) return null;
    if (this.buffer.length !== length + 40) throw new Error('extra frame');
    const payload = this.buffer.subarray(40);
    const expected = crypto.createHmac('sha256', this.key).update(payload).digest();
    if (!crypto.timingSafeEqual(expected, this.buffer.subarray(8, 40)))
      throw new Error('frame authentication');
    this.buffer = Buffer.alloc(0);
    return JSON.parse(payload.toString('utf8'));
  }
}
function exact(value, fields) {
  if (
    !value ||
    typeof value !== 'object' ||
    Array.isArray(value) ||
    Object.keys(value).sort().join('|') !== [...fields].sort().join('|')
  )
    throw new Error('typed schema');
}
function digest(text) {
  return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}
function editRange(text, start, end, replacement) {
  const bytes = Buffer.from(text, 'utf8');
  if (
    !Number.isSafeInteger(start) ||
    !Number.isSafeInteger(end) ||
    start < 0 ||
    start > end ||
    end > bytes.length ||
    bytes.length > 65536 ||
    typeof replacement !== 'string' ||
    Buffer.byteLength(replacement) > 65536 ||
    [...replacement].some((c) => {
      const n = c.codePointAt(0);
      return (n < 32 && ![9, 10, 13].includes(n)) || (n >= 127 && n <= 159);
    })
  )
    throw new Error('edit bounds');
  const prefix = bytes.subarray(0, start).toString('utf8');
  const selected = bytes.subarray(start, end).toString('utf8');
  if (
    !Buffer.from(prefix).equals(bytes.subarray(0, start)) ||
    !Buffer.from(selected).equals(bytes.subarray(start, end))
  )
    throw new Error('UTF-8 boundary');
  const expected = prefix + replacement + bytes.subarray(end).toString('utf8');
  if (Buffer.byteLength(expected) > 65536) throw new Error('result limit');
  return { start: prefix.length, end: prefix.length + selected.length, expected };
}
module.exports = { frame, Decoder, exact, digest, editRange, LIMIT };
