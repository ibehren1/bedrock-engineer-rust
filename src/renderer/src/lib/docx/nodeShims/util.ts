/** Browser stand-in for the parts of Node's `util` that html-to-docx's dependencies call. */
export function inherits(ctor: any, superCtor: any): void {
  ctor.super_ = superCtor
  Object.setPrototypeOf(ctor.prototype, superCtor.prototype)
}

export const isArray = Array.isArray

export function isString(value: unknown): value is string {
  return typeof value === 'string'
}

export function deprecate<T>(fn: T): T {
  return fn
}

export default { inherits, isArray, isString, deprecate }
