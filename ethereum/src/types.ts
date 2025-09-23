export type IAddress = `0x${string}`;

export interface DecodedResult<T = unknown> {
  args: Array<{ data: T }>;
}
