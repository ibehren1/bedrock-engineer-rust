declare module 'html-to-docx' {
  /**
   * Convert an HTML string to a .docx document: a Buffer when `global.Buffer` exists, otherwise
   * a Blob.
   */
  export default function HTMLtoDOCX(
    htmlString: string,
    headerHTMLString?: string | null,
    documentOptions?: Record<string, unknown>,
    footerHTMLString?: string | null
  ): Promise<Uint8Array | ArrayBuffer | Blob>
}
