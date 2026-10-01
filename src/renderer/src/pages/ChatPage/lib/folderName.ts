/**
 * Last path segment of a directory, which for the app's per-chat folders is the readable
 * `<chat-slug>-<shortid>` name. Handles both path separators so it works on Windows.
 */
export const folderName = (directory: string): string =>
  directory
    .replace(/[/\\]+$/, '')
    .split(/[/\\]/)
    .pop() ?? directory
