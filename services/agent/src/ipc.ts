import * as crypto from 'crypto';

const MAGIC = Buffer.from('CUST');
const MAX_FRAME_SIZE = 10 * 1024 * 1024;

export function writeFrame(payload: unknown, key: Buffer): Buffer {
    const jsonStr = JSON.stringify(payload);
    const jsonBytes = Buffer.from(jsonStr, 'utf-8');
    const len = jsonBytes.length;
    
    if (len > MAX_FRAME_SIZE) throw new Error('Frame too large');
    
    const hmac = crypto.createHmac('sha256', key);
    hmac.update(jsonBytes);
    const signature = hmac.digest();
    
    const lenBuf = Buffer.alloc(4);
    lenBuf.writeUInt32LE(len, 0);
    
    return Buffer.concat([MAGIC, lenBuf, signature, jsonBytes]);
}

export function readFrame(buffer: Buffer, key: Buffer): unknown {
    if (buffer.length < 40) throw new Error('Buffer too small');
    
    const magic = buffer.subarray(0, 4);
    if (!magic.equals(MAGIC)) throw new Error('Invalid magic');
    
    const len = buffer.readUInt32LE(4);
    if (len > MAX_FRAME_SIZE) throw new Error('Frame too large');
    if (buffer.length < 40 + len) throw new Error('Incomplete frame');
    
    const expectedSig = buffer.subarray(8, 40);
    const payloadBytes = buffer.subarray(40, 40 + len);
    
    const hmac = crypto.createHmac('sha256', key);
    hmac.update(payloadBytes);
    const actualSig = hmac.digest();
    
    if (!actualSig.equals(expectedSig)) throw new Error('Invalid signature');
    
    return JSON.parse(payloadBytes.toString('utf-8'));
}
