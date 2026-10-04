import { createHmac, timingSafeEqual } from 'node:crypto';

// Byte-compatible with the `custodian-ipc` crate:
// "CUST" | len: u32 LE | HMAC-SHA256(payload) | payload (UTF-8 JSON).

const MAGIC = Buffer.from('CUST');
const HEADER_SIZE = 4 + 4 + 32;
export const MAX_FRAME_SIZE = 10 * 1024 * 1024;

function sign(key: Buffer, payload: Buffer): Buffer {
  return createHmac('sha256', key).update(payload).digest();
}

export function writeFrame(payload: unknown, key: Buffer): Buffer {
  const body = Buffer.from(JSON.stringify(payload), 'utf-8');
  if (body.length > MAX_FRAME_SIZE) throw new Error('Frame too large');

  const len = Buffer.alloc(4);
  len.writeUInt32LE(body.length, 0);
  return Buffer.concat([MAGIC, len, sign(key, body), body]);
}

export function readFrame(buffer: Buffer, key: Buffer): unknown {
  if (buffer.length < HEADER_SIZE) throw new Error('Buffer too small');
  if (!buffer.subarray(0, 4).equals(MAGIC)) throw new Error('Invalid magic');

  const len = buffer.readUInt32LE(4);
  if (len > MAX_FRAME_SIZE) throw new Error('Frame too large');
  if (buffer.length < HEADER_SIZE + len) throw new Error('Incomplete frame');

  const signature = buffer.subarray(8, HEADER_SIZE);
  const body = buffer.subarray(HEADER_SIZE, HEADER_SIZE + len);
  if (!timingSafeEqual(sign(key, body), signature)) throw new Error('Invalid signature');

  return JSON.parse(body.toString('utf-8'));
}
