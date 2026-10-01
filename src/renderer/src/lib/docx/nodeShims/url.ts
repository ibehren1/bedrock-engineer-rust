/** Browser stand-in for Node's `url`, backed by the WHATWG `URL`. */
const NativeURL = globalThis.URL

export { NativeURL as URL }

export function parse(input: string): URL {
  return new NativeURL(input)
}

export function format(input: URL | string): string {
  return String(input)
}

export function domainToASCII(domain: string): string {
  try {
    return new NativeURL(`http://${domain}`).hostname
  } catch {
    return ''
  }
}

export function domainToUnicode(domain: string): string {
  return domain
}

export default { URL: NativeURL, parse, format, domainToASCII, domainToUnicode }
