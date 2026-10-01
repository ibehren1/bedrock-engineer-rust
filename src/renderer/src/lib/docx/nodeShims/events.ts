/**
 * Browser stand-in for Node's `events`, for html-to-docx and its dependencies (see
 * `../htmlToDocx.ts`). A function constructor, not a class: `queue` calls
 * `EventEmitter.call(this)`, which a class would reject.
 */
type Listener = (...args: any[]) => void

export function EventEmitter(this: any): void {
  this._events = Object.create(null)
}

const proto = EventEmitter.prototype as any

proto.on = proto.addListener = function (name: string, fn: Listener) {
  if (!this._events) this._events = Object.create(null)
  ;(this._events[name] ||= []).push(fn)
  return this
}
proto.prependListener = function (name: string, fn: Listener) {
  if (!this._events) this._events = Object.create(null)
  ;(this._events[name] ||= []).unshift(fn)
  return this
}
proto.once = function (name: string, fn: Listener) {
  const wrapped = (...args: any[]) => {
    this.off(name, wrapped)
    fn.apply(this, args)
  }
  return this.on(name, wrapped)
}
proto.off = proto.removeListener = function (name: string, fn: Listener) {
  const list: Listener[] | undefined = this._events?.[name]
  if (list) this._events[name] = list.filter((l) => l !== fn)
  return this
}
proto.removeAllListeners = function (name?: string) {
  if (!this._events) return this
  if (name === undefined) this._events = Object.create(null)
  else delete this._events[name]
  return this
}
proto.emit = function (name: string, ...args: any[]) {
  const list: Listener[] | undefined = this._events?.[name]
  if (!list || list.length === 0) {
    if (name === 'error') throw args[0] instanceof Error ? args[0] : new Error(String(args[0]))
    return false
  }
  for (const fn of [...list]) fn.apply(this, args)
  return true
}
proto.listeners = function (name: string) {
  return [...(this._events?.[name] ?? [])]
}
proto.listenerCount = function (name: string) {
  return this._events?.[name]?.length ?? 0
}
proto.setMaxListeners = function () {
  return this
}
;(EventEmitter as any).EventEmitter = EventEmitter

export default EventEmitter
