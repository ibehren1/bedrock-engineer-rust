/**
 * Browser stand-in for Node modules html-to-docx imports but the chat export never reaches:
 * `fs` / `path` (image-size reading files by path), `http` / `https` / `zlib` (node-fetch
 * downloading remote images). Every exported image is an inline data URL, so these only have to
 * exist when the module loads; any call fails loudly.
 */
function unavailable(name: string) {
  return () => {
    throw new Error(`${name} is not available in the renderer`)
  }
}

export const promises = { open: unavailable('fs.promises.open') }
export const openSync = unavailable('fs.openSync')
export const readSync = unavailable('fs.readSync')
export const closeSync = unavailable('fs.closeSync')
export const statSync = unavailable('fs.statSync')
export const fstatSync = unavailable('fs.fstatSync')
export const resolve = unavailable('path.resolve')
export const request = unavailable('http.request')
export const STATUS_CODES: Record<number, string> = {}
export const constants: Record<string, number> = {}
export const Z_SYNC_FLUSH = 2
export const createGunzip = unavailable('zlib.createGunzip')
export const createInflate = unavailable('zlib.createInflate')
export const createInflateRaw = unavailable('zlib.createInflateRaw')
export const createBrotliDecompress = unavailable('zlib.createBrotliDecompress')

export default {
  promises,
  openSync,
  readSync,
  closeSync,
  statSync,
  fstatSync,
  resolve,
  request,
  STATUS_CODES,
  constants,
  Z_SYNC_FLUSH,
  createGunzip,
  createInflate,
  createInflateRaw,
  createBrotliDecompress
}
