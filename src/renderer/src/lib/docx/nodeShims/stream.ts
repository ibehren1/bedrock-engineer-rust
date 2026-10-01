/**
 * Browser stand-in for Node's `stream`. Only node-fetch (bundled into html-to-docx for remote
 * images, which the chat export never has) uses streams; it needs the classes to exist when the
 * module loads, not to work.
 */
import { EventEmitter } from './events'

export function Stream(this: any): void {
  EventEmitter.call(this)
}
Object.setPrototypeOf(Stream.prototype, EventEmitter.prototype)

export function Readable(this: any): void {
  Stream.call(this)
}
Object.setPrototypeOf(Readable.prototype, Stream.prototype)
;(Readable.prototype as any).destroy = function () {
  return this
}

export function PassThrough(this: any): void {
  Readable.call(this)
}
Object.setPrototypeOf(PassThrough.prototype, Readable.prototype)

Object.assign(Stream, { Stream, Readable, PassThrough })

export default Stream
