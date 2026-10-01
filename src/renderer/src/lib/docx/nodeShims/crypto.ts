/** Browser stand-in for Node's `crypto`: nanoid (image ids in html-to-docx) needs `randomFillSync`. */
export function randomFillSync<T extends ArrayBufferView>(buffer: T): T {
  const bytes = new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength)
  // getRandomValues fills at most 65536 bytes per call.
  for (let offset = 0; offset < bytes.length; offset += 65536) {
    globalThis.crypto.getRandomValues(bytes.subarray(offset, offset + 65536))
  }
  return buffer
}

export default { randomFillSync }
