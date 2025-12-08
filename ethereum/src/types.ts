export interface DecodedResult<T = unknown> {
  args: Array<{ data: T }>;
}

export enum STATUS {
  INITIATED = "initiated",
  IN_PROGRESS = "in progress",
  CLAIM_READY = "claim ready",
  BRIDGED = "bridged",
}
